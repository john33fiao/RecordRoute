# AGENTS.md

이 문서는 RecordRoute 저장소에서 작업하는 에이전트 공통 가이드입니다.

## 1) 프로젝트 개요
- 핵심 애플리케이션은 `rust/` 크레이트(`recordroute_rust`)입니다.
- 파이프라인 단계는 `ffmpeg -> stt -> summary` 순서입니다.
- 상태/결과 저장소 SoT는 `db/index.json`이며, 락 파일(`db/index.lock`)을 통해 동기화됩니다.

## 2) 주요 코드 위치
- 바이너리 엔트리: `rust/src/main.rs`
- CLI/작업 오케스트레이션: `rust/src/app.rs`
- HTTP 서버(axum): `rust/src/server.rs`
- 인덱스/작업 상태 저장: `rust/src/index.rs`
- FFmpeg 래퍼: `rust/src/ffmpeg.rs`
- Whisper 래퍼: `rust/src/whisper.rs`
- Llama 래퍼: `rust/src/llama.rs`
- 아키텍처 문서(최신 기준): `docs/architecture.md`
- OpenAPI 명세: `docs/openapi.yaml`
- API 설계 TODO: `docs/API_TODO.md`

## 3) 실행/개발 기본 명령
- 서버 실행(루트):
  - `./run.sh`
- Rust 실행:
  - `cargo run --manifest-path rust/Cargo.toml -- <mode>`
  - mode: `ffmpeg <input>`, `stt`, `summary`, `prepare-llama-model`, `server`
- 테스트:
  - `cargo test --manifest-path rust/Cargo.toml`
- 전체 초기 빌드(의존 툴체인 포함):
  - `./setup.sh`

## 4) 작업 모델 이해
- Job 상태(`JobStatus`): `running`, `completed`, `failed`
- Task 타입(`TaskType`): `ffmpeg`, `stt`, `summary`
- Task 상태(`TaskStatus`): `running`, `completed`, `failed`
- 제출 결과 disposition:
  - `Submitted`: 실제 실행 필요
  - `Reused`: 기존 산출물 재사용
  - `Deduplicated`: 동일 작업 실행 중이라 합류

## 5) 구현 원칙
- FFmpeg/Whisper/Llama는 Rust 직접 링크가 아닌 **CLI 실행 래핑 모델**입니다.
- API와 CLI는 동일한 도메인 로직(`app.rs`, `index.rs`)을 공유해야 합니다.
- 변경 시 재사용/중복방지 규칙이 깨지지 않는지 우선 검증하세요.
- Job/Task 상태 전이는 인덱스 기록과 함께 원자적으로 다뤄야 합니다.

## 6) 환경 변수/모델 관련
- Whisper 모델: `RECORDROUTE_WHISPER_MODEL` (기본: `models/whisper/ggml-base.bin`)
- Llama 모델/레포: `RECORDROUTE_LLAMA_MODEL`
  - 파일 경로면 로컬 모델로 사용
  - 아니면 Hugging Face repo 문자열로 해석

## 7) 에이전트 문서 규칙
- 에이전트 전용 추가 문서는 이 파일을 기준 문서로 참조합니다.
- `CLAUDE.md`, `GEMINI.md`에는 중복 설명을 최소화하고 본 문서 링크/요약만 둡니다.
- 공통 정책 변경은 우선 `AGENTS.md`에 반영 후, 다른 에이전트 문서는 참조 링크만 갱신하세요.
