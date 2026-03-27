# RecordRoute 임베딩 및 유사도 검색 계획

이 문서는 RecordRoute의 summary 결과물 기준 임베딩 및 유사도 검색 기능에 대한 설계/검증 문서다.
초기에는 계획 초안으로 시작했지만, 현재는 코드베이스와 대조한 구현 상태와 남은 TODO까지 함께 기록한다.
1~9장은 설계 기준과 운영 가정을, 10장 이후는 2026-03-27 기준 구현 검증 결과를 정리한다.

## 1. 배경과 목표

현재 RecordRoute의 최종 LLM 산출물은 `db/<job_id>/summary/result.md`에 저장된다.
이 결과는 사람 읽기용 회의 요약으로는 충분하지만, 다음과 같은 검색 요구를 충족하지 못한다.

- 유사한 회의 요약 찾기
- 특정 질의와 가까운 summary 찾기
- 기존 summary 자산을 재사용 가능한 검색 인덱스로 활용하기

따라서 v1 목표는 summary 결과 텍스트를 문서 단위로 임베딩해 간단한 유사도 검색을 제공하는 것이다.

핵심 원칙:

- llama.cpp는 계속 Rust 직접 링크가 아니라 CLI 래핑 모델로 유지한다.
- 상태와 메타데이터의 SoT는 계속 `db/index.json`으로 유지한다.
- v1은 외부 vector DB 없이 파일 기반으로 구현한다.
- API와 CLI는 같은 도메인 로직을 공유하는 기존 방향을 유지한다.

v1 범위:

- summary/result.md 단위 임베딩 생성
- summary 임베딩 산출물 저장과 재사용
- brute-force cosine similarity 기반 검색
- 신규 summary 자동 임베딩
- 기존 summary backfill 경로 정의

이번 v1에서 제외:

- transcript chunk 단위 임베딩
- summary 섹션 단위 분할 검색
- reranker
- ANN 인덱스
- 외부 vector DB 또는 검색엔진 연동

## 2. 현재 상태 요약

현재 구현 기준 사실:

- summary 출력 경로는 `db/<job_id>/summary/result.md`다.
- `db/index.json`은 현재 버전 `3`이고, `jobs`, `model_preparations`, `summary_embedding` 호환 데이터를 함께 저장한다.
- `TaskType`은 `ffmpeg`, `stt`, `summary`, `embedding`을 가진다.
- Llama 모델 준비는 `prepare-llama-model` CLI와 `POST /models/llama/prepare` API에서 공통 로직을 사용하며, summary model + embedding model umbrella prepare로 동작한다.
- summary 생성은 `submit_summary_job()` / `execute_summary_job()`에서 수행되고, 성공 직후 embedding task가 자동 제출될 수 있다.
- HTTP 인터페이스는 `/jobs/{job_id}/summary/embedding`, `POST /summary/search`까지 제공한다.
- `/models/status`는 `embedding_available`, `embedding_ready`, `embedding_error`를, `/system/status`는 `llama_embedding_model_ready`를 노출한다.
- llama toolchain 탐색은 `.build/llama/<os>-<arch>/bin/llama-cli`와 `llama-embedding` 둘 다를 찾는다.

현재 한계:

- 검색은 여전히 brute-force cosine scan이며 corpus가 커질수록 비용이 증가한다.
- query embedding은 요청 시점에만 계산하고 별도 캐시는 없다.
- transcript chunk 검색, section-level retrieval, reranker, ANN, external vector DB는 아직 범위 밖이다.

## 3. 제안 아키텍처

### 3.1 모델 정책

기본 정책은 summary model과 embedding model을 분리하되, embedding model에는 저장소 기본값을 둔다는 것이다.

- 기존 summary model은 `RECORDROUTE_LLAMA_MODEL`을 계속 사용한다.
- 새 embedding model override는 `RECORDROUTE_LLAMA_EMBEDDING_MODEL`을 사용한다.
- `RECORDROUTE_LLAMA_EMBEDDING_MODEL`이 비어 있으면 기본값은 `Qwen/Qwen3-Embedding-4B-GGUF`를 사용한다.
- llama.cpp 실행 경로는 로컬 GGUF 파일 또는 Hugging Face repo 문자열 둘 다를 지원한다.
- 현재 구현은 다운로드된 GGUF를 `models/llama/hf/<cache-key>.gguf` 경로에 캐시해 재사용한다.
- 값 해석 규칙은 기존 llama model과 유사하게 유지한다.
  - 파일 경로면 로컬 모델 파일로 사용
  - 파일이 아니면 Hugging Face repo 문자열로 해석

운영 가정:

- `RECORDROUTE_LLAMA_EMBEDDING_MODEL`이 없어도 embedding 기능은 기본값 정책으로 동작한다.
- embedding model resolve/download/prepare에 실패해도 summary 기능은 계속 동작한다.
- 이 경우 embedding/search 기능만 비활성화하거나 `not ready` 상태로 노출한다.

### 3.2 llama 준비 흐름 확장

기존 `/models/llama/prepare`와 `prepare-llama-model`는 summary model + embedding model을 함께 준비하는 umbrella 동작으로 확장한다.

계획 방향:

- summary model 준비 로직은 기존 동작을 유지한다.
- embedding model override가 있으면 그 값을, 없으면 기본값 `Qwen/Qwen3-Embedding-4B-GGUF`를 기준으로 같은 prepare 경로에서 함께 준비한다.
- embedding model resolve/download/prepare가 실패한 경우 prepare 전체가 실패하지는 않으며, embedding 관련 상태만 비활성화한다.
- build 스크립트는 `llama-cli`뿐 아니라 `llama-embedding`도 산출하도록 확장한다.
- Rust 쪽 toolchain discovery도 summary용 실행 파일과 embedding용 실행 파일을 함께 찾도록 바꾼다.

### 3.3 저장 구조

실제 벡터는 job 디렉터리 아래 sidecar 파일로 저장하고, `db/index.json`에는 재사용 판단에 필요한 최소 metadata만 저장한다.

계획 저장 위치:

- 벡터 파일: `db/<job_id>/summary/embedding.json`
- index metadata: `JobRecord.summary_embedding`

`JobRecord.summary_embedding` 계획 필드:

- `model_id`: 어떤 embedding model로 생성했는지 식별
- `text_sha256`: `summary/result.md` 본문 해시
- `dimension`: 벡터 차원 수
- `normalized`: L2 normalization 여부
- `file_path`: `embedding.json` 경로
- `created_at`: 생성 시각

현재 구현은 `IndexFile.version`이 `3`이며, version 2 index는 `summary_embedding` 없이도 읽히도록 호환성을 유지한다.

### 3.4 Task 및 상태 모델

임베딩은 별도 task로 다룬다.

계획 변경:

- `TaskType`에 `embedding` 추가
- summary 텍스트 변경 또는 embedding model 변경 시만 embedding task 제출
- summary task 성공 직후 embedding task를 연쇄 실행해 신규 job은 자동으로 검색 대상에 포함

재사용/중복제거 규칙:

- embedding task가 `running`이면 `Deduplicated`
- 같은 `text_sha256` + 같은 `model_id` + 산출물 파일 존재 시 `Reused`
- 그 외에는 `Submitted`

stale 판단 기준:

- `summary/result.md` 내용이 바뀐 경우
- effective embedding model id 또는 GGUF 선택값이 바뀐 경우
- `embedding.json` 파일이 없거나 손상된 경우

## 4. 검색 설계

### 4.1 검색 단위

v1 검색 단위는 summary 문서 1개 = 벡터 1개다.

즉, 한 job에서 검색 대상은 `summary/result.md` 하나이며, transcript chunk 또는 summary 하위 섹션은 다루지 않는다.

### 4.2 벡터 생성과 비교 방식

기본 비교 방식은 L2-normalized vector의 cosine similarity다.

계획 동작:

- summary 임베딩은 생성 시 normalize해서 저장
- query도 요청 시 즉시 임베딩하고 normalize
- corpus 전체를 순회하며 brute-force로 cosine similarity 계산
- score 내림차순으로 정렬해 상위 결과 반환

query embedding은 요청 시점에만 계산하고, v1에서는 별도 캐시하지 않는다.

### 4.3 검색 결과 형식

검색 결과는 최소한 아래 필드를 포함하는 방향으로 잡는다.

- `job_id`
- `score`
- `source_file_name`
- `summary_file_name`
- `summary_excerpt`

기본 정책:

- `limit` 기본값은 `10`
- `limit` 최대값은 `50`
- `min_score`는 선택값

## 5. 인터페이스 현황

이 절은 현재 구현된 HTTP/CLI 인터페이스를 요약한다. 상세 스키마는 `docs/openapi.yaml`을 기준으로 본다.

### 5.1 HTTP

- `POST /jobs/{job_id}/summary/embedding`
  - summary 임베딩 생성 제출, 재사용, 중복 합류
- `GET /jobs/{job_id}/summary/embedding`
  - embedding task 상태와 current metadata 조회
- `POST /summary/search`
  - `{ query, limit?, min_score? }` 기반 유사도 검색

상태 조회 확장 후보:

- `GET /models/status`
  - llama 항목에 `embedding_available`, `embedding_ready`, `embedding_error` 추가
- `GET /system/status`
  - 기존 `llama_model_ready`는 summary readiness 의미로 유지
  - `llama_embedding_model_ready` 추가

### 5.2 CLI

- `prepare-llama-model`
  - summary model + embedding model umbrella prepare
- `embed-summaries`
  - 기존 completed summary backfill 및 stale 재생성
- `search-summaries "<query>"`
  - 동일 검색 로직을 CLI에서 직접 호출

## 6. Backfill 및 운영 고려사항

신규 summary만 자동 처리하면 기존 데이터가 검색 대상에서 빠지므로 별도 backfill 경로가 필요하다.

계획 방향:

- `embed-summaries` CLI 또는 동등한 app 레이어 경로로 기존 completed job 전체를 순회
- `summary/result.md`가 있고 embedding metadata가 없거나 stale이면 재생성
- summary 파일이 없으면 건너뜀
- embedding model resolve/download/prepare에 실패하면 backfill/search는 비활성화하고, summary 기능은 영향 없이 유지

운영 상 고려할 점:

- 모델 변경 시 기존 embedding corpus를 전부 stale로 판단할 수 있어야 한다.
- embedding 산출물 삭제 또는 손상 시 index metadata만 믿지 않고 파일 존재를 함께 검증해야 한다.
- v1에서는 corpus가 커질수록 brute-force 비용이 증가하므로, 대규모 corpus 대응은 후속 단계 과제로 남긴다.

## 7. 구현 시 검증 기준

후속 구현 단계에서 확인할 항목:

- version 2 index를 읽어도 embedding metadata default가 안전하게 채워진다.
- running/reused/submitted 판정이 `text_sha256`와 `model_id` 기준으로 정확하다.
- summary 성공 후 embedding이 자동 제출된다.
- summary 재생성 시 stale embedding이 무효화된다.
- normalized vectors 기준 cosine score 정렬이 맞다.
- `limit`, `min_score`, empty corpus, missing artifact 처리가 의도대로 동작한다.
- embedding model env override 유무와 resolve/download/prepare 실패 여부에 따라 prepare/status/search 동작이 올바르게 갈린다.
- per-job embedding submit/get, global search, backfill command가 같은 app 레이어 로직을 사용한다.

문서 완료 기준:

- 현재 구조와 향후 계획이 명확히 분리되어 있다.
- 모델 정책, 저장 위치, 재사용 규칙, 검색 단위, 제외 범위가 모두 명시되어 있다.
- 후속 구현자가 이 문서만 보고 필요한 코드 변경 축을 파악할 수 있다.

## 8. 현재도 범위 밖인 것

- transcript chunk 단위 임베딩/검색
- summary 섹션 단위 분할 검색
- reranker
- ANN 인덱스
- external vector DB 또는 검색엔진 연동

## 9. 가정

- 현재 저장소에는 `docs/API_Doc.md`가 없으므로, 이 문서는 `docs/architecture.md`와 `docs/openapi.yaml`을 기준으로 작성한다.
- separate embedding model 정책을 기본안으로 채택한다.
- embedding model env가 없으면 기본값 `Qwen/Qwen3-Embedding-4B-GGUF`를 사용한다.
- embedding model resolve/download/prepare 실패 시 summary 기능은 유지되고, embedding/search 기능만 비활성화되는 방향을 기본 가정으로 둔다.
- v1은 운영 단순성과 현재 저장소의 파일 기반 SoT 유지에 우선순위를 둔다.

## 10. 구현 상태 체크리스트 (코드베이스 기준, 2026-03-27)

> 기준: 현재 `rust/src`와 `docs/openapi.yaml` 구현을 대조해 완료/미완료를 표기한다.
> 상태 표기: `완료`, `부분 완료`, `미완료`

### 10.1 v1 범위

- `완료`: summary/result.md 단위 임베딩 생성
- `완료`: summary 임베딩 산출물 저장과 재사용
- `완료`: brute-force cosine similarity 기반 검색
- `완료`: 신규 summary 자동 임베딩
- `완료`: 기존 summary backfill 경로(`embed-summaries`) 제공

### 10.2 모델 정책/준비

- `완료`: `RECORDROUTE_LLAMA_EMBEDDING_MODEL`이 metadata 식별(`model_id`)과 실제 embedding 실행 경로 둘 다에 반영됨
- `완료`: 임베딩 실행 모델이 summary 모델과 분리되어 동작함
- `완료`: llama toolchain에서 `llama-cli`와 `llama-embedding` 실행 파일을 함께 탐색함
- `완료`: `prepare-llama-model`/`/models/llama/prepare`가 summary+embedding umbrella prepare로 동작함
- `완료`: embedding prepare 실패를 독립 상태(`embedding_error`)로 반영하고 summary와 분리 노출함

### 10.3 저장 구조/인덱스

- `완료`: `db/<job_id>/summary/embedding.json` sidecar 저장
- `완료`: `JobRecord.summary_embedding` metadata 저장
- `완료`: `IndexFile.version` 3 반영 및 `summary_embedding` 기본값 호환 처리

### 10.4 Task/상태 모델

- `완료`: `TaskType.embedding` 추가
- `완료`: summary 변경/모델 id 변경/sidecar 손상 감지 시 stale로 재생성
- `완료`: running/reused/submitted 판정 로직 반영
- `완료`: summary 성공 직후 embedding 연쇄 실행

### 10.5 검색 설계

- `완료`: 문서 1개(summary 1개) = 벡터 1개 단위
- `완료`: query 즉시 임베딩 후 corpus 전체 brute-force 스코어링
- `완료`: score 내림차순 정렬
- `완료`: `limit` 기본 10, 최대 50 적용
- `완료`: `min_score` 선택 필터 적용
- `완료`: 결과 필드(`job_id`, `score`, `source_file_name`, `summary_file_name`, `summary_excerpt`) 반환

### 10.6 인터페이스(HTTP/CLI/OpenAPI)

- `완료`: `POST/GET /jobs/{job_id}/summary/embedding` 구현
- `완료`: `POST /summary/search` 구현
- `완료`: `embed-summaries`, `search-summaries "<query>"` CLI 구현
- `완료`: `/models/status`, `/system/status`에 embedding 관련 필드 노출
- `완료`: OpenAPI에 임베딩/검색 경로 및 스키마 반영

### 10.7 이번 작업 범위(문서 전용)와의 정합성

- `완료`: 문서 상단, 인터페이스 절, 범위 절을 현재 코드 기준 표현으로 정리함

## 11. 구현 로드맵(권장 순서)

아래 순서는 후속 구현자가 리스크를 낮추면서 단계적으로 합칠 수 있도록 정리한 권장안이다.

### Phase 1: 데이터 모델/인덱스 기반 마련

- `IndexFile.version` 3 도입 및 v2 읽기 호환 추가
- `JobRecord.summary_embedding` metadata 스키마 추가
- `TaskType::embedding` 추가(기존 상태 전이 규칙 유지)
- summary embedding stale/reuse 판정 함수(app/index 공용) 추가

Phase 1 DoD:

- v2 인덱스를 로드해도 읽기 실패 없이 기본값으로 해석된다.
- v3 저장 시 `summary_embedding` 누락/부분값에 대한 방어 로직이 있다.
- stale/reuse 판정이 `text_sha256 + model_id + 파일 존재` 기준으로 일관되게 동작한다.

### Phase 2: llama embedding 실행 경로 추가

- llama toolchain 탐색에서 `llama-embedding` 탐지 추가
- summary용/embedding용 모델 resolve 로직 분리(공통 인터페이스 유지)
- `prepare-llama-model` umbrella prepare로 확장
- embedding 준비 실패 시 summary 경로 비차단 보장

Phase 2 DoD:

- summary 모델 준비 성공 + embedding 모델 실패 상황에서 prepare 명령/API가 부분 성공 상태를 표현한다.
- embedding 불가 상태가 `/models/status`, `/system/status`에 반영될 준비가 되어 있다.

### Phase 3: 임베딩 생성/저장 및 자동 연쇄

- `summary/result.md -> embedding.json` 생성 경로 구현
- summary 성공 직후 embedding task 자동 제출
- per-job embedding submit/get API 및 app 레이어 진입점 추가

Phase 3 DoD:

- 신규 summary 생성 시 embedding task가 자동으로 제출된다.
- 같은 입력/같은 모델에 대해 재요청 시 `Reused` 또는 `Deduplicated`가 정확히 반환된다.
- 산출물 손상/누락 시 자동으로 stale 처리되어 재생성 경로로 진입한다.

### Phase 4: 검색/백필 경로 완성

- `POST /summary/search` 구현(브루트포스 cosine)
- `embed-summaries` 백필 CLI 구현(또는 동등 app 경로)
- empty corpus, `limit`, `min_score` 정책 확정 및 에러 핸들링

Phase 4 DoD:

- 검색 결과가 score 내림차순으로 안정 정렬된다.
- backfill이 missing summary를 건너뛰고, stale만 재생성한다.
- query 임베딩 실패/모델 미준비 시 명확한 오류 또는 not ready 응답을 반환한다.

## 12. 테스트 전략(구현 단계 체크리스트)

단위 테스트(우선):

- `summary text -> sha256` 변경 감지 테스트
- stale 판정 조합 테스트
  - 동일 text/hash + 동일 model + 파일 존재 = 재사용
  - text 변경/모델 변경/파일 누락 = stale
- cosine similarity 계산/정렬 테스트

통합 테스트(가능 범위):

- summary 완료 후 embedding task 자동 제출
- per-job embedding submit/get 상태 전이
- global search 결과 스키마/기본 limit 적용

수동 검증(운영 시나리오):

- `RECORDROUTE_LLAMA_EMBEDDING_MODEL` 유/무에 따른 준비 경로 확인
- embedding 모델 준비 실패 시 summary 파이프라인이 계속 동작하는지 확인
- 기존 v2 index 데이터에서 backfill 정상 동작 확인

## 13. 롤백/장애 대응 기준

- 기능 플래그 관점:
  - embedding 준비 실패 시 검색 기능만 비활성화하고 summary는 유지
  - 필요 시 embedding task 제출 자체를 일시 비활성화할 수 있어야 함
- 데이터 관점:
  - `summary_embedding` metadata 제거/무시 시에도 기존 summary 조회 기능은 유지
  - `embedding.json` 파일 손상 시 재생성 가능해야 하며, 수동 삭제만으로도 복구 가능해야 함
- 릴리스 관점:
  - Phase 단위로 분리 배포하여 장애 발생 시 직전 Phase로 되돌릴 수 있도록 구성

## 14. 오픈 이슈(구현 전 확정 필요)

- `llama-embedding` 실행 파라미터 표준화(차원/정규화 옵션 고정 여부)
- `summary_excerpt` 생성 규칙(길이, 마크다운 제거 여부, 멀티바이트 안전 자르기)
- 검색 응답 스키마에 `model_id`/`created_at` 노출 여부
- embedding 준비 상태 표현 시 `error` 필드의 민감 정보 마스킹 수준
