# AGENTS.md

이 문서는 RecordRoute 저장소에서 작업하는 에이전트 공통 가이드입니다.

## 1) 프로젝트 개요
- 핵심 애플리케이션은 `rust/` 크레이트(`recordroute_rust`)입니다.
- 오디오 파이프라인은 다음 순서로 동작합니다.
  1. FFmpeg로 채널 분리/모노 믹스 생성
  2. Whisper로 STT 생성
  3. Llama로 요약 생성
- 상태/결과 저장소는 `db/index.json` 기반 인덱스입니다.

## 2) 주요 코드 위치
- CLI 엔트리: `rust/src/main.rs`
- CLI/작업 오케스트레이션: `rust/src/app.rs`
- HTTP 서버(axum): `rust/src/server.rs`
- 인덱스/작업 상태 저장: `rust/src/index.rs`
- FFmpeg 래퍼: `rust/src/ffmpeg.rs`
- Whisper 래퍼: `rust/src/whisper.rs`
- Llama 래퍼: `rust/src/llama.rs`
- 아키텍처 문서: `docs/architecture.md`
- API 설계 TODO: `docs/API_TODO.md`

## 3) 실행/개발 기본 명령
- 서버 실행(루트):
  - `./run.sh`
- Rust 개발 실행:
  - `cargo run --manifest-path rust/Cargo.toml -- <mode>`
  - mode 예시: `ffmpeg <input>`, `stt`, `summary`, `prepare-llama-model`, `server`
- 테스트:
  - `cargo test --manifest-path rust/Cargo.toml`
- 전체 초기 빌드(의존 툴체인 포함):
  - `./setup.sh`

## 4) 작업 모델 이해
- Job 상태(`JobStatus`): `running`, `completed`, `failed`
- Task 타입(`TaskType`): `ffmpeg`, `stt`, `summary`
- 중복 요청 처리:
  - 동일 입력의 완료 결과가 있으면 재사용(reused)
  - 동일 입력의 실행중 작업이 있으면 deduplicate

## 5) 구현 원칙
- FFmpeg/Whisper/Llama는 Rust에서 라이브러리 직접 링크가 아니라 CLI 실행 래핑 모델이다.
- `db/index.json`은 단일 진실 공급원(SoT)으로 취급한다.
- API와 CLI가 동일한 도메인 로직(`app.rs`, `index.rs`)을 공유하도록 유지한다.
- 변경 시 재사용/중복방지 로직이 깨지지 않게 확인한다.

## 6) 에이전트 문서 규칙
- 에이전트 전용 추가 문서는 이 파일을 기준 문서로 참조한다.
- `CLAUDE.md`, `GEMINI.md`에는 중복 설명을 최소화하고 본 문서 링크/요약만 둔다.
- 공통 정책 변경은 우선 `AGENTS.md`에 반영 후, 다른 에이전트 문서는 참조 링크만 갱신한다.
