# RecordRoute 아키텍처 (2026-03-28 코드 기준)

이 문서는 현재 `rust/src/**` 구현을 기준으로 RecordRoute의 실행 구조, 상태 저장 방식, 파이프라인, 서버 구성을 요약한다.
설계 기준보다 코드가 우선이며, HTTP 상세 스키마는 `docs/openapi.yaml`을 함께 본다.

## 1. 엔트리포인트와 런타임 루트

### 1.1 실행 바이너리

- `rust/src/main.rs`
  - 기본 CLI 엔트리포인트
  - `recordroute_rust::main_cli()` 호출
- `rust/src/bin/recordroute.rs`
  - 패키징된 런처 엔트리포인트
  - `RecordRouteServer` 프로세스를 띄우고 브라우저를 연다
- `rust/src/bin/recordroute_server.rs`
  - 패키징된 전용 서버 엔트리포인트
  - `recordroute_rust::main_server()` 호출

### 1.2 런타임 루트 결정

`rust/src/runtime_root.rs`는 다음 순서로 런타임 루트를 찾는다.

1. 환경변수 `RECORDROUTE_RUNTIME_ROOT`
2. 현재 실행 파일 디렉터리 안의 `.recordroute-runtime-root`
3. 현재 실행 파일 디렉터리 안의 `RecordRouteServer(.exe)`
4. 개발 환경에서는 저장소 루트

런처와 전용 서버 바이너리는 이 런타임 루트를 기준으로 `db/`, `models/`, `logs/`, `.env`를 읽는다.

### 1.3 런처 동작

`rust/src/launcher.rs`의 패키징 런처는 다음 순서로 동작한다.

1. `logs/server.pid`로 기존 자신이 띄운 서버를 정리
2. `127.0.0.1:38080` 포트 사용 가능 여부 확인
3. `RecordRouteServer`를 하위 프로세스로 실행
4. `/server/ping` 응답을 polling 해서 준비 완료 확인
5. 기본 브라우저에서 `http://127.0.0.1:38080/` 오픈
6. 표준출력/표준에러를 `logs/server.log`에 누적 기록

## 2. 코드 모듈 지도

### 2.1 도메인 로직

- `rust/src/app.rs`
  - facade와 공용 타입 정의
- `rust/src/app/ffmpeg_stage.rs`
  - ffmpeg job 제출/실행/재사용
- `rust/src/app/stt_stage.rs`
  - STT task 제출/실행
- `rust/src/app/summary_stage.rs`
  - summary task 제출/실행
- `rust/src/app/embedding_stage.rs`
  - summary embedding 생성, backfill, 검색
- `rust/src/app/models.rs`
  - whisper/llama 준비 상태 관리, heartbeat, umbrella prepare
- `rust/src/app/artifacts.rs`
  - 산출물 경로 규약과 legacy 파일명 승격
- `rust/src/app/cli.rs`
  - CLI 명령 파싱과 출력

### 2.2 서버 계층

- `rust/src/server.rs`
  - axum router 조립, `spawn_blocking` 경계, 서버 바인드
- `rust/src/server/routes/*.rs`
  - 기능별 HTTP 핸들러
- `rust/src/server/types.rs`
  - 요청/응답 스키마
- `rust/src/server/files.rs`
  - 파일/텍스트/진행률 조회
- `rust/src/server/upload.rs`
  - multipart 업로드 저장 및 제한
- `rust/src/server/errors.rs`
  - `AppError`를 HTTP 응답으로 변환

### 2.3 상태 저장 계층

- `rust/src/index.rs`
  - 저장소 facade
- `rust/src/index/types.rs`
  - `IndexFile`, `JobRecord`, `TaskRecord`, `ModelPreparationRecord`
- `rust/src/index/store.rs`
  - 인덱스 읽기/쓰기, 원자적 replace
- `rust/src/index/lock.rs`
  - `db/index.lock` 파일 기반 동기화

## 3. CLI 표면

직접 인자 모드에서 지원하는 명령은 다음과 같다.

- `ffmpeg <input>`
- `<input>` 단독 인자
- `stt`
- `summary`
- `prepare-models`
- `prepare-llama-model`
- `embed-summaries`
- `search-summaries <query>`
- `server`

인자 없이 실행하면 대화형 선택 프롬프트가 뜨지만, 현재 프롬프트 메뉴는 `ffmpeg`, `stt`, `summary`, `server`만 노출한다.
즉 `prepare-models`, `embed-summaries`, `search-summaries`는 직접 인자로만 호출할 수 있다.

## 4. 상태 저장과 데이터 모델

### 4.1 저장소 구조

- SoT: `db/index.json`
- 동기화 락: `db/index.lock`
- 업로드 캐시: `db/uploads/<sha256>.bin`
- job 산출물: `db/<job_id>/...`

`IndexStore`는 인덱스를 임시 파일에 먼저 기록하고 `rename`으로 교체한다.
즉 JSON 저장은 overwrite가 아니라 원자적 replace 흐름이다.

### 4.2 인덱스 포맷

현재 `IndexFile.version`은 `3`이다.

핵심 필드:

- `model_preparations`
  - `whisper`
  - `llama`
  - `llama_embedding`
- `jobs`

### 4.3 Job / Task

`JobRecord` 핵심 필드:

- `job_id`
- `status`: `running | completed | failed`
- `source_path`, `source_file_name`
- `job_dir`
- `probe`
- `split_strategy`
- `outputs`
- `error_message`
- `summary_embedding`
- `tasks`

`TaskType`:

- `ffmpeg`
- `stt`
- `summary`
- `embedding`

`TaskRecord`:

- `task_id` (uuid)
- `status`: `running | completed | failed`
- `started_at`, `finished_at`
- `last_error`
- `retry_count`

### 4.4 모델 준비 상태

`ModelPreparationRecord`는 다음 필드를 가진다.

- `status`: `idle | running | completed | failed`
- `started_at`
- `finished_at`
- `heartbeat_at`
- `last_error`

`app/models.rs`는 준비 중 모델에 대해 10초 heartbeat를 기록하고, 60초 동안 heartbeat가 갱신되지 않으면 stale running으로 본다.

## 5. 산출물 규약

### 5.1 FFmpeg 산출물

- `db/<job_id>/mono_mix.wav`
- `db/<job_id>/channel_01.wav`
- `db/<job_id>/channel_02.wav`
- ...

### 5.2 STT 산출물

- `db/<job_id>/stt/<audio_stem>.txt`

지원 입력은 현재 `wav`, `mp3`, `flac`, `ogg`다.

### 5.3 Summary 산출물

canonical 경로:

- `db/<job_id>/summary/result.md`

legacy 호환:

- `summary/result.txt`
- `summary/<source_stem>.md`

기존 legacy 파일이 있으면 `app/artifacts.rs`가 canonical 경로 `summary/result.md`로 승격한다.

### 5.4 Embedding 산출물

- sidecar 파일: `db/<job_id>/summary/embedding.json`
- index metadata: `JobRecord.summary_embedding`

`SummaryEmbeddingRecord`는 `model_id`, `text_sha256`, `dimension`, `normalized`, `created_at`, `file_path`를 기록한다.

## 6. 파이프라인 단계

### 6.1 FFmpeg

흐름:

1. 입력 경로 canonicalize
2. 동일 `source_path` 완료 job이 있고 산출물이 유효하면 `Reused`
3. 동일 입력의 running job이 있으면 `Deduplicated`
4. 아니면 새 job 생성 후 `ffprobe`, `ffmpeg` 실행

특징:

- `ffprobe`로 채널 수와 레이아웃을 읽는다.
- `ffmpeg` 한 번으로 채널별 mono wav와 merged mono wav를 같이 생성한다.
- 실패 시 부분 산출물은 정리한다.

### 6.2 STT

흐름:

1. ffmpeg 완료 job만 허용
2. `audio_files` subset 또는 `mono_mix_only=true`를 해석
3. 대상 transcript가 이미 모두 있으면 `Reused`
4. task running이면 `Deduplicated`
5. 아니면 whisper 모델 준비 후 `whisper-cli` 실행

특징:

- 출력 완료 후 연속 중복 라인을 제거하는 후처리를 수행한다.
- 진행률 API는 task 상태와 현재 transcript 파일 개수를 조합해 계산한다.

### 6.3 Summary

흐름:

1. `stt/*.txt`를 읽어 프롬프트 파일 생성
2. llama summary 모델 준비
3. `summary/result.md` 생성
4. 성공 시 embedding task를 best-effort로 자동 연쇄 실행

프롬프트 정책:

- 한국어 Markdown 출력
- 첫 줄은 `## 요약`
- transcript 파일명을 본문에 나열하지 않음
- 실행 항목이 있으면 마지막에 bullet list

### 6.4 Embedding

흐름:

1. summary 파일 존재 확인
2. 기존 metadata와 sidecar를 기준으로 stale 여부 판단
3. stale이면 `llama-embedding`으로 벡터 생성
4. `summary/embedding.json` 저장
5. `JobRecord.summary_embedding` metadata 갱신

검색:

- query도 같은 embedding 모델로 즉시 임베딩
- corpus 전체를 brute-force cosine similarity로 스캔
- score 내림차순 정렬
- 기본 limit 10, 최대 50

## 7. 모델 준비와 외부 도구

### 7.1 FFmpeg

- 실행 파일 탐색: `.build/ffmpeg/<os>-<arch>/install/bin`
- 필요 명령: `ffmpeg`, `ffprobe`

### 7.2 Whisper

- 실행 파일 탐색: `.build/whisper/<os>-<arch>/bin/whisper-cli`
- 모델 환경변수: `RECORDROUTE_WHISPER_MODEL`
- 기본 모델 경로: `models/whisper/ggml-base.bin`

준비 방식:

- 로컬 파일이 있으면 그대로 사용
- 없으면 Rust가 직접 모델 파일을 확보한다
  - `RECORDROUTE_WHISPER_MODEL_SOURCE_DIR`가 있으면 해당 디렉터리에서 복사 시도
  - 아니면 `RECORDROUTE_WHISPER_MODEL_URL_TEMPLATE` 또는 기본 Hugging Face URL로 다운로드

### 7.3 Llama summary / embedding

- 실행 파일 탐색: `.build/llama/<os>-<arch>/bin`
  - `llama-cli`
  - `llama-embedding`
- summary 모델 환경변수: `RECORDROUTE_LLAMA_MODEL`
- embedding 모델 환경변수: `RECORDROUTE_LLAMA_EMBEDDING_MODEL`

기본 모델:

- summary: `ggml-org/gemma-3-4b-it-GGUF`
- embedding: `Qwen/Qwen3-Embedding-4B-GGUF`

모델 해석 규칙:

- 환경변수 값이 파일이면 local GGUF
- 아니면 Hugging Face repo 문자열
- Hugging Face repo면 캐시 경로는 `models/llama/hf/*.gguf`

`prepare-llama-model`과 `POST /models/llama/prepare`는 summary 모델과 embedding 모델을 함께 준비하는 umbrella 동작이다.

## 8. HTTP 서버와 웹 UI

서버 바인드 주소는 `127.0.0.1:38080`다.

정적 UI:

- `GET /`
- `GET /app.js`
- `GET /app.css`

상태/모델:

- `POST /server/ping`
- `GET /system/status`
- `GET /models/status`
- `POST /models/whisper/prepare`
- `POST /models/llama/prepare`

job:

- `POST /jobs`
- `POST /jobs/upload`
- `GET /jobs`
- `GET /jobs/completed`
- `GET /jobs/by-source`
- `GET /jobs/{job_id}`
- `GET /jobs/{job_id}/status`

stage / artifacts:

- `POST/GET /jobs/{job_id}/stt`
- `GET /jobs/{job_id}/stt/progress`
- `GET /jobs/{job_id}/stt/texts`
- `GET /jobs/{job_id}/stt/texts/{transcript_id}`
- `POST/GET /jobs/{job_id}/summary`
- `GET /jobs/{job_id}/summary/text`
- `POST/GET /jobs/{job_id}/summary/embedding`
- `POST /summary/search`
- `GET /jobs/{job_id}/files`
- `GET /jobs/{job_id}/files/{*file_name}`

긴 작업 실행은 모두 `tokio::task::spawn_blocking`으로 백그라운드에 넘긴다.
`POST /jobs/upload`는 multipart를 스트리밍 저장하며 업로드 파일 제한은 512 MiB다.

## 9. 현재 아키텍처의 운영상 특징

- CLI와 HTTP API는 같은 `app/*`, `index/*` 도메인 로직을 공유한다.
- 재사용과 중복방지는 각 stage submit 단계에서 먼저 판정한다.
- summary 파일명은 canonical path로 통일했고, legacy 이름은 읽는 시점에 승격한다.
- embedding 검색은 파일 기반 sidecar + brute-force scan 구조이며 외부 vector DB는 사용하지 않는다.
- query embedding 캐시는 아직 없다.
- OpenAPI는 수동 유지 문서이므로, 라우트나 응답 타입이 바뀌면 같이 갱신해야 한다.
