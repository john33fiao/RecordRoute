# RecordRoute Rust 아키텍처 기준서

이 문서는 Rust/C++ 백엔드 전환의 **단일 진입점/엔진 경계**를 고정하기 위한 최소 아키텍처 기준을 정의한다.
상세 운영/단계 계획은 `docs/rust-cpp-backend-rewrite-plan.md`, WBS 추적은 `TODO/TODO.md`를 따른다.

## 1. 단일 진입점 원칙

- 외부 클라이언트 트래픽은 **Rust API 서버(`:18000`)**로만 유입한다.
- Rust 계층이 다음 책임을 전담한다.
  - 요청 인증/검증
  - 큐 라우팅(`stt/summarize/embed`)
  - 작업 상태 관리(`queued|running|completed|failed|timeout|canceled|rejected`)
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
- 큐 소비는 엔진별 **고정 worker N개**로 수행하며, worker 수가 동시성 상한이다.
- `recv -> spawn -> permit 대기` 형태를 피해서, permit 대기 태스크가 무한 누적되어 backpressure가 무력화되는 상황을 방지한다.

### 주요 환경변수

- `RECORDROUTE_STT_QUEUE_CAPACITY`, `RECORDROUTE_SUMMARIZE_QUEUE_CAPACITY`, `RECORDROUTE_EMBED_QUEUE_CAPACITY`
- `RECORDROUTE_STT_CONCURRENCY`, `RECORDROUTE_SUMMARIZE_CONCURRENCY`, `RECORDROUTE_EMBED_CONCURRENCY`
- `RECORDROUTE_ENGINE_TIMEOUT_SECS`, `RECORDROUTE_ENGINE_RETRY_COUNT`, `RECORDROUTE_ENGINE_BACKOFF_MS`
- `RECORDROUTE_ENGINE_CONNECT_TIMEOUT_MS`
- `RECORDROUTE_STT_ENGINE_URL`, `RECORDROUTE_SUMMARIZE_ENGINE_URL`, `RECORDROUTE_EMBED_ENGINE_URL`

## 4. 잡 상태 머신

- OpenAPI 기준 상태 enum(고정): `queued|running|completed|failed|timeout|canceled|rejected`
- 기본 전이(OpenAPI 기준): `queued -> running -> completed|failed|timeout|canceled`
- 예외 전이(OpenAPI 기준): `queued -> rejected` (enqueue 거부 시)
- OpenAPI 기준 에러 코드 enum(고정): `queue_full|engine_full|engine_dispatcher_closed|engine_dispatcher_unavailable|engine_connect_timeout|engine_request_timeout|engine_transport_error|engine_upstream_4xx|engine_upstream_5xx|engine_retry_exhausted|engine_endpoint_invalid|engine_invalid_http|engine_invalid_json|engine_not_configured|job_timeout|job_canceled|invalid_job_id|job_not_found|not_found|method_not_allowed`
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
- 상태/오류 계약은 OpenAPI enum 기준으로 관리한다.
  - 상태: `queued|running|completed|failed|timeout|canceled|rejected`
  - 에러 코드: `queue_full|engine_full|engine_dispatcher_closed|engine_dispatcher_unavailable|engine_connect_timeout|engine_request_timeout|engine_transport_error|engine_upstream_4xx|engine_upstream_5xx|engine_retry_exhausted|engine_endpoint_invalid|engine_invalid_http|engine_invalid_json|engine_not_configured|job_timeout|job_canceled|invalid_job_id|job_not_found|not_found|method_not_allowed`

### 목표 상태
- Rust API(`:18000`) + 엔진 3종(`18101~18103`) + Swagger(`:14000`) 분리 운영
- 엔진 프로세스 슈퍼비전/헬스체크/재시작 정책 고도화
- `symphonia` 전처리 및 `/process` 전체 워크플로우 전환
- 상태 전이/에러 코드는 OpenAPI enum 기준(`queued|running|completed|failed|timeout|canceled|rejected` 및 ErrorCode enum)으로 문서·구현·운영 가이드를 고정 유지

## 7. 로컬 실행 및 테스트

```bash
cargo run
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

엔진 mock이 필요한 통합 시나리오는 `src/main.rs` 테스트의 `MockEngineClient`로 검증한다.

## 8. 관측성 기준(운영)

- `queue_depth{engine}`: accepted enqueue 기준의 **근사값**. RAII guard Drop으로 감소 정합성 보장.
- `in_flight{engine}`: 실제 처리 중 작업 수(최대값이 worker 수를 넘지 않아야 함).
- `jobs_total{engine,status}`: `completed|failed|timeout|canceled|rejected` 누적 카운트.
- `engine_timeout_total{engine}`: connect/read/write timeout 누적 카운트.
- `engine_latency_ms{engine}`: 가능하면 p50/p95/p99 추적.

## 9. 간단 Runbook

- `queue_full(429)` 급증: engine별 queue depth와 in-flight(worker 포화 여부) 확인 → queue capacity/concurrency 조정.
- `engine_timeout` 급증: 엔진 프로세스 헬스/네트워크/포트 확인 → `RECORDROUTE_ENGINE_CONNECT_TIMEOUT_MS`, `RECORDROUTE_ENGINE_TIMEOUT_SECS` 점검.
- `failed` 급증(5xx): 엔진 로그와 재시도 소진 여부 확인 → 엔진 재시작 또는 임시 트래픽 완화.
- 롤백 원칙: 최근 설정 변경(동시성/타임아웃/엔드포인트)부터 원복하고, API 계약은 유지한다.

## 10. 추적 링크

- 상세 설계: `docs/rust-cpp-backend-rewrite-plan.md`
- 실행 WBS: `TODO/TODO.md`
