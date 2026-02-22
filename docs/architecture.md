# RecordRoute Rust 아키텍처 기준서

이 문서는 Rust/C++ 백엔드 전환의 **단일 진입점/엔진 경계**를 고정하기 위한 최소 아키텍처 기준을 정의한다.
상세 운영/단계 계획은 `docs/rust-cpp-backend-rewrite-plan.md`, WBS 추적은 `TODO/TODO.md`를 따른다.

## 1. 단일 진입점 원칙

- 외부 클라이언트 트래픽은 **Rust API 서버(`:18000`)**로만 유입한다.
- Rust 계층이 다음 책임을 전담한다.
  - 요청 인증/검증
  - 큐 라우팅(`stt/summarize/embed`)
  - 작업 상태 관리(`queued|running|succeeded|failed|rejected`)
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
- 큐 포화는 `try_send` 실패로 감지하고 `429(queue_full)`로 응답한다.
- 큐 소비(worker)와 엔진별 semaphore를 조합해 동시성 상한을 강제한다.

### 주요 환경변수

- `RECORDROUTE_STT_QUEUE_CAPACITY`, `RECORDROUTE_SUMMARIZE_QUEUE_CAPACITY`, `RECORDROUTE_EMBED_QUEUE_CAPACITY`
- `RECORDROUTE_STT_CONCURRENCY`, `RECORDROUTE_SUMMARIZE_CONCURRENCY`, `RECORDROUTE_EMBED_CONCURRENCY`
- `RECORDROUTE_ENGINE_TIMEOUT_SECS`, `RECORDROUTE_ENGINE_RETRY_COUNT`, `RECORDROUTE_ENGINE_BACKOFF_MS`
- `RECORDROUTE_STT_ENGINE_URL`, `RECORDROUTE_SUMMARIZE_ENGINE_URL`, `RECORDROUTE_EMBED_ENGINE_URL`

## 4. 잡 상태 머신

- 기본 전이: `queued -> running -> succeeded|failed`
- 예외 전이: `queued -> rejected` (`queue_full`)
- 상태 메타데이터:
  - `created_at_ms`
  - `started_at_ms`
  - `finished_at_ms`
  - `error.code`, `error.message`
  - `result` (성공 시)

## 5. 오디오 전처리 기준

- 기본값은 Rust `symphonia` 기반 전처리(16kHz, 16-bit mono WAV)다.
- 외부 `ffmpeg`/`ffprobe` 의존은 기본 경로에서 제외한다.
- `whisper-server`는 추론에 집중하고 변환 책임은 Rust에 둔다.

## 6. 현재 상태 vs 목표 상태

### 현재 상태
- 저장소 루트 Rust 서버는 `/healthz`, `/readyz`, `POST /jobs`, `GET /jobs/{id}`를 제공한다.
- 엔진별 bounded queue + worker + concurrency limit + 실패/배압 상태 전이까지 구현되어 있다.

### 목표 상태
- Rust API(`:18000`) + 엔진 3종(`18101~18103`) + Swagger(`:14000`) 분리 운영
- 엔진 프로세스 슈퍼비전/헬스체크/재시작 정책 고도화
- `symphonia` 전처리 및 `/process` 전체 워크플로우 전환

## 7. 로컬 실행 및 테스트

```bash
cargo run
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

엔진 mock이 필요한 통합 시나리오는 `src/main.rs` 테스트의 `MockEngineClient`로 검증한다.

## 8. 추적 링크

- 상세 설계: `docs/rust-cpp-backend-rewrite-plan.md`
- 실행 WBS: `TODO/TODO.md`
