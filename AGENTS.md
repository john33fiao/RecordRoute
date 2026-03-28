# AGENTS.md

이 문서는 RecordRoute 저장소에서 작업하는 에이전트 공통 가이드입니다.

## 1) 프로젝트 개요
- 핵심 애플리케이션은 `rust/` 크레이트(`recordroute_rust`)입니다.
- 파이프라인 단계는 `ffmpeg -> stt -> summary -> embedding` 순서로 확장되었습니다.
- 상태/결과 저장소 SoT는 메타데이터 DB와 오디오 저장소입니다.
- 메타데이터 기본 저장소는 `db/index.sqlite3`이며, `StorageConfig`를 통해 PostgreSQL backend도 지원합니다.
- `db/index.json`은 legacy 입력으로만 남아 있고 현재 권위 저장소가 아니며, `db/index.lock`도 더 이상 사용하지 않습니다.

## 2) 주요 코드 위치
- 바이너리 엔트리: `rust/src/main.rs`
- CLI/작업 오케스트레이션: `rust/src/app.rs`, `rust/src/app/cli.rs`
- HTTP 서버(axum): `rust/src/server.rs`
- API 라우트: `rust/src/server/routes/*.rs`
- 인덱스/작업 상태 저장: `rust/src/index.rs`, `rust/src/index/types.rs`
- FFmpeg 래퍼: `rust/src/ffmpeg.rs`
- Whisper 래퍼: `rust/src/whisper.rs`
- Llama(요약/임베딩) 래퍼: `rust/src/llama.rs`
- 아키텍처 문서(최신 기준): `docs/architecture.md`
- OpenAPI 명세: `docs/openapi.yaml`
- 과거 API TODO 기록: `docs/deprecated/API_TODO.md`

## 3) 실행/개발 기본 명령
- 서버 실행(루트):
  - `./run.sh`
- Rust 실행:
  - `cargo run --manifest-path rust/Cargo.toml -- <mode>`
  - mode:
    - `ffmpeg <input>`
    - `stt`
    - `summary`
    - `prepare-models`
    - `prepare-llama-model`
    - `embed-summaries`
    - `search-summaries <query>`
    - `server`
- 테스트:
  - `cargo test --manifest-path rust/Cargo.toml`
- 전체 초기 빌드(의존 툴체인 포함):
  - `./setup.sh`

## 4) 작업 모델 이해
- Job 상태(`JobStatus`): `running`, `completed`, `failed`
- Task 타입(`TaskType`): `ffmpeg`, `stt`, `summary`, `embedding`
- Task 상태(`TaskStatus`): `running`, `completed`, `failed`
- 제출 결과 disposition:
  - `Submitted`: 실제 실행 필요
  - `Reused`: 기존 산출물 재사용
  - `Deduplicated`: 동일 작업 실행 중이라 합류

추가 데이터 모델 메모:
- `JobRecord`에는 `split_strategy`, `summary_embedding`이 포함됩니다.
- `TaskRecord`는 `task_id`(uuid), `retry_count`, `last_error`를 관리합니다.
- 인덱스 포맷 버전은 현재 `4`입니다.
- 모델 준비 상태는 `model_preparations.whisper|llama|llama_embedding`에 저장됩니다.

## 5) 구현 원칙
- FFmpeg/Whisper/Llama는 Rust 직접 링크가 아닌 **CLI 실행 래핑 모델**입니다.
- API와 CLI는 동일한 도메인 로직(`app.rs`, `index.rs`)을 공유해야 합니다.
- 변경 시 재사용/중복방지 규칙이 깨지지 않는지 우선 검증하세요.
- Job/Task/Model preparation 상태 전이는 인덱스 기록과 함께 원자적으로 다뤄야 합니다.

## 6) 환경 변수/모델 관련
- Whisper 모델: `RECORDROUTE_WHISPER_MODEL` (기본: `models/whisper/ggml-base.bin`)
- Llama 요약 모델/레포: `RECORDROUTE_LLAMA_MODEL`
  - 파일 경로면 로컬 모델로 사용
  - 아니면 Hugging Face repo 문자열로 해석
- Llama 임베딩 모델/레포: `RECORDROUTE_LLAMA_EMBEDDING_MODEL`
  - 파일 경로면 로컬 모델로 사용
  - 아니면 Hugging Face repo 문자열로 해석

## 7) API 작업 시 체크포인트
- 상태/모델 계열:
  - `/server/ping`
  - `/system/status`, `/models/status`
  - `/models/{whisper|llama}/prepare`
- 큐 계열:
  - `/queue`
- Job 계열:
  - `/jobs`, `/jobs/completed`, `/jobs/upload`, `/jobs/batch-process`
  - `/jobs/{job_id}`, `/jobs/{job_id}/status`
- 산출물/태스크 계열:
  - `/jobs/{job_id}/stt`, `/jobs/{job_id}/stt/progress`
  - `/jobs/{job_id}/stt/texts`, `/jobs/{job_id}/stt/texts/{transcript_id}`
  - `/jobs/{job_id}/summary`, `/jobs/{job_id}/summary/text`
  - `/jobs/{job_id}/summary/embedding`
  - `/summary/search`
  - `/jobs/{job_id}/files`, `/jobs/{job_id}/files/{*file_name}`

## 8) 에이전트 문서 규칙
- 코드 또는 문서 수정을 시작하기 전에는 `.agents/skills/rtd-before/SKILL.md`를 먼저 수행해 범위, DoD, 테스트 전략, 롤백 계획을 점검합니다.
- 코드 또는 문서 수정이 끝난 후에는 `.agents/skills/rtd-after/SKILL.md`를 수행해 목적 적합성, 회귀, 검증 근거, READY 여부를 점검합니다.
- 에이전트 전용 추가 문서는 이 파일을 기준 문서로 참조합니다.
- `CLAUDE.md`, `GEMINI.md`에는 중복 설명을 최소화하고 본 문서 링크/요약만 둡니다.
- 공통 정책 변경은 우선 `AGENTS.md`에 반영 후, 다른 에이전트 문서는 참조 링크만 갱신하세요.
