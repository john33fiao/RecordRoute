# RecordRoute 임베딩과 요약 검색 (2026-03-28 코드 기준)

이 문서는 현재 구현된 summary embedding, backfill, similarity search 동작을 설명한다.
초기 설계 초안이 아니라 현행 코드 설명 문서이며, 세부 HTTP 스키마는 `docs/openapi.yaml`을 본다.

## 1. 범위

현재 임베딩 기능의 대상은 job당 summary 문서 1개다.

- 입력 문서: `db/<job_id>/summary/result.md`
- 임베딩 단위: summary 문서 전체 1개
- 저장 방식: 파일 sidecar + `db/index.json` metadata
- 검색 방식: brute-force cosine similarity

현재 범위 밖:

- transcript chunk 단위 검색
- summary 섹션 단위 검색
- query embedding 캐시
- ANN 인덱스
- reranker
- 외부 vector DB

## 2. 모델 정책

### 2.1 summary 모델과 embedding 모델

summary와 embedding은 서로 다른 모델 소스를 쓸 수 있다.

- summary 모델: `RECORDROUTE_LLAMA_MODEL`
- embedding 모델: `RECORDROUTE_LLAMA_EMBEDDING_MODEL`

기본값:

- summary: `ggml-org/gemma-3-4b-it-GGUF`
- embedding: `Qwen/Qwen3-Embedding-4B-GGUF`

해석 규칙:

- 환경변수 값이 실제 파일이면 local GGUF 파일로 사용
- 아니면 Hugging Face repo 문자열로 해석

Hugging Face repo를 쓰는 경우 캐시 경로는 `models/llama/hf/*.gguf`다.

### 2.2 준비 흐름

`prepare-llama-model`과 `POST /models/llama/prepare`는 umbrella prepare다.
즉 summary 모델 준비와 embedding 모델 준비를 같은 진입점에서 다룬다.

관찰 가능한 상태:

- `/system/status`
  - `llama_model_ready`
  - `llama_embedding_model_ready`
- `/models/status`
  - `llama.available`
  - `llama.ready`
  - `llama.embedding_available`
  - `llama.embedding_ready`
  - `llama.embedding_error`

주의:

- summary 생성 자체는 embedding 성공에 의존하지 않는다.
- summary 생성 후 embedding 연쇄 실행이 실패해도 summary task는 유지되고 embedding task만 실패할 수 있다.
- 반면 umbrella prepare는 embedding toolchain 또는 embedding 모델 설정 문제를 같이 드러낼 수 있다.

## 3. 저장 구조

### 3.1 sidecar 파일

실제 벡터는 `summary/embedding.json`에 저장된다.

경로:

- `db/<job_id>/summary/embedding.json`

파일 형식:

- `model_id`
- `text_sha256`
- `dimension`
- `normalized`
- `created_at`
- `vector`

### 3.2 인덱스 metadata

`db/index.json`의 `JobRecord.summary_embedding`에는 검색 재사용 판단에 필요한 최소 정보만 저장한다.

필드:

- `model_id`
- `text_sha256`
- `dimension`
- `normalized`
- `created_at`
- `file_path`

현재 인덱스 포맷 버전은 `3`이며, `summary_embedding`이 없는 이전 데이터도 읽을 수 있게 유지돼 있다.

### 3.3 task 모델

임베딩은 독립 task로 기록된다.

- `TaskType.embedding`
- `TaskStatus.running | completed | failed`

즉 summary 생성과 embedding 생성은 같은 job 아래 서로 다른 task 레코드로 남는다.

## 4. 생성 흐름

### 4.1 자동 연쇄

`execute_summary_job()`가 성공하면 `submit_summary_embedding_job()`을 호출한다.
새로운 embedding이 필요하다고 판단되면 같은 흐름 안에서 best-effort로 바로 실행한다.

즉 신규 summary는 별도 호출 없이 자동으로 검색 대상이 되도록 설계돼 있다.

### 4.2 submit 시 재사용 / 중복방지

`submit_summary_embedding_job()` 판정 규칙:

- embedding task가 이미 `running`이면 `Deduplicated`
- summary가 없으면 에러
- sidecar와 metadata가 현재 summary 본문, 현재 embedding 모델과 일치하면 `Reused`
- 그 외에는 `Submitted`

### 4.3 stale 판정

다음 중 하나라도 만족하면 stale로 보고 재생성한다.

- `JobRecord.summary_embedding`이 없음
- `summary/result.md`가 없음
- 현재 summary 본문의 sha256이 저장된 `text_sha256`과 다름
- 현재 embedding 모델 id가 저장된 `model_id`와 다름
- sidecar 파일을 읽을 수 없음
- sidecar의 `text_sha256`, `model_id`, `dimension`이 index metadata와 다름

## 5. embedding 실행 방식

`rust/src/llama.rs`는 `llama-embedding`을 직접 실행한다.

핵심 인자:

- `--pooling mean`
- `--embd-normalize 2`
- `--embd-output-format array`
- `-p <summary_text>`

모델 지정:

- local 파일이면 `-m <path>`
- Hugging Face repo면 `-hf <repo>` + `LLAMA_CACHE=<download_cache_dir>`

출력 파싱:

- 단일 `Vec<f32>` JSON
- 중첩 `Vec<Vec<f32>>` JSON
- `{"data":[{"embedding":[...]}]}` 형태

이 세 형식을 순서대로 허용한다.

실행 실패 시에는 summary용 llama와 같은 CPU fallback 규칙을 사용한다.

## 6. 검색 동작

### 6.1 검색 입력

`search_summaries()`와 `POST /summary/search`는 query 문자열을 같은 embedding 모델로 즉시 임베딩한다.

현재 query embedding 캐시는 없다.

### 6.2 후보 선정

completed job 전체를 순회하면서 다음 조건을 만족하는 job만 후보로 삼는다.

- `summary_embedding` metadata 존재
- sidecar 읽기 성공
- sidecar의 `model_id`가 현재 embedding 모델과 일치
- sidecar의 `dimension`이 query 벡터 길이와 일치

조건을 만족하지 않는 job은 조용히 건너뛴다.

### 6.3 스코어 계산

sidecar와 query 모두 정규화된 벡터라는 전제에서 dot product를 cosine similarity로 사용한다.

처리 순서:

1. 후보 전체를 순회
2. `min_score`가 있으면 임계치 미만 제거
3. score 내림차순 정렬
4. `limit`만큼 truncate

HTTP API는 `limit`를 `1..=50`으로 clamp 한다.
기본값은 `10`이다.

### 6.4 결과 필드

현재 검색 결과는 다음 필드를 돌려준다.

- `job_id`
- `score`
- `source_file_name`
- `summary_file_name`
- `summary_excerpt`

`summary_excerpt`는 summary 파일을 읽어 줄바꿈을 공백으로 바꾸고, 공백을 정규화한 뒤 최대 200자까지만 잘라 만든다.

## 7. CLI와 HTTP 인터페이스

### 7.1 CLI

- `prepare-llama-model`
  - summary + embedding 모델 준비
- `embed-summaries`
  - completed job 전체를 돌며 stale embedding만 재생성
- `search-summaries "<query>"`
  - 검색 결과를 `job_id score excerpt` 형태로 출력

### 7.2 HTTP

- `POST /jobs/{job_id}/summary/embedding`
  - embedding 제출, 재사용, 중복 합류
- `GET /jobs/{job_id}/summary/embedding`
  - embedding task 상태 + 현재 metadata 조회
- `POST /summary/search`
  - summary similarity search

status 계열:

- `GET /system/status`
- `GET /models/status`

## 8. 운영상 특징과 한계

- summary 파일명은 canonical path `summary/result.md`로 통일돼 있다.
- embedding metadata와 sidecar가 불일치하면 자동으로 stale로 본다.
- 검색 코퍼스가 커질수록 전체 스캔 비용이 그대로 증가한다.
- query 캐시와 ANN이 없으므로 latency는 코퍼스 크기에 선형으로 비례한다.
- embedding task 완료와 `summary_embedding` metadata 기록은 현재 두 번의 인덱스 업데이트로 나뉘어 있어, 아주 짧은 순간에는 task 상태와 metadata가 완전히 동시에 보이지 않을 수 있다.

## 9. 문서 갱신 기준

이 문서가 바뀌어야 하는 경우:

- embedding 모델 기본값 또는 환경변수 규칙 변경
- sidecar 형식 변경
- stale 판정 기준 변경
- 검색 랭킹 또는 limit 정책 변경
- CLI/HTTP 진입점 변경
