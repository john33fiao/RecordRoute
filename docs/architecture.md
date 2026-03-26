# RecordRoute 아키텍처 (현재 코드 기준)

이 문서는 `rust/src/*.rs` 구현을 기준으로 현재 RecordRoute 동작 구조를 요약합니다.

## 1. 런타임 구성

- 실행 바이너리 엔트리: `rust/src/main.rs`
  - `recordroute_rust::main_cli()` 호출 후 에러 시 `stderr` 출력/exit(1)
- 실제 애플리케이션 오케스트레이션: `rust/src/app.rs`
- HTTP 서버: `rust/src/server.rs` (axum)
- 영속 상태 저장(SoT): `db/index.json` (`rust/src/index.rs`)

핵심 원칙:
- FFmpeg/Whisper/Llama 모두 Rust 라이브러리 링크가 아니라 **로컬 CLI 실행 래퍼** 방식
- API/CLI는 동일한 도메인 함수(`app.rs`, `index.rs`)를 공유
- job/task/model preparation 상태는 모두 `db/index.json`에 저장한다.

## 2. CLI 모드

`main_cli()`는 다음 모드를 지원합니다.

- `ffmpeg <input>` 또는 `<input>`: 채널 분리 + 모노 믹스 job
- `stt`: 완료된 ffmpeg job의 wav를 대상으로 STT 실행
- `summary`: STT 텍스트 기반 요약 생성
- `prepare-llama-model`: llama 모델 캐시 사전 준비
- `server`: HTTP API 서버 실행 (`127.0.0.1:38080`)

인자 없이 실행하면 모드 선택 프롬프트를 제공합니다.

## 3. 데이터 모델 및 상태 저장

`db/index.json` 구조의 핵심 타입은 다음과 같습니다.

- `version`: 현재 저장 포맷 버전은 `2`
- `jobs`: `JobRecord` 목록
- `model_preparations`
  - `whisper`: `ModelPreparationRecord`
  - `llama`: `ModelPreparationRecord`

주요 타입:
- `JobRecord`
  - `job_id`, `status`, `started_at`, `finished_at`
  - `source_path`, `source_file_name`, `job_dir`
  - `probe`, `outputs`, `error_message`
  - `tasks` (`TaskRecord` 목록)
- `JobStatus`: `running | completed | failed`
- `TaskType`: `ffmpeg | stt | summary`
- `TaskStatus`: `running | completed | failed`
- `ModelKind`: `whisper | llama`
- `ModelPreparationStatus`: `idle | running | completed | failed`
- `ModelPreparationRecord`
  - `status`, `started_at`, `finished_at`, `heartbeat_at`, `last_error`

`IndexStore`는 `db/index.lock` 파일 락 기반으로 읽기/쓰기 동기화를 수행합니다.
기존 `model_preparations` 필드가 없는 구버전 인덱스도 `serde(default)`로 읽을 수 있습니다.

## 4. 파이프라인 단계

### 4.1 FFmpeg 단계

1. `submit_ffmpeg_job()`
   - 입력 경로 canonicalize
   - 동일 `source_path`의 최근 완료 job + 실제 산출물 파일 존재 시 `Reused`
   - 동일 입력 `running` job 존재 시 `Deduplicated`
   - 둘 다 아니면 새 job 생성(`Submitted`)
2. `execute_ffmpeg_job()`
   - `ffprobe`로 채널 정보 수집
   - `ffmpeg` 단일 실행으로 `channel_XX.wav` + `mono_mix.wav` 생성
   - 성공 시 job completed, 실패 시 partial 출력 정리 + failed

출력 위치:
- `db/<job_id>/channel_01.wav` ...
- `db/<job_id>/mono_mix.wav`

### 4.2 STT 단계

1. `submit_stt_job()`
   - ffmpeg 완료 job만 허용
   - 기존 STT task가 running이면 `Deduplicated`
   - 대상 wav들의 transcript가 모두 존재하면 `Reused`
   - 아니면 STT task를 running으로 upsert하고 `Submitted`
2. `execute_stt_job()`
   - whisper 준비 상태를 `ensure_model_prepared(..., ModelKind::Whisper)`로 확인
   - 각 대상 wav에 대해 `stt/<stem>.txt` 생성
   - 성공 시 STT task completed

선택 실행:
- 전체 wav 대상
- `audio_files` 부분집합 대상
- `mono_mix_only=true` 단일 대상

### 4.3 Summary 단계

1. `submit_summary_job()`
   - summary task running이면 `Deduplicated`
   - `force_regenerate=false` + 기존 summary 파일 존재 시 `Reused`
   - 아니면 summary task running으로 upsert 후 `Submitted`
2. `execute_summary_job()`
   - llama 준비 상태를 `ensure_model_prepared(..., ModelKind::Llama)`로 확인
   - `stt/*.txt` 모아 프롬프트 파일 생성
   - llama 실행 후 결과 텍스트 저장
   - 임시 프롬프트 파일 정리
   - 성공 시 Summary task completed

출력 위치:
- `db/<job_id>/summary/result.txt`

## 5. 모델 준비 오케스트레이션

모델 준비는 `app.rs`의 공통 헬퍼로 통합되어 있습니다.

- `submit_model_preparation()`
  - 모델이 이미 준비돼 있으면 `AlreadyReady`
  - 같은 모델 준비가 진행 중이고 heartbeat가 살아 있으면 `Deduplicated`
  - stale 실행이면 `failed`로 정리 후 새 준비를 `Submitted`
- `execute_model_preparation()`
  - 실제 다운로드/준비 수행
  - 실행 중 `heartbeat_at`를 10초 주기로 갱신
  - 성공 시 `completed`, 실패 시 `failed`와 `last_error` 기록
- `wait_for_model_preparation()`
  - blocking 경로(STT/Summary/CLI)에서 기존 실행 중 preparation이 끝날 때까지 대기
- `ensure_model_prepared()`
  - submit + execute/wait를 묶어 즉시 모델이 필요한 경로에서 사용

동작 규칙:
- whisper와 llama는 서로 독립적으로 동시에 준비 가능
- `running` 상태의 heartbeat가 60초 이상 갱신되지 않으면 stale로 간주
- HTTP API, CLI(`prepare-llama-model`), STT/Summary 런타임 자동 준비가 같은 상태 저장과 deduplicate 규칙을 공유

## 6. 외부 툴체인 통합

### 6.1 FFmpeg

- 탐색 경로: `.build/ffmpeg/<os>-<arch>/install/bin`
- 필요 명령: `ffmpeg`, `ffprobe`
- 빌드 스크립트 안내: `scripts/build_ffmpeg.{sh|bat}`

### 6.2 Whisper

- 탐색 경로: `.build/whisper/<os>-<arch>/bin/whisper-cli`
- 모델 환경변수: `RECORDROUTE_WHISPER_MODEL`
- 기본 모델 경로: `models/whisper/ggml-base.bin`
- 모델 부재 시 다운로드 스크립트 실행: `scripts/download_whisper_model.{sh|bat}`
- 특정 백엔드 실패 시(macOS/Windows) CPU fallback 재시도

### 6.3 Llama

- 탐색 경로: `.build/llama/<os>-<arch>/bin/llama-cli`
- 모델 환경변수: `RECORDROUTE_LLAMA_MODEL`
  - 파일 경로면 local model 사용
  - 파일이 아니면 Hugging Face repo 문자열로 간주
- 기본 소스: `ggml-org/gemma-3-4b-it-GGUF`
- 캐시 경로: `models/llama/hf/...`
- 백엔드 실패 시(macOS/Windows) CPU fallback 재시도

## 7. HTTP API 레이어

`server.rs`는 `app.rs` 함수를 호출해 비동기 제출/조회 API를 제공합니다.

주요 엔드포인트:
- `POST /jobs`: ffmpeg job 제출(재사용/중복제거 포함)
- `POST /jobs/{job_id}/stt`: STT task 제출
- `POST /jobs/{job_id}/summary`: summary task 제출
- `GET /jobs`, `GET /jobs/{job_id}`, `GET /jobs/{job_id}/status`
- `GET /jobs/{job_id}/stt/texts`, `GET /jobs/{job_id}/summary/text`
- `GET /system/status`: coarse-grained toolchain/model 가용성 점검
- `GET /models/status`: 모델별 준비 상세 상태 조회
- `POST /models/whisper/prepare`: whisper 모델 준비 시작 또는 deduplicate
- `POST /models/llama/prepare`: llama 모델 준비 시작 또는 deduplicate

실행이 필요한 작업은 `tokio::task::spawn_blocking`으로 백그라운드 수행됩니다.

모델 준비 API 응답 규칙:
- 이미 준비된 모델이면 `200 OK` + `already_ready=true`
- 새 준비 시작 또는 실행 중 준비 합류면 `202 Accepted`
- 툴체인 탐지 실패나 잘못된 모델 설정으로 시작 자체가 불가능하면 `503`

## 8. 재사용/중복제거 규칙 요약

- FFmpeg: 입력 canonical path 기준
  - 완료 + 파일 유효 -> 재사용
  - running 존재 -> 중복제거
- STT:
  - task running -> 중복제거
  - 요청 대상 transcript 모두 존재 -> 재사용
- Summary:
  - task running -> 중복제거
  - 결과 파일 존재 + 강제 재생성 아님 -> 재사용
- Model preparation:
  - 모델 파일/캐시가 이미 준비됨 -> 재사용 성격의 `AlreadyReady`
  - same model running + heartbeat 정상 -> 중복제거
  - stale running -> 실패로 전환 후 새 실행 허용

이 규칙 덕분에 동일 요청 반복 시 불필요한 재연산을 줄이고, API/CLI 간 일관된 동작을 유지합니다.
