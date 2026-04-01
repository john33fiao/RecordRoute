# RecordRoute API 기준 문서 (2026-03-31 코드 기준)

이 문서는 RecordRoute의 현재 HTTP API를 에이전트가 빠르게 파악하고 작업 기준으로 삼기 위한 문서입니다.

- 에이전트 작업의 1차 기준 문서: `docs/API_Doc.md`
- 상세 wire schema / 컴포넌트 스키마: `docs/openapi.yaml`
- 구현 확인 기준: `rust/src/server.rs`, `rust/src/server/types.rs`, `rust/src/index/types.rs`
- 범위: 현재 제공 중인 로컬 HTTP API만 포함합니다. CLI 모드와 내부 Rust 모듈 API는 포함하지 않습니다.

## 1. 공통 규칙

### 1.1 서버와 저장소

- 기본 bind 주소: `127.0.0.1:38080`
- 메타데이터 SoT 기본값: `db/index.sqlite3`
- 오디오 저장 SoT 기본값: `db/audio/`
- 메타데이터 backend는 SQLite 기본, `StorageConfig`로 PostgreSQL도 지원합니다.
- 패키지 런처는 서버를 `RECORDROUTE_QUEUE_START_PAUSED=1`로 띄운 뒤 `/queue/pause`로 재개시키는 경로를 사용합니다.

### 1.2 핵심 상태와 타입

- `JobStatus`: `queued`, `running`, `completed`, `failed`
- `TaskType`: `ffmpeg`, `stt`, `summary`, `embedding`
- `TaskStatus`: `queued`, `running`, `completed`, `failed`
- `SourceKind`: `local_file`, `upload`
- `SplitStrategy`: `per_channel_plus_merged_mono`, `merged_mono_only`
- `QueueCategory`: `ffmpeg`, `stt`, `llm`, `embed`

주의:

- API에서 task 타입은 `summary`, `embedding`을 쓰지만 queue category는 각각 `llm`, `embed`를 씁니다.
- `JobRecord`에는 `split_strategy`, `summary_embedding`, `tasks`가 포함됩니다.
- `TaskRecord`에는 `task_id`, `retry_count`, `last_error`, 시각 필드, dedupe용 `request_fingerprint`가 포함됩니다.

### 1.3 비동기 제출 의미

- `Submitted`: 실제 새 작업이 큐에 들어가거나 즉시 실행 대상이 됨
- `Reused`: 완료된 기존 산출물을 재사용함
- `Deduplicated`: 같은 작업이 이미 `queued` 또는 `running` 상태여서 기존 실행에 합류함

상태 코드 관례:

- `200 OK`: 조회 성공, 완료 job 재사용, 또는 모델이 이미 준비된 경우
- `202 Accepted`: 비동기 작업 제출이 접수된 경우
- stage 제출 API인 `POST /jobs/{job_id}/stt`, `POST /jobs/{job_id}/summary`, `POST /jobs/{job_id}/summary/embedding`은 재사용 또는 중복 합류여도 `202`를 유지하고, 본문의 `reused`/`deduplicated`로 구분합니다.
- `400 Bad Request`: 잘못된 JSON, 잘못된 경로/식별자, 잘못된 상태 조합
- `404 Not Found`: job, transcript, dictionary keyword, file 등이 없음
- `503 Service Unavailable`: 외부 툴체인 또는 모델 준비 상태 문제

## 2. 엔드포인트 개요

### 2.1 Web 자산

보조 라우트이며 에이전트 작업의 주 대상은 아닙니다.

- `GET /`
  - 내장 Web UI의 index HTML을 반환합니다.
- `GET /app.js`
  - 내장 Web UI JavaScript를 반환합니다.
- `GET /app.css`
  - 내장 Web UI CSS를 반환합니다.

### 2.2 Server / System / Models

- `POST /server/ping`
  - 목적: JSON round-trip 확인용 ping
  - 요청: `code`, `message`
  - 응답: `PingResponse`
  - 현재 구현은 요청의 `message`를 포함한 환영 문구를 돌려주고, 응답 `code`는 항상 문자열 `"200"`입니다.

- `GET /system/status`
  - 목적: ffmpeg / whisper / llama 가용성과 모델 준비 여부의 빠른 스냅샷 확인
  - 응답: `SystemStatusResponse`
  - 핵심 필드: `ffmpeg_available`, `whisper_available`, `llama_available`, `whisper_model_ready`, `llama_model_ready`, `llama_embedding_model_ready`, `errors`

- `GET /models/status`
  - 목적: 모델 준비 상태를 더 자세히 확인
  - 응답: `ModelStatusResponse`
  - 핵심 필드:
    - `whisper.preparation`
    - `llama.preparation`
    - `llama.embedding_ready`, `llama.embedding_error`
  - 준비 상태 레코드는 `idle`, `running`, `completed`, `failed`를 사용합니다.

- `POST /models/whisper/prepare`
  - 목적: whisper 모델 준비 작업 제출
  - 응답: `ModelPrepareResponse`
  - `200`: 이미 준비됨
  - `202`: 새 준비 작업 접수 또는 기존 준비 작업에 합류

- `POST /models/llama/prepare`
  - 목적: summary용 llama 모델과 embedding용 llama 모델을 함께 준비
  - 응답: `ModelPrepareResponse`
  - `200`: 이미 준비됨
  - `202`: 새 준비 작업 접수 또는 기존 준비 작업에 합류
  - 이 엔드포인트는 summary와 embedding 모델 준비를 umbrella 작업으로 다룹니다.

### 2.3 Queue

- `GET /queue`
  - 목적: 영속 queue 상태 확인
  - 응답: `QueueStatusResponse`
  - 핵심 필드: `paused`, `burst_limit`, `active_batch`, `pending_batches`

- `POST /queue/pause`
  - 목적: queue 일시정지 또는 재개
  - 요청: `{ "paused": true | false }`
  - 응답: `QueueStatusResponse`
  - `paused=false`로 재개하면 dispatcher를 깨워 대기 작업을 다시 진행시킵니다.
  - 런처 경로에서는 서버가 처음부터 paused로 올라올 수 있으므로, 초기 복구/검증 뒤 이 엔드포인트로 재개하는 흐름을 염두에 둡니다.

- `POST /queue/cancel-pending`
  - 목적: 현재 실행 중인 작업은 유지하고, pending queue만 비웁니다.
  - 응답: `QueueCancelPendingResponse`
  - 카운터 필드: `total_cancelled`, `ffmpeg_cancelled`, `stt_cancelled`, `summary_cancelled`, `embedding_cancelled`

### 2.4 Statistics

- `GET /stats/overview`
  - 목적: 업로드 기준 처리 현황을 단계별 스냅샷으로 집계
  - 응답: `StatsOverviewResponse`
  - 핵심 필드:
    - `generated_at`: 통계 스냅샷 생성 시각
    - `upload_job_count`: `source_kind=upload` 기준 job 수
    - `all_job_count`: 전체 job 수(`local_file` 포함)
    - `stages`: `ffmpeg`, `stt`, `summary`, `embedding` 순서의 단계별 집계
  - `stages[]` 필드:
    - `stage`: 파이프라인 단계
    - `label`: UI 표시용 단계 이름
    - `completed_count`: 해당 단계 산출물이 저장된 job 수
    - `in_progress_count`: 해당 단계 task가 `queued` 또는 `running`인 job 수
    - `unprocessed_count`: 완료/처리중/실패 어디에도 속하지 않는 upload job 수
  - summary/embedding 단계는 persisted 결과 유무를 완료 기준으로 봅니다.
  - 실패 task는 이 통계에서 제외됩니다.

### 2.5 Dictionary

- `GET /dictionary/keywords`
  - 목적: STT dictionary 키워드 조회
  - 응답: `DictionaryKeywordListResponse`
  - 필드:
    - `user_keywords`: 사용자가 관리하는 키워드
    - `auto_keywords`: summary에서 자동 추출된 전역 후보 키워드

- `POST /dictionary/keywords`
  - 목적: user keyword 추가
  - 요청: `{ "keyword": "..." }`
  - 응답: `DictionaryKeywordListResponse`
  - 이미 auto 목록에 있던 키워드를 user로 승격하는 경우도 포함합니다.

- `POST /dictionary/keywords/auto/{keyword}/promote`
  - 목적: auto keyword를 user keyword로 승격
  - 응답: `DictionaryKeywordListResponse`
  - 없으면 `404`

- `DELETE /dictionary/keywords/auto`
  - 목적: auto keyword 전체 삭제
  - 응답: `DictionaryKeywordListResponse`
  - auto keyword가 비어 있어도 `200`으로 현재 목록을 반환합니다.

- `DELETE /dictionary/keywords/auto/{keyword}`
  - 목적: auto keyword 삭제
  - 응답: `DictionaryKeywordListResponse`
  - 없으면 `404`

- `DELETE /dictionary/keywords/{keyword}`
  - 목적: user keyword 삭제
  - 응답: `DictionaryKeywordListResponse`
  - 없으면 `404`

주의:

- 현재 STT 프롬프트에 자동 주입되는 것은 `user_keywords`입니다.
- `auto_keywords`는 review 및 promote 대상이며 아직 자동 주입 기준이 아닙니다.

### 2.6 Jobs

- `POST /jobs`
  - 목적: 서버 로컬 파일 경로로 ffmpeg job 생성 또는 재사용
  - 요청: `{ "input_path": "/absolute/or/local/path.wav" }`
  - 응답: `JobSubmissionResponse`
  - `200`: 완료 job 재사용
  - `202`: 새 job 접수 또는 실행 중 job dedup
  - 핵심 응답 필드: `job_id`, `status`, `reused`, `deduplicated`, `queue`, `source_ref`, `source_kind`, `source_content_sha256`, `probe`, `outputs`

- `GET /jobs`
  - 목적: job 목록 조회
  - query:
    - `source_ref`: 정확히 일치하는 source reference만 필터링
  - 응답: `JobListResponse`

- `GET /jobs/completed`
  - 목적: 완료된 job만 조회
  - 응답: `JobListResponse`

- `POST /jobs/upload`
  - 목적: multipart 업로드 후 ffmpeg job 생성 또는 재사용
  - 요청: `multipart/form-data`의 `file`
  - 응답: `JobSubmissionResponse`
  - 업로드 최대 크기: `512MB`
  - `200`: 완료 job 재사용
  - `202`: 새 job 접수 또는 실행 중 job dedup
  - `400`: `file` 필드 누락, 중복 `file` 필드, 빈 파일, malformed multipart
  - `413`: 파일 크기 제한 초과

- `POST /jobs/batch-process`
  - 목적: 기존 job들을 batch queue에 넣어 전체 파이프라인 또는 특정 stage만 재진행
  - 요청 본문: 생략 가능
  - 요청 필드:
    - `target`: `all`, `ffmpeg`, `stt`, `summary`, `embedding`
  - 응답: `BatchQueueSubmissionResponse`
  - 빈 본문 또는 공백 본문은 `target=all`로 해석됩니다.
  - 이 엔드포인트는 현재 스냅샷에서 조건을 만족한 task만 큐에 넣습니다.
  - 같은 요청 안에서 미래 summary 결과를 예측해 embedding까지 예약하지는 않습니다.

- `GET /jobs/{job_id}`
  - 목적: 전체 `JobRecord` 조회
  - 응답: `JobRecord`
  - 상태, source 정보, split 전략, outputs, task 목록, embedding metadata까지 확인할 때 사용합니다.

- `GET /jobs/{job_id}/status`
  - 목적: job 상태와 task 상태만 빠르게 조회
  - 응답: `JobStatusResponse`
  - 필드: `job_status`, `tasks`, `error_message`

- `POST /jobs/{job_id}/reset`
  - 목적: source/job identity는 유지한 채 특정 stage 산출물만 삭제
  - 요청 필드: `all`, `ffmpeg`, `stt`, `summary`, `embedding`
  - 응답: `JobResetResponse`
  - 전제 조건:
    - 전역 queue가 비어 있어야 함
    - 어떤 job에도 `queued` 또는 `running` task가 없어야 함
  - `all=true`면 나머지 stage 선택을 모두 포함하는 master flag로 정규화됩니다.

### 2.7 STT

- `POST /jobs/{job_id}/stt`
  - 목적: STT task 제출
  - 요청: `SttRequest`
  - 요청 필드:
    - `audio_files`: job의 logical audio file subset. 예: `["channel_01.wav"]`
    - `mono_mix_only`: `true`면 `mono_mix.wav`만 대상으로 고정
    - `keywords`: proper noun / domain term 힌트
  - 응답: `TaskSubmissionResponse`
  - 상태 코드는 항상 `202`
  - 제약:
    - `mono_mix_only=true`와 비어 있지 않은 `audio_files`를 동시에 사용할 수 없음
    - language는 요청이 아니라 서버 환경 변수 `RECORDROUTE_WHISPER_LANGUAGE`에서 가져옴

- `GET /jobs/{job_id}/stt`
  - 목적: STT task 상태 조회
  - 응답: `TaskSubmissionResponse`
  - 아직 STT task가 없으면 `task=null`일 수 있습니다.

- `GET /jobs/{job_id}/stt/progress`
  - 목적: 현재 STT 진행률 스냅샷 조회
  - 응답: `SttProgressResponse`
  - 핵심 필드: `phase`, `total_files`, `completed_files`, `progress_percent`
  - `phase`: `idle`, `queued`, `running`, `completed`, `failed`

- `GET /jobs/{job_id}/stt/texts`
  - 목적: transcript 목록 조회
  - 응답: `SttTranscriptListResponse`
  - 각 transcript는 `transcript_id`, `file_name`, `text`를 가집니다.

- `GET /jobs/{job_id}/stt/texts/{transcript_id}`
  - 목적: 단일 transcript 조회
  - 응답: `SttTranscriptText`
  - `transcript_id`는 slash, backslash, `..`를 포함할 수 없습니다.

### 2.8 Summary / Embedding / Search

- `POST /jobs/{job_id}/summary`
  - 목적: summary task 제출
  - 요청 본문: 생략 가능
  - 요청 필드:
    - `force_regenerate`: `true`면 summary 재생성 의도
  - 응답: `TaskSubmissionResponse`
  - 상태 코드는 항상 `202`
  - 현재 구현은 본문이 없거나 JSON 파싱에 실패해도 `force_regenerate=false`처럼 처리합니다.

- `GET /jobs/{job_id}/summary`
  - 목적: summary task 상태 조회
  - 응답: `TaskSubmissionResponse`

- `GET /jobs/{job_id}/summary/text`
  - 목적: summary markdown 본문 조회
  - 응답: `SummaryTextResponse`
  - 핵심 필드:
    - `file_name`: 보통 `result.md`
    - `text`: markdown summary
    - `one_line_summary`: 한 줄 요약 alias

- `POST /jobs/{job_id}/summary/embedding`
  - 목적: summary embedding task 제출
  - 응답: `SummaryEmbeddingResponse`
  - 상태 코드는 항상 `202`
  - summary 산출물이 없으면 `400`
  - summary 성공이 embedding 제출로 자동 연쇄되지는 않으므로, 검색 최신화가 필요하면 이 엔드포인트나 batch/backfill 경로를 별도로 호출해야 합니다.

- `GET /jobs/{job_id}/summary/embedding`
  - 목적: embedding task 상태와 metadata 조회
  - 응답: `SummaryEmbeddingResponse`
  - `metadata`에는 `model_id`, `text_sha256`, `dimension`, `normalized`, `created_at`가 들어갑니다.

- `POST /summary/search`
  - 목적: summary embedding 기반 similarity search
  - 요청: `SummarySearchRequest`
  - 요청 필드:
    - `query`: 필수
    - `limit`: 생략 시 `10`, 최소 `1`, 최대 `50`
    - `min_score`: 선택
  - 응답: `SummarySearchResponse`
  - 검색 결과 항목:
    - `job_id`
    - `score`
    - `source_file_name`
    - `summary_file_name`
    - `summary_excerpt`
  - 현재 summary 본문과 어긋난 stale embedding은 결과에서 제외됩니다.
  - embedding toolchain 또는 모델이 준비되지 않았으면 `503`

### 2.9 Files

- `GET /jobs/{job_id}/files`
  - 목적: 다운로드 가능한 logical file 목록 조회
  - 응답: `FileListResponse`
  - 반환 가능한 범주:
    - 오디오 산출물 logical name
    - `stt/<file_name>`
    - `summary/<file_name>`

- `GET /jobs/{job_id}/files/{*file_name}`
  - 목적: 허용된 단일 산출물 다운로드
  - 예시:
    - `mono_mix.wav`
    - `channel_01.wav`
    - `stt/channel_01.txt`
    - `summary/result.md`
  - 응답: raw bytes
  - 주의:
    - 실제 axum 라우트는 wildcard `/{*file_name}`입니다.
    - 슬래시가 포함된 경로는 클라이언트에서 URL 인코딩해야 합니다.
    - 역슬래시, 절대경로, `..`는 거부됩니다.
    - 텍스트 산출물도 raw bytes로 내려오며 Content-Type을 명시적으로 세팅하지 않을 수 있습니다.

## 3. 에이전트용 권장 흐름

### 3.1 로컬 파일 기반 처리

1. `POST /jobs`로 job을 제출합니다.
2. `GET /jobs/{job_id}` 또는 `GET /jobs/{job_id}/status`로 ffmpeg 완료를 확인합니다.
3. `POST /jobs/{job_id}/stt`로 transcript를 생성합니다.
4. `GET /jobs/{job_id}/stt/texts`로 transcript를 확인합니다.
5. `POST /jobs/{job_id}/summary`로 summary를 생성합니다.
6. `GET /jobs/{job_id}/summary/text`로 요약 본문과 `one_line_summary`를 확인합니다.
7. `POST /jobs/{job_id}/summary/embedding`으로 embedding을 생성합니다.
8. `POST /summary/search`로 유사 summary 검색을 수행합니다.

### 3.2 업로드 기반 처리

1. `POST /jobs/upload`로 파일을 업로드하며 job을 생성합니다.
2. 이후 흐름은 로컬 파일 기반 처리와 동일합니다.
3. 업로드 실패 또는 용량 초과는 `400` 또는 `413`을 우선 확인합니다.

### 3.3 Reset 후 재처리

1. `POST /jobs/{job_id}/reset` 전에 `GET /queue`로 queue가 idle인지 확인합니다.
2. reset 응답의 `deleted`와 `job`을 보고 어느 stage가 비워졌는지 확인합니다.
3. 필요한 stage부터 다시 제출합니다.
4. summary만 reset한 경우 embedding도 stale 해질 수 있으므로 summary 이후 embedding까지 다시 맞춥니다.

## 4. 유지보수 규칙

- API 의미를 바꾸는 변경은 먼저 `docs/API_Doc.md`를 갱신합니다.
- 그 다음 `docs/openapi.yaml`을 wire schema 기준으로 맞춥니다.
- 문서 반영 후에는 최소한 아래 파일과의 정합성을 다시 확인합니다.
  - `rust/src/server.rs`
  - `rust/src/server/routes/*.rs`
  - `rust/src/server/types.rs`
  - `rust/src/index/types.rs`
- 엔드포인트를 추가하거나 제거하면 `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`의 API 참조 문구가 새 기준을 계속 가리키는지도 확인합니다.
