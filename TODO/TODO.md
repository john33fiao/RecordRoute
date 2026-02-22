# Rust/C++ 백엔드 재작성 WBS (Work Breakdown Structure)

이 문서는 **현재 코드베이스 기준 실제 진행 상태**를 반영한 실행 추적표입니다.
세부 설계는 `docs/rust-cpp-backend-rewrite-plan.md`를 단일 기준으로 따릅니다.

## 작업 로그

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
- [ ] 작업 API (`POST /jobs`, `GET /jobs/{id}`)
- [x] 헬스 엔드포인트 (`/healthz`, `/readyz`)
- [ ] 엔진별 큐 (`stt/summarize/embed`)
- [ ] 엔진 프로세스 슈퍼비전 (spawn/health/restart/shutdown)
- [ ] Rust `symphonia` 전처리 파이프라인
- [ ] Swagger 분리 배포 (`:14000`)

## WBS 진행 현황

## 1.0 아키텍처/계약 정합성

- [x] 1.1 Rust 단일 진입점/엔진 경계 문서 기준 확정
  - 근거 문서: `docs/architecture.md`, `docs/rust-cpp-backend-rewrite-plan.md`
- [ ] 1.2 OpenAPI/API 계약을 Rust 목표 엔드포인트 기준으로 재정렬
  - 비고: 현재 `docs/openapi.yaml`은 레거시 경로 중심

## 2.0 런타임 스캐폴딩

- [x] 2.1 Rust 실행 진입점 및 로깅 부트스트랩 구현
- [x] 2.2 HTTP 서버 최소 구현(`/healthz`/`/readyz`)
- [x] 2.3 설정 로더(포트/타임아웃/큐 크기) 도입 (포트/호스트 최소값)

## 3.0 엔진 통합 기반

- [ ] 3.1 엔진별 클라이언트/포트 설정 (`18101`, `18102`, `18103`)
- [ ] 3.2 엔진별 bounded queue + semaphore
- [ ] 3.3 큐 포화/엔진 포화 `429` 규약 및 메트릭 라벨 분리

## 4.0 잡 모델/오류 계약

- [ ] 4.1 잡 상태 전이 모델 (`queued|running|completed|failed|timeout|canceled`)
- [ ] 4.2 타임아웃 계층 분리 (HTTP vs Job)
- [ ] 4.3 에러 코드/응답 필드 계약 고정

## 5.0 오디오 전처리/처리량 정책

- [ ] 5.1 `symphonia` 기반 오디오 정규화 (16kHz/16-bit mono WAV)
- [ ] 5.2 길이/예산 계산 기반 timeout 산정식 적용
- [ ] 5.3 whisper-server 추론 책임 한정(변환 책임 제거)

## 6.0 슈퍼비전/운영 안정성

- [ ] 6.1 child 생명주기 감시 + backoff 재시작
- [ ] 6.2 graceful shutdown + 강제 종료 fallback
- [ ] 6.3 degraded 상태/관측성 메트릭 반영

## 7.0 배포/문서 분리

- [ ] 7.1 API(18000) / Swagger(14000) 분리 배포 구성
- [ ] 7.2 모델 manifest 정책 및 `.gitignore` 운영 검증
- [ ] 7.3 운영 점검 시나리오(장애/복구/부하) 문서화

## 다음 우선순위 (실행 단위)

1. 잡 엔티티 + 인메모리 저장소 + `POST /jobs`/`GET /jobs/{id}` 골격 구현
2. 엔진별 큐 추상화(`stt/summarize/embed`)와 기본 backpressure 규약(`429`) 확정
3. 엔진별 클라이언트/포트 설정(`18101`, `18102`, `18103`) + readiness 연동
