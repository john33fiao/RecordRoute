# Rust/C++ 백엔드 재작성 WBS (Work Breakdown Structure)

이 문서는 **현재 코드베이스 기준 실제 진행 상태**를 반영한 실행 추적표입니다.
세부 설계는 `docs/rust-cpp-backend-rewrite-plan.md`를 단일 기준으로 따릅니다.

## 작업 로그


- 2026-04-01: WBS 9.3 패키징/보안 기준선 확정
  - `docs/tauri-packaging-security-baseline.md`: `tauri.conf.json` allowlist/CSP/파일·프레임 최소 권한 정책 고정
  - `RECORDROUTE_*` 환경변수 주입 경로, 모델 경로, 비밀 마스킹/비노출 전략 확정
  - `engine_manager` 기반 종료(`shutdown`)/장애 재시작/업그레이드 재진입 상태 정합성(`Booting/Ready/Degraded/Stopping/Stopped`) 명시


- 2026-02-27: WBS 9.4 설치/배포 자동화 통합 완료
  - `docs/tauri-install-deploy-unified-flow.md`: 설치 게이트(`--check`)→빌드/패키징→실행 검증→업데이트 단일 사용자 플로우 고정
  - `README.md`, `docs/deployment-asset-policy.md`에 빌드 산출물 포함/제외(`frontend/dist`, `target/release`, `target/**` 중간 산출물 제외) 정책 동기화
  - Tauri installer/업데이트 진입 전 `scripts/install_*.sh|bat --check` PASS를 필수 조건으로 명시
- 2026-02-25: WBS 8.0 설치/실행 자동화 스크립트 완료(READY)
  - `scripts/install_windows.bat`, `scripts/install_unix.sh`: 기본 모델 env 검증, 누락 시 중단, 모델 파일 확인/미존재 시 중단 또는 pull 선택지 제공
  - 설치 단계 자동화: 프론트 의존성 설치 + 프론트 빌드 + Rust release 빌드 통합
  - `scripts/run_windows.bat`, `scripts/run_unix.sh`: Rust 서버와 프론트 개발 서버 동시 기동 스크립트 제공

- 2026-02-25: Tauri 전환 상태 점검(현황 재검증)
  - 완료 확인: `frontend/src/runtime/endpoints.ts`에서 `resolveApiBaseUrl`/`resolveWebSocketUrl` + `VITE_TAURI_BACKEND_URL` fallback이 구현되어 9.1 선행 과제 1건만 완료
  - 미완료 확인: Tauri lifecycle 기동/종료(오케스트레이터/Swagger), 다중 OS 포트 충돌 검증, 로그 수집 경로 정렬은 미착수
  - 보안/배포 미완료: 패키징/릴리스 smoke gate(9.4~9.5) 미확정

- 2026-02-25: WBS 9.0 Tauri 전환 선행 작업(프론트 런타임 엔드포인트 정책) 착수
  - `frontend/src/runtime/endpoints.ts` 추가: 웹/데스크톱 런타임을 구분해 API base/WS URL 결정 로직을 단일화
  - `VITE_API_BASE_URL`, `VITE_WS_URL` 우선 정책 유지 + `VITE_TAURI_BACKEND_URL` 단일 오버라이드 경로 추가
  - Tauri 프로토콜(`tauri:`, `asset:`)에서는 기본 `127.0.0.1:8080` fallback을 사용해 PoC 단계의 연결 불확실성을 축소

- 2026-02-24: WBS 7.4 운영 점검 정례화 정책 수립 완료 (7.4.1~7.4.4)
  - `docs/operations/weekly-drill/README.md`: 주기/역할/시나리오별 PASS/FAIL 기준/완료 조건 고정
  - `docs/operations/weekly-drill/_template.md`: 주간 점검 결과 템플릿 생성
  - `docs/operations-runbook-scenarios.md` 섹션 7-1 저장 경로를 `docs/operations/weekly-drill/`로 정렬
  - 4주 점검 일정(2026-03-02, 03-09, 03-16, 03-30) 실행 완료
  - 7.4.5 완료 조건 달성: 4회 누적/시나리오 빈도/근거 지표/액션 관리 충족

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

- 2026-02-24: 상태 전이/에러 코드 설명 문구를 OpenAPI enum 기준으로 고정(문서 드리프트 완화)
  - `docs/architecture.md`, `docs/rust-cpp-backend-rewrite-plan.md`, `README.md`, `TODO/TODO.md` 상태명 표기를 `queued|running|completed|failed|timeout|canceled|rejected`로 교차 점검/정렬
  - 상태 전이와 에러 코드 설명은 OpenAPI enum을 단일 기준 텍스트로 유지

- 2026-02-23: OpenAPI/API 계약 Rust 목표 엔드포인트 정렬 상태 확인 및 WBS 반영
  - 당시 판정 기준 OpenAPI 버전(커밋): `225c316`
  - `docs/openapi.yaml`, `docs/swagger/openapi.yaml` 기준 엔드포인트가 `/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}`로 정렬됨을 재검증
  - 잡 상태/에러 코드(enum)가 Rust 오케스트레이터 구현 계약(`queued|running|completed|failed|timeout|canceled|rejected`, `invalid_job_id`, `queue_full|engine_full` 등)과 일치함을 확인
  - WBS `1.2 OpenAPI/API 계약 재정렬` 항목 완료 처리

- 2026-02-24: WBS 1.2(OpenAPI/API 계약 재정렬) 재검토 상태로 환원
  - 당시 판정 기준 OpenAPI 버전(커밋): `78020f1`
  - 구현 라우트/파라미터와 OpenAPI 간 1:1 매핑 검증 대상 엔드포인트 세트를 `/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}`로 고정하고 증적 부족으로 완료 판정을 보류
  - 재완료 조건: Rust 구현 라우트/파라미터 ↔ OpenAPI path/param의 1:1 매핑 확인 체크리스트 통과
  - 판정 체크리스트 기준 문서: `docs/openapi-wbs-1.2-recompletion-gate.md`
  - 후속 태스크: CI에 정적 계약 점검(필수 path/param 존재 + path-param 명칭 일치 검사 스크립트) 도입
  - 증적 규칙: 회차 로그에 CI 정적 계약 점검 스크립트 산출물 링크/경로(`artifacts/contracts/<run-id>/contract-drift-report.json` 등)를 첨부

- 2026-02-25: WBS 1.2(OpenAPI/API 계약 재정렬) 재완료 판정
  - 수동 대조 증적 문서: `docs/openapi-impl-path-param-manual-checklist.md`
  - 정적 계약 점검: `scripts/check_contract_drift.py`, `src/bin/check_contract_drift.rs`
  - CI 워크플로: `.github/workflows/contract-drift.yml`, `.github/workflows/contract-drift-check.yml`
  - 산출물 경로 규칙: `artifacts/contracts/${run_id}/contract-drift-report.json`

- 2026-02-22: Phase B-1 엔진별 큐 수용량/배압(429) 스켈레톤 도입
  - `POST /jobs?engine=<stt|summarize|embed>` 라우팅 추가(기본값 `stt`)
  - 엔진별 bounded capacity 기반 큐 포화 시 `429 + queue_full|engine_full` 반환
  - 엔진별 큐 포화가 다른 큐 접수에는 영향을 주지 않는 단위 테스트 추가
- 2026-02-22: Phase A+ 잡 API 스켈레톤 도입
  - `POST /jobs`에서 `202 + job_id` 반환 구현
  - `GET /jobs/{job_id}` 인메모리 조회(초기 상태 `queued`) 구현
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
- [x] 작업 API (`POST /jobs`, `GET /jobs/{job_id}`)
- [x] 헬스 엔드포인트 (`/healthz`, `/readyz`)
- [x] 엔진별 큐 (`stt/summarize/embed`) 수용량/배압 스켈레톤
- [x] 엔진 프로세스 슈퍼비전 (spawn/health/restart/shutdown)
- [x] Rust `symphonia` 전처리 파이프라인
- [x] Swagger 분리 배포 (`:14000`)

## WBS 진행 현황

## 1.0 아키텍처/계약 정합성

- [x] 1.1 Rust 단일 진입점/엔진 경계 문서 기준 확정
  - 근거 문서: `docs/architecture.md`, `docs/rust-cpp-backend-rewrite-plan.md`
- [x] 1.2 OpenAPI/API 계약을 Rust 목표 엔드포인트 기준으로 재정렬 (재완료: 2026-02-25)
  - 재완료 조건:
    - [x] `docs/openapi.yaml`, `docs/swagger/openapi.yaml`에 Rust 목표 엔드포인트가 동일하게 반영되어 있다.
    - [x] 구현 라우트/파라미터와 OpenAPI path/query/path-param의 **1:1 매핑 확인** 체크를 통과했다. (`docs/openapi-impl-path-param-manual-checklist.md`)
    - [x] 계약 드리프트 점검 항목(주간 점검/CI 정적 점검)이 활성 상태다. (`docs/operations/weekly-drill/README.md`, `.github/workflows/contract-drift.yml`, `.github/workflows/contract-drift-check.yml`)
    - [x] 문서 경로 파라미터 명칭 통일(`GET /jobs/{job_id}`) 체크포인트를 통과했다.

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
- [x] 4.1-확장 잡 상태 전이 전체 모델 (`running|completed|failed|timeout|canceled|rejected`)
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
- [x] 7.4 운영 점검 정례화(주기/역할/합격 기준/누적 완료조건)
  - [x] 7.4.1 운영 주기/역할/산출물 정책 고정 — `docs/operations/weekly-drill/README.md`
  - [x] 7.4.2 시나리오 A(장애 주입) PASS/FAIL 기준 고정
  - [x] 7.4.3 시나리오 B(복구 검증) PASS/FAIL 기준 고정
  - [x] 7.4.4 시나리오 C(부하/배압) PASS/FAIL 기준 고정
  - [x] 7.4.5 완료 조건 달성(최근 4회 누적 + 미해결 액션 0건)
  - 최소 운영 cadence: 주 1회(기본), 릴리스 안정화 구간은 격주로 완화 가능
  - 역할: 운영 담당(시나리오 실행/증적 수집), 리뷰어(판정/액션 승인)
  - 산출물: `docs/operations/weekly-drill/<YYYY-MM-DD>.md` + 액션 트래킹 표
  - 결과 템플릿: `docs/operations/weekly-drill/_template.md`

## 8.0 설치/실행 자동화 스크립트

- [x] 8.1 프로젝트 설치 스크립트(Windows `.bat`) 작성
  - [x] 설치 시작 전 env 기본 모델(STT/임베딩/요약) 세팅 확인
  - [x] 기본 모델 세팅 누락 시 설치 중단
  - [x] 모델 파일 존재 확인
  - [x] 모델 파일 누락 시 선택지 제공(설치 중단 / 모델 pull 진행)
  - [x] 프론트 의존성 설치 및 빌드(`npm` 등) 수행
  - [x] Rust 빌드 수행
- [x] 8.2 프로젝트 설치 스크립트(macOS/Linux `.sh`) 작성
  - [x] 설치 시작 전 env 기본 모델(STT/임베딩/요약) 세팅 확인
  - [x] 기본 모델 세팅 누락 시 설치 중단
  - [x] 모델 파일 존재 확인
  - [x] 모델 파일 누락 시 선택지 제공(설치 중단 / 모델 pull 진행)
  - [x] 프론트 의존성 설치 및 빌드(`npm` 등) 수행
  - [x] Rust 빌드 수행
- [x] 8.3 프로젝트 실행 스크립트(Windows `.bat`) 작성
  - [x] Rust 서버 실행
  - [x] 프론트 서버 실행
- [x] 8.4 프로젝트 실행 스크립트(macOS/Linux `.sh`) 작성
  - [x] Rust 서버 실행
  - [x] 프론트 서버 실행


## 9.0 Tauri 데스크톱 앱 전환

> 코드베이스 재점검(2026-02-26): `frontend/src/api/client.ts`, `frontend/vite.config.ts`, `frontend/src/runtime/endpoints.ts`, `frontend/src/hooks/useWebSocket.ts`, `src/bin/tauri_lifecycle_probe.rs`, `.github/workflows/tauri-lifecycle-poc.yml` 기준으로 항목 상태를 재검토했습니다.

- [ ] 9.1 Tauri 런처/런타임 PoC
  - [x] 프론트 런타임 엔드포인트 해석 로직 단일화(`resolveApiBaseUrl`, `resolveWebSocketUrl`) 및 Tauri fallback 추가
  - [x] Rust 오케스트레이터(`recordroute-orchestrator`)와 Swagger(`swagger_server`)를 Tauri lifecycle에서 기동/종료할 수 있는지 검증 (`src/bin/tauri_lifecycle_probe.rs`)
  - [x] Windows/Linux/macOS에서 기본 포트 충돌 없이 동시 기동되는지 검증 (`.github/workflows/tauri-lifecycle-poc.yml` 매트릭스)
  - [x] 앱 로그 수집/표시/회수 경로를 기존 run 스크립트(`scripts/run_*`)와 정렬 (`artifacts/tauri-lifecycle/<ts-os-pid>/`)

- [x] 9.2 프론트-백 계약 정합성 선결
  - [x] 프론트엔드 API 호출 경로(`/upload`, `/process`, `/tasks`, `/progress`, `/shutdown` 등)와 Rust API 간 갭을 문서화하고 우선순위 확정
    - 기준 문서: `docs/tauri-frontend-backend-contract-alignment.md`
    - P0: 프론트 레거시 경로와 Rust 핵심 계약(`/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}`) 갭을 명시하고 단계 전환 정책을 고정
  - [x] `frontend/vite.config.ts`의 프록시 대상(`http://localhost:8080`)과 런처에서 사용하는 `VITE_API_BASE_URL`를 단일 기준으로 재정의
    - `VITE_API_BASE_URL` 우선, `VITE_TAURI_BACKEND_URL` 보조, 미설정 시 `http://localhost:8080` fallback 규칙 문서화 완료
  - [x] `useWebSocket`의 `VITE_WS_URL`/`window.location` 기반 정책이 Tauri에서 동작할지 확인하고 필요 시 계약 고정
    - `resolveWebSocketUrl()` 정책(`VITE_WS_URL` 우선, Tauri fallback `ws://127.0.0.1:8080/ws`)을 모드별로 고정
  - [x] Tauri 도입 전/후 API 라우트 표준 (`/api/*` 래핑 여부 등) 결정
    - 신규 Rust 핵심 계약은 non-`/api` 기본, 기존 `/api/*` read-heavy endpoint는 전환 완료 전까지 한시 유지

- [x] 9.3 패키징/보안 체계 수립
  - [x] `tauri.conf.json` allowlist, CSP, 파일/프레임 권한 최소화 정책 확정
  - [x] 환경변수 주입(`RECORDROUTE_*`, 모델 경로, 로그 레벨) 및 비밀 관리 전략 확정
  - [x] 종료 처리(`shutdown`), 장애 재시작, 업그레이드 재진입 경로를 `engine_manager` 상태와 정합성 있게 정의
  - 기준 문서: `docs/tauri-packaging-security-baseline.md`

- [x] 9.4 설치/배포 자동화 통합
  - [x] 기존 설치/실행 스크립트(`scripts/install_*.sh`, `scripts/run_*.sh`)와 Tauri 배포 플로우를 1개 사용자 플로우로 통합
  - [x] 빌드 산출물 포함/제외(`target`, 앱 패키지 아티팩트) 정책을 `README/배포 문서`와 동기화
  - [x] 설치 게이트(필수 모델/의존성 확인)와 Tauri installer/업데이트 흐름 연동

- [ ] 9.5 릴리스 품질 게이트
  - [ ] `check_contract_drift`를 CI 필수 게이트로 유지하고 Tauri smoke test(기동 + 최소 기능 경로)와 단일 워크플로에서 결합
    - 산출물 기준: CI 로그에 contract drift 결과 + Tauri smoke 결과 + 아티팩트 경로(`artifacts/tauri-lifecycle/<ts-os-pid>/`)가 함께 남아야 함
  - [ ] WBS 1.2 재완료 규칙(`implement path/param 1:1`, path-param `job_id`)과 Tauri 런치 플로우 회귀 감시를 연동
    - 회귀 감시 기준 엔드포인트: `/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}`
  - [ ] 1인 실행 스크립트 실패 시 사용자 복구 가이드(`fallback`, `재실행`, `로그 조회`)를 문서화
    - 문서 포함 범위: `scripts/install_*.sh|bat --check` 실패 대응, `scripts/run_*.sh|bat` 재실행 순서, 로그 수집/확인 위치
## 다음 우선순위 (실행 단위)

1. **WBS 7.4.5 운영 점검 정례화 — 완료(2026-03-30)**
   - 정책(7.4.1~7.4.4)은 수립 완료: `docs/operations/weekly-drill/README.md` 참조
   - 진행 집계 문서: `docs/operations/weekly-drill/STATUS.md` (회차 추가 시 동시 갱신)
   - 4주 점검 일정: 2026-03-02, 2026-03-09, 2026-03-16, 2026-03-30
   - 7.4.5 완료 조건 체크리스트:
     - [x] 최근 4회(최소 1개월) 점검 결과가 `docs/operations/weekly-drill/`에 누적되어 있다.
     - [x] 시나리오 A/B/C 각각의 최소 수행 빈도를 충족한다.
     - [x] 회차별 PASS/FAIL 및 근거 지표(ready 복귀 시간, 429 reason 관측)가 기록되어 있다.
     - [x] 미해결 액션 아이템이 0건이거나, 모든 액션에 책임자/기한이 지정되어 있다.

2. **WBS 1.2 재완료 게이트 — OpenAPI/API 계약 1:1 매핑 검증 자동화**
   - [x] 재완료 판정 체크리스트 문서 고정 (`docs/openapi-wbs-1.2-recompletion-gate.md`)
   - [x] 구현 라우트/파라미터와 OpenAPI path/param의 수동 대조 체크리스트 완료 (`docs/openapi-impl-path-param-manual-checklist.md`)
   - [x] CI 정적 계약 점검(필수 path/param 존재 + path-param 명칭 일치 확인 스크립트) 추가 (`scripts/check_contract_drift.py`, `.github/workflows/contract-drift.yml`, `src/bin/check_contract_drift.rs`, `.github/workflows/contract-drift-check.yml`)
   - [x] 스크립트 결과와 산출물 링크/경로를 근거로 WBS 1.2 재완료 판정 (`artifacts/contracts/${run_id}/contract-drift-report.json`, `docs/openapi-impl-path-param-manual-checklist.md`)

3. **WBS 8.0 설치/실행 자동화 스크립트 — 완료(READY)**
   - [x] 설치 스크립트 2종 작성: Windows `.bat`, macOS/Linux `.sh`
   - [x] 설치 선행게이트 반영: env 기본 모델(STT/임베딩/요약) 세팅 확인 + 미세팅 시 중단
   - [x] 모델 파일 점검 반영: 미존재 시 선택지 제공(중단 / 모델 pull)
   - [x] 설치 단계 빌드 반영: 프론트 설치/빌드 + Rust 빌드
   - [x] 실행 스크립트 2종 작성: Windows `.bat`, macOS/Linux `.sh`
   - [x] 실행 단계 반영: Rust 서버 + 프론트 서버 실행
