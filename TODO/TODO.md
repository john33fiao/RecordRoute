# Rust/C++ 백엔드 재작성 WBS (Work Breakdown Structure)

이 문서는 **현재 코드베이스 기준 실제 진행 상태**를 반영한 실행 추적표입니다.
세부 설계는 `docs/rust-cpp-backend-rewrite-plan.md`를 단일 기준으로 따릅니다.

## 작업 로그

- 2026-02-23: Phase E-1 길이/예산 기반 job timeout 산정식 1차 반영
  - `POST /jobs?engine=stt&audio_ms=<ms>` 입력 시 `job_timeout = clamp((audio_ms * per_audio_sec_ms / 1000) + buffer_ms, min, max)` 산식으로 timeout budget 계산
  - timeout 파라미터는 환경변수(`RECORDROUTE_JOB_TIMEOUT_MIN_SECS`, `RECORDROUTE_JOB_TIMEOUT_MAX_SECS`, `RECORDROUTE_STT_TIMEOUT_PER_AUDIO_SEC_MS`, `RECORDROUTE_STT_TIMEOUT_BUFFER_MS`)로 제어
  - 워커가 요청별 timeout budget을 사용하도록 조정하고 기존 timeout 회귀 테스트 통과 확인

- 2026-02-24: WBS 7.2 모델 manifest 정책 및 `.gitignore` 운영 검증 반영
  - `.gitignore`를 `vendor` 소스 추적 + 빌드 산출물 제외 정책으로 정렬
  - `models/**` raw 데이터 제외 + `manifest.yml|yaml|json` 메타데이터 추적 예외 규칙 반영
  - `docs/deployment-asset-policy.md` 체크포인트(정책 반영/manifest 추적) 완료 처리

- 2026-02-24: 운영 점검 시나리오(장애/복구/부하) runbook 문서화
  - `docs/operations-runbook-scenarios.md`에 장애 주입/복구 판정/부하(429 reason) 점검 절차 및 롤백 기준 추가
  - WBS 7.3 항목 완료 처리

- 2026-02-23: Phase E-1 whisper-server 추론 책임 한정(변환 책임 제거) 반영
  - STT 엔진 payload에 `audio_contract`(normalized_by/format/conversion_required=false)를 명시해 변환 책임이 Rust에 있음을 고정
  - whisper-server는 변환 없이 추론 전용 경로를 사용한다는 계약을 테스트로 검증

- 2026-02-23: Phase C-1 degraded readiness/운영 메트릭 노출 반영
  - `/readyz`가 용량 리젝션/디스패처 종료 시 `degraded` 상태를 503으로 반환하도록 조정
  - `/metrics` 엔드포인트에 readiness(ready/degraded), 엔진별 queue/running, 리젝션(engine/reason) 스냅샷 추가
  - 라우팅 단위 테스트(`readyz degraded`, `metrics`) 추가 및 회귀 검증

- 2026-02-23: Phase B-2 429 사유 분리/리젝션 메트릭 라벨 분리 반영
  - `POST /jobs` enqueue 실패(Full) 시 `engine_full`/`queue_full` reason을 worker 포화 기준으로 분리
  - 엔진/사유(`engine`, `reason`) 단위 리젝션 카운터를 오케스트레이터 메모리 지표로 추가
  - 관련 단위 테스트(사유 분기, 리젝션 카운트) 갱신 및 통과

- 2026-02-23: OpenAPI/API 계약 Rust 목표 엔드포인트 정렬 상태 확인 및 WBS 반영
  - `docs/openapi.yaml`, `docs/swagger/openapi.yaml` 기준 엔드포인트가 `/healthz`, `/readyz`, `POST /jobs`, `GET /jobs/{job_id}`로 정렬됨을 재검증
  - 잡 상태/에러 코드(enum)가 Rust 오케스트레이터 구현 계약(`queued|running|completed|failed|timeout|canceled|rejected`, `invalid_job_id`, `queue_full|engine_full` 등)과 일치함을 확인
  - WBS `1.2 OpenAPI/API 계약 재정렬` 항목 완료 처리
- 2026-02-22: Phase B-1 엔진별 큐 수용량/배압(429) 스켈레톤 도입
  - `POST /jobs?engine=<stt|summarize|embed>` 라우팅 추가(기본값 `stt`)
  - 엔진별 bounded capacity 기반 큐 포화 시 `429 + queue_full|engine_full` 반환
  - 엔진별 큐 포화가 다른 큐 접수에는 영향을 주지 않는 단위 테스트 추가
- 2026-02-22: Phase A+ 잡 API 스켈레톤 도입
  - `POST /jobs`에서 `202 + job_id` 반환 구현
  - `GET /jobs/{id}` 인메모리 조회(초기 상태 `queued`) 구현
  - 향후 엔진별 큐/실행기 연동 전까지 오케스트레이터 스켈레톤 상태 유지
- 2026-02-22: Phase A 최소 HTTP 서버 도입
  - `tokio::net::TcpListener` 기반 API 서버 바인딩(`:18000` 기본값) 구현
  - `/healthz`, `/readyz` 엔드포인트 및 readiness 상태 코드 분리 반영
  - 환경변수(`RECORDROUTE_API_HOST`, `RECORDROUTE_API_PORT`) 기반 최소 설정 로더 도입
- 2026-02-22: Rust 오케스트레이터 스캐폴딩 반영 확인
  - 루트 `Cargo.toml` + `src/main.rs` 존재
  - `tokio` 런타임/`tracing` 초기화 및 bootstrap 로그 출력 구현
  - 아직 HTTP API 라우팅, 엔진 오케스트레이션, 큐/슈퍼비전은 미구현
- 2026-02-22: 전환 문서 동기화
  - `TODO/TODO.md` 상태를 코드베이스 기준으로 재정렬
  - `docs/rust-cpp-backend-rewrite-plan.md`를 "현재 상태/다음 단계" 중심으로 갱신

## 현재 구현 스냅샷 (코드 기준)

- [x] Rust 실행 바이너리 스캐폴딩 (`recordroute-orchestrator`)
- [x] 비동기 런타임 초기화 (`tokio`)
- [x] 기본 로깅/필터 초기화 (`tracing`, `tracing-subscriber`)
- [x] API 서버 바인딩 (`:18000`)
- [x] 작업 API (`POST /jobs`, `GET /jobs/{id}`)
- [x] 헬스 엔드포인트 (`/healthz`, `/readyz`)
- [x] 엔진별 큐 (`stt/summarize/embed`) 수용량/배압 스켈레톤
- [x] 엔진 프로세스 슈퍼비전 (spawn/health/restart/shutdown)
- [x] Rust `symphonia` 전처리 파이프라인
- [x] Swagger 분리 배포 (`:14000`)

## WBS 진행 현황

## 1.0 아키텍처/계약 정합성

- [x] 1.1 Rust 단일 진입점/엔진 경계 문서 기준 확정
  - 근거 문서: `docs/architecture.md`, `docs/rust-cpp-backend-rewrite-plan.md`
- [x] 1.2 OpenAPI/API 계약을 Rust 목표 엔드포인트 기준으로 재정렬
  - 근거 문서: `docs/openapi.yaml`, `docs/swagger/openapi.yaml`

## 2.0 런타임 스캐폴딩

- [x] 2.1 Rust 실행 진입점 및 로깅 부트스트랩 구현
- [x] 2.2 HTTP 서버 최소 구현(`/healthz`/`/readyz`)
- [x] 2.3 설정 로더(포트/타임아웃/큐 크기) 도입 (포트/호스트 최소값)

## 3.0 엔진 통합 기반

- [x] 3.1 엔진별 클라이언트/포트 설정 (`18101`, `18102`, `18103`)
- [x] 3.2 엔진별 bounded queue + semaphore
- [x] 3.3 큐 포화/엔진 포화 `429` 규약 및 메트릭 라벨 분리

## 4.0 잡 모델/오류 계약

- [x] 4.1 잡 상태 전이 모델 (`queued` 초기 상태 + 조회 스켈레톤)
- [x] 4.1-확장 잡 상태 전이 전체 모델 (`running|completed|failed|timeout|canceled`)
- [x] 4.2 타임아웃 계층 분리 (HTTP vs Job)
- [x] 4.3 에러 코드/응답 필드 계약 고정

## 5.0 오디오 전처리/처리량 정책

- [x] 5.1 `symphonia` 기반 오디오 정규화 (16kHz/16-bit mono WAV)
- [x] 5.2 길이/예산 계산 기반 timeout 산정식 적용
- [x] 5.3 whisper-server 추론 책임 한정(변환 책임 제거)

## 6.0 슈퍼비전/운영 안정성

- [x] 6.1 child 생명주기 감시 + backoff 재시작
- [x] 6.2 graceful shutdown + 강제 종료 fallback
- [x] 6.3 degraded 상태/관측성 메트릭 반영

## 7.0 배포/문서 분리

- [x] 7.1 API(18000) / Swagger(14000) 분리 배포 구성
- [x] 7.2 모델 manifest 정책 및 `.gitignore` 운영 검증
- [x] 7.3 운영 점검 시나리오(장애/복구/부하) 문서화
  - 근거 문서: `docs/operations-runbook-scenarios.md`

## 다음 우선순위 (실행 단위)

1. 운영 점검 시나리오 기반 장애훈련(정기) 및 결과 누적
