# RecordRoute Architecture

이 문서는 `RecordRoute`의 현재 코드 기준 아키텍처와 구현 상태를 기록한다. 사용자 사용법이 아니라, 저장소 구조, 런타임 구성, 모듈 경계, 데이터 흐름, 현재 구현 범위와 제약을 빠르게 파악하기 위한 참조 문서다.

## 1. 문서 범위

- 기준 코드: 루트 `rust/` 아래 `record-route-api`
- 기준 외부 의존성: `ffmpeg/`, `whisper.cpp/`, `llama.cpp/`는 서브모듈이며 애플리케이션이 직접 링크하지 않고 외부 실행 파일 또는 HTTP sidecar로 사용한다.
- 최종 기준은 항상 코드다. 이 문서는 현재 구현을 요약한 스냅샷이며, 코드 변경 시 같이 갱신하는 것을 전제로 한다.

## 2. 저장소 구성

| 경로 | 역할 |
| --- | --- |
| `rust/` | 실제 애플리케이션 코드와 테스트 |
| `rust/src/lib.rs` | 앱 부트스트랩, 설정 로드, SQLite 초기화, worker 시작 |
| `rust/src/api/mod.rs` | 공개 HTTP API와 요청 검증 |
| `rust/src/jobs.rs` | 큐 polling, job claim, 재시도/실패 처리 |
| `rust/src/pipeline.rs` | ffmpeg 변환, 전사, 요약, 임베딩 파이프라인 |
| `rust/src/storage.rs` | 로컬 파일 저장, SQLite 접근, FTS 검색, artifact similarity |
| `rust/src/sidecar_clients.rs` | whisper/llama sidecar HTTP 클라이언트 |
| `rust/src/models.rs` | API/DB 직렬화 타입, 상태 enum |
| `rust/tests/mock_integration.rs` | 외부 서비스 없는 빠른 테스트 |
| `rust/tests/sqlite_repository.rs` | 실제 SQLite 저장소/검색 검증 |
| `rust/tests/real_sidecar_smoke.rs` | 실사이드카 smoke placeholder |

## 3. 런타임 토폴로지

```mermaid
flowchart LR
    Client["HTTP client"] --> API["Axum API"]
    API --> SQLITE["SQLite file"]
    API --> FS["Local storage root"]
    API --> EMBQ["Embedding sidecar<br/>query embedding"]
    API --> Worker["In-process JobWorker"]

    Worker --> SQLITE
    Worker --> FS
    Worker --> FFMPEG["ffmpeg CLI"]
    Worker --> Whisper["Whisper sidecar"]
    Worker --> Summary["Llama summary sidecar"]
    Worker --> Embed["Llama embedding sidecar"]
```

현재 구조는 하나의 Rust 프로세스 안에 HTTP 서버와 background worker가 같이 들어있는 형태다. 업로드 요청은 SQLite에 job을 적재하고 `Notify`로 worker를 깨운다. 실제 비동기 처리와 검색도 같은 프로세스 안에서 수행된다.

## 4. 프로세스 시작 순서

`rust/src/lib.rs` 기준 시작 순서는 다음과 같다.

1. tracing 초기화
2. 환경 변수에서 `AppConfig` 생성
3. 로컬 저장소 루트 디렉터리 생성
4. SQLite DB 파일 초기화와 schema 보장
5. whisper / summary / embedding HTTP 클라이언트 생성
6. `JobWorker` 생성 후 Tokio task로 spawn
7. Axum router 구성 후 bind / serve

중요한 현재 상태:

- `.env` 자동 로드는 없다. 환경 변수는 실행 전에 외부에서 주입해야 한다.
- DB schema는 앱 시작 시 `SqliteRepository` 초기화에서 보장한다.
- worker는 별도 프로세스가 아니라 앱 생명주기와 같이 시작/종료된다.
- SQLite 경로 기본값은 `APP_STORAGE_ROOT/record-route.db`다.

## 5. 모듈 경계와 책임

### `config.rs`

- 환경 변수 파싱과 기본값 처리만 담당한다.
- 현재 설정은 bind address, SQLite 파일 경로, storage root, ffmpeg 경로, sidecar base URL, 모델 이름, timeout, 업로드 제한, worker retry 정책, 검색 limit으로 구성된다.

### `api/mod.rs`

- 현재 공개 엔드포인트는 `POST /v1/recordings`, `GET /v1/recordings`, `GET /v1/recordings/:recording_id`, `GET /v1/jobs/:job_id`, `GET /v1/search`다.
- 업로드는 multipart에서 첫 번째 파일 필드만 처리하고 나머지는 무시한다.
- 검색 시 query embedding 생성이 실패하면 경고 로그를 남기고 keyword-only 검색으로 fallback한다.

### `jobs.rs`

- 저장소에서 처리 가능한 queued job을 하나 claim한다.
- 없으면 `Notify` 또는 poll interval 중 먼저 오는 이벤트를 기다린다.
- SQLite claim은 `BEGIN IMMEDIATE` 트랜잭션으로 보호된다.

### `pipeline.rs`

- 실제 오디오 처리와 sidecar 호출을 오케스트레이션한다.
- `ProcessingStep`을 기준으로 오류를 분류한다.
- WAV, transcript, summary는 이미 있으면 재사용한다.
- embedding은 artifact 파일을 다시 기록하고, DB에는 메타데이터만 저장한다.

### `storage.rs`

- `RecordingRepository` trait로 저장소 추상화를 유지한다.
- 실제 구현은 `SqliteRepository`, 테스트용 구현은 `MockRepository`다.
- `LocalFileStore`는 업로드 검증, 파일명 sanitizing, artifact 쓰기, 상대 경로 계산을 담당한다.
- 검색 시 keyword score는 SQLite FTS5와 filename `LIKE`를 사용하고, similarity는 `embedding.json`을 읽어 cosine으로 계산한다.

## 6. 요청과 처리 흐름

### 업로드에서 완료까지

1. `POST /v1/recordings`
2. multipart의 첫 파일을 `LocalFileStore::save_upload_field`로 저장
3. `recordings`, `jobs` row를 같은 요청 안에서 생성
4. worker를 `notify_one`
5. worker가 `claim_next_job`으로 가장 오래된 queued job을 claim
6. `ffmpeg`로 `16kHz`, `mono`, `pcm_s16le` WAV 생성
7. whisper sidecar로 전사
8. summary sidecar로 구조화 요약 생성
9. embedding sidecar로 summary canonical text 임베딩 생성
10. DB 메타데이터와 artifact 파일 갱신 후 job 완료 처리

### 실패와 재시도

- 파이프라인 실패는 `ProcessingStep`과 함께 기록된다.
- claim 시 `attempt_count`가 증가한다.
- `max_attempts` 미만이면 `queued`로 되돌리고 `next_attempt_at`을 미래 시점으로 설정한다.
- 최대 시도 수를 넘기면 `failed`로 종료한다.
- `recordings.last_error`와 `jobs.last_error`를 둘 다 갱신한다.

## 7. 파일 저장 구조

현재 파일 구조는 `APP_STORAGE_ROOT` 아래 recording 단위 디렉터리로 정리된다.

| 경로 | 내용 |
| --- | --- |
| `{recording_id}/original/{sanitized filename}` | 원본 업로드 |
| `{recording_id}/wav/standard.wav` | 표준화된 WAV |
| `{recording_id}/artifacts/whisper-vjson.json` | whisper 원본 응답 |
| `{recording_id}/artifacts/summary.json` | 구조화 요약 JSON |
| `{recording_id}/artifacts/embedding.json` | 임베딩 벡터와 차원 메타데이터 |

파일명 sanitizing 규칙:

- 유지: 영숫자, `.`, `_`, `-`
- 치환: 그 외 문자는 `_`
- 결과가 비면 기본 파일명은 `upload.wav`

## 8. DB 구조와 검색 방식

### `recordings`

- 원본 메타데이터, WAV 경로, transcript, summary JSON, canonical summary text, 상태, 에러, 생성/수정 시각 저장
- `has_embedding`, `embedding_dim`으로 artifact 존재 여부와 차원을 저장한다.

### `jobs`

- recording 당 정확히 1개 job
- 상태, 현재 단계, attempt 수, last error, started/completed 시각, 다음 재시도 시각 저장

### `recordings_fts`

- SQLite FTS5 virtual table
- `original_filename`, `summary_canonical_text`, `transcript`를 색인한다.
- 동기화는 DB trigger가 아니라 repository write path에서 수동으로 수행한다.

### 검색

- keyword 검색: SQLite FTS5 + filename `LIKE`
- similarity 검색: query embedding 생성 후 각 recording의 `embedding.json`을 읽어 cosine similarity 계산
- query embedding 생성이 실패하면 keyword-only로 자동 전환
- artifact가 없거나 깨져 있거나 차원이 다르면 해당 recording의 similarity만 제외한다.
- 현재 동작은 keyword + vector union에 가깝다.

## 9. 현재 구현 상태

### 구현되어 있는 것

- Axum 기반 업로드/조회/검색 API
- SQLite 기반 recording/job 저장
- in-process worker와 retry/backoff 처리
- ffmpeg를 통한 업로드 오디오 표준화
- whisper 전사, llama summary, llama embedding sidecar 연동
- 로컬 JSON artifact 저장
- SQLite FTS + artifact similarity 기반 hybrid search

### 현재 코드에 없는 것

- 인증/권한 관리
- recording 삭제, 재처리, 취소용 API
- 별도 worker 프로세스 또는 외부 큐 브로커
- chunk 단위 임베딩, diarization, speaker metadata
- 다중 인스턴스 분산 처리 보장

### 운영상 주의가 필요한 것

- `real_sidecar_smoke.rs`는 placeholder 수준이라 실제 sidecar E2E 보장을 제공하지 않는다.
- 빠른 테스트는 `MockRepository` 기반이라 전체 SQLite 경로를 모두 검증하지 않는다.
- SQLite 설계는 단일 인스턴스 로컬 실행을 기준으로 한다.
- similarity는 artifact full-scan 기반이므로 초기 소규모 데이터셋에 맞춘 구현이다.

## 10. 테스트 전략

현재 테스트는 세 갈래다.

- `rust/tests/mock_integration.rs`
  - API 라우팅
  - 업로드 저장
  - 파이프라인 상태 전이
  - 검색 fallback
- `rust/tests/sqlite_repository.rs`
  - 실제 SQLite 저장소 초기화
  - FTS 검색
  - artifact similarity와 손상 artifact skip 동작
- `rust/tests/real_sidecar_smoke.rs`
  - ignored placeholder

즉, 현재 저장소는 빠른 mock 회귀와 SQLite 저장소 검증은 갖췄고, 실사이드카 E2E는 여전히 약한 상태다.
