# RecordRoute Rust 아키텍처 기준서

이 문서는 Rust/C++ 백엔드 전환의 **단일 진입점/엔진 경계**를 고정하기 위한 최소 아키텍처 기준을 정의한다.
상세 운영/단계 계획은 `docs/rust-cpp-backend-rewrite-plan.md`, WBS 추적은 `TODO/TODO.md`를 따른다.

## 1. 단일 진입점 원칙

- 외부 클라이언트 트래픽은 **Rust API 서버(`:18000`)**로만 유입한다.
- Rust 계층이 다음 책임을 전담한다.
  - 요청 인증/검증
  - 큐 라우팅(`stt/summarize/embed`)
  - 작업 상태 관리(`queued|running|completed|failed|timeout|canceled`)
  - 타임아웃/재시도/배압 정책
  - 엔진 헬스체크 및 슈퍼비전
- Swagger UI는 `:14000`에서 별도 프로세스로 운영하며, API 가용성과 분리한다.

## 2. 엔진 실행 경계

- `llama-text`, `llama-embed`, `whisper-server`는 **독립 프로세스**로 실행한다.
- Rust↔엔진 통신은 FFI가 아닌 **내부 HTTP 계약**을 사용한다.
- 엔진 바인딩은 `127.0.0.1`로 고정한다(직접 외부 노출 금지).

### 엔진 포트 고정

- `127.0.0.1:18101` — `llama-text` (요약/생성)
- `127.0.0.1:18102` — `llama-embed` (임베딩)
- `127.0.0.1:18103` — `whisper-server` (STT)

## 3. 큐/동시성 분리 원칙

- 단일 글로벌 큐를 사용하지 않는다.
- 엔진별 bounded queue를 분리한다.
  - `stt_queue`
  - `summarize_queue`
  - `embed_queue`
- 큐 포화와 엔진 포화를 분리해 `429` 및 메트릭 라벨을 구분한다.

## 4. 오디오 전처리 기준

- 기본값은 Rust `symphonia` 기반 전처리(16kHz, 16-bit mono WAV)다.
- 외부 `ffmpeg`/`ffprobe` 의존은 기본 경로에서 제외한다.
- `whisper-server`는 추론에 집중하고 변환 책임은 Rust에 둔다.

## 5. 현재 상태 vs 목표 상태

### 현재 상태
- 저장소 루트에 Rust 실행 코드(`Cargo.toml`, `/src`)는 아직 없다.
- 프론트엔드/레거시 코드가 운영 기준선으로 남아 있다.

### 목표 상태
- Rust API(`:18000`) + 엔진 3종(`18101~18103`) + Swagger(`:14000`) 분리 운영
- 엔진별 큐/동시성/슈퍼비전/타임아웃 정책이 Rust에 내재화된 상태

## 6. 추적 링크

- 상세 설계: `docs/rust-cpp-backend-rewrite-plan.md`
- 실행 WBS: `TODO/TODO.md`
