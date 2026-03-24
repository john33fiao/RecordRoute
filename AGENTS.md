# AGENTS.md

이 문서는 `RecordRoute` 코드베이스에서 작업하는 에이전트의 기본 운영 문서다. `GEMINI.md`와 `CLAUDE.md`는 이 문서를 우선 참조한다.

## 1. 저장소 개요

- 실제 애플리케이션 코드는 `rust/` 아래의 `record-route-api`다.
- 루트의 `ffmpeg/`, `llama.cpp/`, `whisper.cpp/`는 `.gitmodules`에 등록된 외부 서브모듈이다.
- 명시적 요청이 없는 한 기능 수정은 `rust/`에 한정하고, 서브모듈 코드는 수정하지 않는다.
- 런타임 구조는 `Axum API + Postgres + 백그라운드 워커 + HTTP sidecar(whisper, llama summary, llama embedding)` 조합이다.

## 2. 우선 확인할 파일

- `rust/src/lib.rs`: 앱 부트스트랩, 설정 로드, DB 연결, migration 실행, 워커 시작
- `rust/src/api/mod.rs`: 공개 HTTP 엔드포인트와 요청 검증
- `rust/src/jobs.rs`: 큐 polling, 재시도, 실패 처리
- `rust/src/pipeline.rs`: ffmpeg 변환, 전사, 요약, 임베딩 파이프라인
- `rust/src/storage.rs`: 파일 저장, 업로드 검증, Postgres 쿼리, pgvector 검색
- `rust/src/sidecar_clients.rs`: whisper/llama sidecar HTTP 계약
- `rust/src/models.rs`: 상태 enum, API 직렬화 타입
- `rust/migrations/0001_initial.sql`: 현재 스키마 기준점
- `rust/tests/mock_integration.rs`: 빠른 통합 테스트
- `rust/tests/real_sidecar_smoke.rs`: 실사이드카 연동용 ignored smoke placeholder
- `rust/.env.example`: 로컬 실행용 설정 템플릿

## 3. 현재 동작 기준

1. `POST /v1/recordings`
   multipart에서 첫 번째 파일 필드를 읽고 업로드를 저장한다.
   허용 확장자는 `mp3`, `m4a`, `wav`이며, 허용 content-type도 `storage.rs`에 하드코딩돼 있다.
2. 업로드 저장 후 `recordings`와 `jobs`에 row를 만들고 워커를 깨운다.
3. 워커는 queued job을 claim한 뒤 `ffmpeg`로 입력 파일을 `16kHz`, `mono`, `pcm_s16le` WAV로 변환한다.
4. whisper sidecar는 최초 한 번 `POST /load`로 모델을 적재하고, 이후 `POST /inference`로 `vjson` 응답을 받는다.
5. summary sidecar는 `POST /v1/chat/completions`를 호출하고 `{title, abstract, bullet_points}` JSON만 받아야 한다.
6. embedding sidecar는 `POST /v1/embeddings`를 호출하고 float 벡터를 받아야 한다.
7. transcript, summary, embedding을 저장한 뒤 job을 완료 처리한다.
8. `GET /v1/search`는 임베딩 생성이 실패하면 키워드 검색만으로 자동 fallback한다.

## 4. 파일/아티팩트 구조

- 원본 업로드: `APP_STORAGE_ROOT/{recording_id}/original/<sanitized filename>`
- 표준 WAV: `APP_STORAGE_ROOT/{recording_id}/wav/standard.wav`
- whisper 결과: `APP_STORAGE_ROOT/{recording_id}/artifacts/whisper-vjson.json`
- summary 결과: `APP_STORAGE_ROOT/{recording_id}/artifacts/summary.json`
- embedding 결과: `APP_STORAGE_ROOT/{recording_id}/artifacts/embedding.json`

파일명 sanitizing은 영숫자와 `.`, `_`, `-`만 유지하고 나머지는 `_`로 치환한다.

## 5. DB와 검색

- `recordings`: 원본 메타데이터, transcript, summary JSON, 상태, `search_document` TSVECTOR 저장
- `jobs`: recording당 1개 job, attempt 수와 retry 시점 저장
- `recording_embeddings`: 첫 임베딩 저장 시 동적으로 생성된다
- `vector` extension이 필요하다

중요한 제약:

- 임베딩 테이블의 차원 수는 첫 저장 시점의 벡터 길이로 고정된다.
- 다른 차원의 embedding 모델로 교체하면 현재 구현은 런타임 에러를 낸다.
- 검색 SQL은 키워드 검색과 cosine similarity를 함께 사용한다.

## 6. 환경 변수

`rust/.env.example` 기준 필수/기본값은 다음과 같다.

- `APP_BIND_ADDR` 기본값: `127.0.0.1:3000`
- `APP_STORAGE_ROOT` 필수
- `DATABASE_URL` 필수
- `FFMPEG_BIN` 기본값: `ffmpeg`
- `WHISPER_BASE_URL` 필수
- `LLAMA_SUMMARY_BASE_URL` 필수
- `LLAMA_EMBED_BASE_URL` 필수
- `WHISPER_MODEL` 필수
- `SUMMARY_MODEL` 필수
- `EMBED_MODEL` 필수
- `SIDECAR_TIMEOUT_SECS` 기본값: `300`
- `WORKER_POLL_INTERVAL_MS` 기본값: `1000`
- `MAX_UPLOAD_SIZE_BYTES` 기본값: `104857600`
- `MAX_JOB_ATTEMPTS` 기본값: `3`
- `JOB_RETRY_BACKOFF_SECS` 기본값: `15`
- `MAX_SEARCH_LIMIT` 기본값: `50`

코드상 `dotenv` 자동 로드는 구현되어 있지 않다. 실행 전에 환경 변수를 셸이나 런처에서 명시적으로 주입해야 한다.

## 7. 기본 작업 명령

작업 디렉터리는 기본적으로 `rust/`를 사용한다.

- 테스트: `cargo test`
- ignored smoke 확인: `cargo test -- --ignored`
- 로컬 실행: `cargo run`

실행 시 주의:

- 앱 시작 시 DB migration이 자동 실행된다.
- Postgres와 sidecar endpoint가 먼저 떠 있어야 한다.
- `real_sidecar_smoke.rs`는 현재 placeholder 수준이라, 실제 E2E 보장은 `mock_integration.rs`보다 약하다.

## 8. 변경 시 같이 봐야 하는 연동 지점

### API 응답/요청 필드 변경

아래를 함께 점검한다.

- `rust/src/models.rs`
- `rust/src/api/mod.rs`
- `rust/src/storage.rs` row mapping
- `rust/tests/mock_integration.rs`

### 처리 단계 변경

아래를 함께 점검한다.

- `rust/src/models.rs`의 `ProcessingStep`, `ProcessingStatus`
- `rust/src/pipeline.rs`
- `rust/src/jobs.rs`
- `rust/src/storage.rs`
- 상태를 노출하는 API 응답과 테스트

### 업로드 허용 형식 변경

아래를 함께 점검한다.

- `rust/src/storage.rs`의 확장자 검사
- `rust/src/storage.rs`의 content-type 검사
- 업로드 API 테스트

### 임베딩/검색 변경

아래를 함께 점검한다.

- `rust/src/sidecar_clients.rs`
- `rust/src/storage.rs`의 `ensure_embedding_table_for_dimension`, `save_embedding`, `search_recordings`
- DB migration 전략

### 스키마 변경

- 이미 배포 가능한 저장소라는 가정으로, 기존 migration을 덮어쓰기보다 신규 migration 추가를 우선한다.
- 스키마를 바꾸면 SQL row mapping과 테스트 데이터를 반드시 같이 갱신한다.

## 9. 에이전트 작업 원칙

- 먼저 `rust/`에서 해결 가능한지 판단하고, 서브모듈 수정은 마지막 수단으로 둔다.
- 변경은 최소 범위로 하고, 현재 구현된 HTTP 계약과 파일 구조를 임의로 바꾸지 않는다.
- 파이프라인 실패 처리와 retry 동작은 사용자 기능이다. 에러를 삼키지 말고 상태/last_error 갱신 흐름을 유지한다.
- 검색은 “임베딩이 있으면 hybrid, 실패하면 keyword-only”라는 현재 동작을 깨지 않도록 주의한다.
- 새 설정값을 추가하면 `rust/src/config.rs`와 `rust/.env.example`를 함께 갱신한다.
- 테스트를 추가할 때는 가능한 한 `mock_integration.rs`처럼 외부 의존성 없는 빠른 경로를 우선한다.

## 10. 건드리지 말아야 할 것

- 사용자 요청이 없는 `ffmpeg/`, `llama.cpp/`, `whisper.cpp/` 서브모듈 내부 코드
- `rust/target/` 산출물
- 임베딩 차원과 search SQL을 고려하지 않은 상태에서의 `recording_embeddings` 스키마 변경

## 11. 얇은 참조 문서 정책

- `GEMINI.md`와 `CLAUDE.md`는 이 문서를 복제하지 않는다.
- 기본 지침 변경은 `AGENTS.md`만 수정하고, 나머지 문서는 참조 관계만 유지한다.
