# RecordRoute 임베딩 및 유사도 검색 계획

이 문서는 RecordRoute에 summary 결과물 기준 임베딩 및 유사도 검색 기능을 도입하기 위한 계획 초안이다.
현재 구현 상태를 설명하는 문서가 아니라, 후속 구현 범위와 설계 방향을 고정하기 위한 기준 문서다.
이번 작업 범위는 이 문서 작성까지이며, 코드, OpenAPI, 아키텍처 문서는 아직 변경하지 않는다.

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
- `db/index.json`은 현재 버전 `2`이고, `jobs` 및 `model_preparations`를 저장한다.
- `TaskType`은 현재 `ffmpeg`, `stt`, `summary`만 가진다.
- Llama 모델 준비는 `prepare-llama-model` CLI와 `POST /models/llama/prepare` API에서 공통 로직을 사용한다.
- summary 생성은 `submit_summary_job()` / `execute_summary_job()`에서 수행되며, `ensure_model_prepared(..., ModelKind::Llama)` 이후 `llama-cli`를 실행한다.
- 현재 HTTP 인터페이스는 `/jobs/{job_id}/summary`, `/jobs/{job_id}/summary/text`까지 제공하며, 임베딩 또는 검색용 엔드포인트는 없다.
- 현재 llama toolchain 탐색은 `.build/llama/<os>-<arch>/bin/llama-cli` 기준이다.

현재 한계:

- summary 텍스트는 생성 후 그대로 파일로만 남고 검색용 구조가 없다.
- summary가 바뀌었는지, 어떤 모델로 임베딩했는지 추적하는 메타데이터가 없다.
- embedding model 준비 상태를 따로 표현할 방법이 없다.
- 질의를 벡터화하고 corpus 전체와 비교하는 공통 경로가 없다.

## 3. 제안 아키텍처

### 3.1 모델 정책

기본 정책은 summary model과 embedding model을 분리하는 것이다.

- 기존 summary model은 `RECORDROUTE_LLAMA_MODEL`을 계속 사용한다.
- 새 embedding model은 `RECORDROUTE_LLAMA_EMBEDDING_MODEL`을 사용한다.
- 값 해석 규칙은 기존 llama model과 동일하다.
  - 파일 경로면 로컬 모델 파일로 사용
  - 파일이 아니면 Hugging Face repo 문자열로 해석

운영 가정:

- embedding model이 설정되지 않아도 summary 기능은 계속 동작한다.
- embedding/search 기능만 비활성화하거나 `not configured` 상태로 노출한다.

### 3.2 llama 준비 흐름 확장

기존 `/models/llama/prepare`와 `prepare-llama-model`는 summary model + embedding model을 함께 준비하는 umbrella 동작으로 확장한다.

계획 방향:

- summary model 준비 로직은 기존 동작을 유지한다.
- embedding model이 설정된 경우 같은 prepare 경로에서 함께 준비한다.
- embedding model이 미설정인 경우 prepare 전체가 실패하지는 않으며, embedding 관련 상태만 비활성화한다.
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

후속 구현 시 `IndexFile.version`은 `3`으로 올리고, version 2 index는 `summary_embedding` 없이도 읽히도록 호환성을 유지한다.

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
- `RECORDROUTE_LLAMA_EMBEDDING_MODEL` 값이 바뀐 경우
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

## 5. 인터페이스 초안

이 절은 구현된 인터페이스가 아니라 향후 도입 후보를 정리한 것이다.
이번 문서 작업에서 `docs/openapi.yaml` 또는 Rust 코드는 수정하지 않는다.

### 5.1 HTTP 후보

추가 후보:

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

### 5.2 CLI 후보

추가 후보:

- `prepare-llama-model`
  - summary model + embedding model umbrella prepare
- `embed-summaries`
  - 기존 completed summary backfill 및 stale 재생성
- `search-summaries "<query>"`
  - 동일 검색 로직을 CLI에서 직접 호출

문서 독자가 혼동하지 않도록, 위 항목은 모두 계획안이며 현재 구현은 아니다.

## 6. Backfill 및 운영 고려사항

신규 summary만 자동 처리하면 기존 데이터가 검색 대상에서 빠지므로 별도 backfill 경로가 필요하다.

계획 방향:

- `embed-summaries` CLI 또는 동등한 app 레이어 경로로 기존 completed job 전체를 순회
- `summary/result.md`가 있고 embedding metadata가 없거나 stale이면 재생성
- summary 파일이 없으면 건너뜀
- embedding model 미설정이면 backfill/search는 비활성화하고, summary 기능은 영향 없이 유지

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
- embedding model env 유무에 따라 prepare/status/search 동작이 올바르게 갈린다.
- per-job embedding submit/get, global search, backfill command가 같은 app 레이어 로직을 사용한다.

문서 완료 기준:

- 현재 구조와 향후 계획이 명확히 분리되어 있다.
- 모델 정책, 저장 위치, 재사용 규칙, 검색 단위, 제외 범위가 모두 명시되어 있다.
- 후속 구현자가 이 문서만 보고 필요한 코드 변경 축을 파악할 수 있다.

## 8. 이번 작업에 포함하지 않는 것

이번 문서 작성 작업에는 아래가 포함되지 않는다.

- `docs/openapi.yaml` 직접 수정
- `docs/architecture.md` 직접 수정
- Rust 타입, 도메인 로직, API, CLI 구현
- ANN, chunk retrieval, reranker, external vector DB 설계 구체화

## 9. 가정

- 현재 저장소에는 `docs/API_Doc.md`가 없으므로, 이 문서는 `docs/architecture.md`와 `docs/openapi.yaml`을 기준으로 작성한다.
- separate embedding model 정책을 기본안으로 채택한다.
- embedding model 미설정 시 summary 기능은 유지되고, embedding/search 기능만 비활성화되는 backward-compatible opt-in 방향을 기본 가정으로 둔다.
- v1은 운영 단순성과 현재 저장소의 파일 기반 SoT 유지에 우선순위를 둔다.
