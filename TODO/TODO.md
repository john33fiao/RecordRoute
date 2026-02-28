# Rust/C++ 백엔드 재작성 WBS (Work Breakdown Structure)

이 문서는 **현재 코드베이스 기준 실제 진행 상태**를 반영한 실행 추적표입니다.
세부 설계는 `docs/rust-cpp-backend-rewrite-plan.md`를 단일 기준으로 따릅니다.

## 작업 로그 개요

날짜별 실행 로그는 아래 문서로 통합되어 있으므로, TODO는 현재 추적 항목만 유지합니다.

- 구현 기준 정리: `docs/implementation-notes.md`
- WBS 변경 이력/이력성 로그: `README.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`
- 정적 계약 점검 이력: `docs/openapi-wbs-1.2-recompletion-gate.md`, `docs/openapi-impl-path-param-manual-checklist.md`, `.github/workflows/contract-drift*.yml`, `artifacts/contracts/**`

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
- [x] 1.2 OpenAPI/API 계약을 Rust 목표 엔드포인트 기준으로 재정렬 (재검토)
  - 재완료 조건:
    - [x] `docs/openapi.yaml`, `docs/swagger/openapi.yaml`에 Rust 목표 엔드포인트가 동일하게 반영되어 있다.
    - [ ] 구현 라우트/파라미터와 OpenAPI path/query/path-param의 ***1:1 매핑 확인*** 체크를 통과했다.
    - [ ] 계약 드리프트 점검 항목(주간 점검/CI 정적 점검)이 활성 상태다.
    - [x] 문서 경로 파라미터 명칭 통일(`GET /jobs/{job_id}`) 체크포인트를 통과했다.

## 2.0 런타임 스캐폴딩

- [x] 2.1 Rust 실행 진입점 및 로깅 부트스트랩 구현
- [x] 2.2 HTTP 서버 최소 구현(`/healthz`/`/readyz`)
- [ ] 2.3 설정 로더(포트/타임아웃/큐 크기) 도입 (포트/호스트 최소값)

## 3.0 엔진 통합 기반

- [x] 3.1 엔진별 클라이언트/포트 설정 (`18101`, `18102`, `18103`)
- [ ] 3.2 엔진별 bounded queue + semaphore
  - [x] 엔진별 bounded queue(`mpsc` 채널) 분리 구성
  - [x] 엔진별 worker pool(`concurrency` 기반)으로 동시 처리 상한 적용
  - [ ] 429 반환을 `semaphore` 기반으로도 동일 규약에 맞게 재정리(포화 감지/표기 경로 일치)
  - [ ] `semaphore` 도입 후 3.2 완료 기준(엔진별 queue/engine 포화 규약) 문구 재정합성 정리
- [x] 3.3 큐 포화/엔진 포화 `429` 규약 및 메트릭 라벨 분리

## 4.0 잡 모델/오류 계약

- [x] 4.1 잡 상태 전이 모델 (`queued` 초기 상태 + 조회 스켈레톤)
- [ ] 4.1-확장 잡 상태 전이 전체 모델 (`running|completed|failed|timeout|canceled|rejected`)
  - 완료: 상태 enum(`queued|running|completed|failed|timeout|canceled|rejected`) 자체는 코드/문서에 존재한다 (`src/domain.rs:38-48`, `docs/openapi.yaml`, `docs/swagger/openapi.yaml`)
  - 미완료: `timeout` 경로가 실제 전이로 사용되지 않으며, 현재 worker timeout은 `mark_canceled`로 수렴해 상태가 `canceled`/`job_canceled`로 종료됨 (`src/workers.rs:132-150`, `src/domain.rs:206-217`)
- [ ] 4.2 타임아웃 계층 분리 (HTTP vs Job)
  - 완료: HTTP timeout(`engine_connect_timeout`, `engine_request_timeout`)와 job timeout budget(`audio_ms` 기반 계산+clamp)는 분리되어 계산/전송된다 (`src/main.rs:775-808`, `src/main.rs:949-1062`, `src/main.rs:1185-1210`)
  - 미완료: job timeout 초과 시 상태/코드가 `timeout`/`job_timeout`로 표기되지 않고 `canceled`/`job_canceled`로 종료됨 (`src/workers.rs:132-150`, `src/main.rs:1396-1412`)
- [ ] 4.3 에러 코드/응답 필드 계약 고정
  - 완료: 응답 포맷(`ErrorBody { code, message }`)은 고정되어 있으며 OpenAPI 오류 스키마와 동기화 체계를 유지함 (`src/main.rs:876-877`, `docs/openapi.yaml`, `docs/swagger/openapi.yaml`)
  - 미완료: 구현에서 발생하는 `invalid_audio_payload` 에러코드가 OpenAPI `ErrorCode` enum에 미포함 (`src/main.rs:614-623`, `docs/openapi.yaml:116-138`)

## 5.0 오디오 전처리/처리량 정책

- [x] 5.1 `symphonia` 기반 오디오 정규화 (16kHz/16-bit mono WAV)
- [x] 5.2 길이/예산 계산 기반 timeout 산정식 적용
- [x] 5.3 whisper-server 추론 책임 한정(변환 책임 제거)

## 6.0 슈퍼비전/운영 안정성

- [ ] 6.1 child 생명주기 감시 + backoff 재시작
  - 구현: `EngineManager::spawn` + `supervise_engine` 루프에서 child spawn/종료 감시/예외 시 backoff 재시작이 동작한다. (`/src/engine_manager.rs:66~131`, `/src/engine_manager.rs:234~257`)
  - 미완료: 기본 실행은 `RECORDROUTE_ENGINE_SUPERVISION_ENABLED=false`로 시작되어 감독 기능이 기본 off 상태다. (`/src/main.rs:221~224`, `/src/main.rs:416~425`)
- [x] 6.2 graceful shutdown + 강제 종료 fallback
  - 구현: `graceful_shutdown`에서 grace 기간 대기 후 타임아웃 시 `start_kill` fallback을 수행한다. (`/src/engine_manager.rs:199~231`)
  - `/shutdown` 경로가 종료 플래그를 올리고, run loop 종료 후 엔진 supervisor 정리를 수행한다. (`/src/main.rs:589~597`, `/src/main.rs:446~447`)
- [ ] 6.3 degraded 상태/관측성 메트릭 반영
  - 구현: `/readyz`에서 `degraded` 기반 503/200 전환, `/metrics`의 readiness/engine/rejections 스냅샷 반영이 완료돼 있다. (`/src/main.rs:552~583`, `/src/main.rs:644~692`)
  - 미완료: child/재시작 이벤트가 readiness/degraded 상태 전환에 직접 연결되지 않아, 엔진 장애 복구 상태가 항상 자동으로 degraded 반영되는 흐름이 보장되지 않는다.

## 7.0 배포/문서 분리

- [ ] 7.1 API(18000) / Swagger(14000) 분리 배포 구성
  - 완료 근거:
    - API 서버 포트/설정은 `RECORDROUTE_API_PORT`로 18000 기본값 사용 (`src/main.rs`)
    - Swagger 서버는 별도 바이너리(`swagger_server`) + `RECORDROUTE_SWAGGER_PORT`(기본 14000)로 구동 (`src/bin/swagger_server.rs`)
    - 분리 실행/검증 경로는 `scripts/run-swagger.sh` 및 `scripts/run_unix.sh` + `scripts/run_windows.bat`에 반영
    - 라이프사이클 검증은 `.github/workflows/tauri-lifecycle-poc.yml`에서 orchestrator+swagger 동시 기동/종료 및 로그 수집으로 수행
  - 미완료:
    - `docs/deployment-asset-policy.md`의 WBS 1.2.2 체크리스트에서 `API(:18000)`와 `Swagger(:14000)` 독립 배포 정의 항목이 현재 미체크 상태여서, 기준 문서의 완료 조건이 최종적으로 충족되지 않음.
    - 조치: 체크리스트와 배포 정의의 완료 판정 기준을 동기화해 READY 상태로 갱신 필요.
- [ ] 7.2 모델 manifest 정책 및 `.gitignore` 운영 검증
  - 완료 근거:
    - `.gitignore`의 `models/**` 원본 제외 + `manifest.yml|yaml|json` 예외 규칙이 반영됨
    - `models/*/manifest.yml`가 저장되어 운영 추적 경로를 확보 (`models/stt`, `models/text`, `models/embed`)
  - 미완료:
    - 현재 manifest에 `checksum_sha256: "REPLACE_WITH_REAL_SHA256"` placeholder가 남아 있어 운영 실사용 검증용 메타데이터 신뢰도를 완성하지 못함
    - 조치: 실제 checksum/버전 메타 반영 및 산출물 검증 절차를 문서/파이프라인에 반영 필요
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
  - [x] RTD 상세: `scripts/install_windows.bat`에서 env/model 점검, 모델 누락 시 `prompt/--yes-pull/--no-pull` 분기, npm/cargo prerequisite 검사, frontend/npm 빌드, `cargo build --release`가 모두 실행됨이 확인됨
- [x] 8.2 프로젝트 설치 스크립트(macOS/Linux `.sh`) 작성
  - [x] 설치 시작 전 env 기본 모델(STT/임베딩/요약) 세팅 확인
  - [x] 기본 모델 세팅 누락 시 설치 중단
  - [x] 모델 파일 존재 확인
  - [x] 모델 파일 누락 시 선택지 제공(설치 중단 / 모델 pull 진행)
  - [x] 프론트 의존성 설치 및 빌드(`npm` 등) 수행
  - [x] Rust 빌드 수행
  - [x] RTD 상세: `scripts/install_unix.sh`에서 env/model 점검, 비대화형시 기본 cancel 처리, npm/cargo prerequisite 검사, frontend/npm 빌드, `cargo build --release`가 모두 실행됨이 확인됨
- [x] 8.3 프로젝트 실행 스크립트(Windows `.bat`) 작성
  - [x] Rust 서버 실행
  - [x] 프론트 서버 실행
  - [x] RTD 상세: `scripts/run_windows.bat`에서 release/dev 모드 분기 후 Rust API 서버 + `npm run dev`를 각각 별도 창으로 기동
- [x] 8.4 프로젝트 실행 스크립트(macOS/Linux `.sh`) 작성
  - [x] Rust 서버 실행
  - [x] 프론트 서버 실행
  - [x] RTD 상세: `scripts/run_unix.sh`에서 release/dev 모드 분기 후 백그라운드로 Rust API + frontend를 동시 기동하며 종료 시 cleanup trap으로 동시 종료 처리


## 9.0 Tauri 데스크톱 앱 전환

> 코드베이스 재점검(2026-02-26): `frontend/src/api/client.ts`, `frontend/vite.config.ts`, `frontend/src/runtime/endpoints.ts`, `frontend/src/hooks/useWebSocket.ts`, `src/bin/tauri_lifecycle_probe.rs`, `.github/workflows/tauri-lifecycle-poc.yml` 기준으로 항목 상태를 재검토했습니다.

- [x] 9.1 Tauri 런처/런타임 PoC
  - [x] 프론트 런타임 엔드포인트 해석 로직 단일화(`resolveApiBaseUrl`, `resolveWebSocketUrl`) 및 Tauri fallback 추가
  - [x] Rust 오케스트레이터(`recordroute-orchestrator`)와 Swagger(`swagger_server`)를 Tauri lifecycle에서 기동/종료할 수 있는지 검증 (`src/bin/tauri_lifecycle_probe.rs`)
  - [ ] Windows/Linux/macOS에서 기본 포트 충돌 없이 동시 기동되는지 검증 (`.github/workflows/tauri-lifecycle-poc.yml` 매트릭스)
    - `tauri_lifecycle_probe`에서 오케스트레이터 API 포트 충돌(18010/기본값)에 대한 실패 케이스 검증은 완료됨.
    - Swagger 포트(14010) 충돌/경합 검증이 별도 항목으로 없습니다.
  - [ ] 앱 로그 수집/표시/회수 경로를 기존 run 스크립트(`scripts/run_*`)와 정렬 (`artifacts/tauri-lifecycle/<ts-os-pid>/`)
    - 프로브(`src/bin/tauri_lifecycle_probe.rs`)는 `artifacts/tauri-lifecycle/<ts-os-pid>/`에 로그·요약 저장을 수행.
    - 기존 `scripts/run_unix.sh`, `scripts/run_windows.bat`는 로그 파일 아티팩트 수집 기능이 없어 사용자 실행 경로와 정렬되지 않음.

- [x] 9.2 프론트-백 계약 정합성 선결
  - [x] 프론트엔드 API 호출 경로(`/upload`, `/process`, `/tasks`, `/progress/{task_id}`, `/shutdown` 등)와 Rust API 간 갭을 문서화하고 우선순위 확정
    - 기준 문서: `docs/tauri-frontend-backend-contract-alignment.md`
    - P0: 프론트 레거시 경로와 Rust 핵심 계약(`/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}`) 갭을 명시하고 단계 전환 정책을 고정
    - 근거: 런타임에서 실제로 legacy 경로를 호출(`frontend/src/api/client.ts`)하며, `resolveApiBaseUrl`는 Rust 핵심 계약 경로로의 직접 전환 이전 단계임이 전제
  - [x] `frontend/vite.config.ts`의 프록시 대상(`http://localhost:8080`)과 런처에서 사용하는 `VITE_API_BASE_URL`를 단일 기준으로 재정의
    - `VITE_API_BASE_URL` 우선, `VITE_TAURI_BACKEND_URL` 보조, 미설정 시 `http://localhost:8080` fallback 규칙 문서화 완료
    - 근거: `resolveDevProxyHttpTarget()` 우선순위가 코드(`frontend/vite.config.ts`)와 문서(`docs/tauri-frontend-backend-contract-alignment.md`) 모두에서 동일하게 고정됨
  - [x] `useWebSocket`의 `VITE_WS_URL`/`window.location` 기반 정책이 Tauri에서 동작할지 확인하고 필요 시 계약 고정
    - `resolveWebSocketUrl()` 정책(`VITE_WS_URL` 우선, Tauri fallback `ws://127.0.0.1:8080/ws`)을 모드별로 고정
    - 근거: `resolveWebSocketUrl()`는 `VITE_WS_URL` → `VITE_TAURI_BACKEND_URL` 변환(`/ws`) → Tauri fallback로 동작하고, 웹 모드에서는 `window.location` 기반으로 fallback
  - [x] Tauri 도입 전/후 API 라우트 표준 (`/api/*` 래핑 여부 등) 결정
    - 신규 Rust 핵심 계약은 non-`/api` 기본, 기존 `/api/*` read-heavy endpoint는 전환 완료 전까지 한시 유지
    - 근거: 현재 `frontend/src/api/client.ts`에 `/api/similarity-graph`는 legacy read-heavy 용도 유지로 남아있어, 문서 기준과 코드가 일치

- [ ] 9.3 패키징/보안 체계 수립
  - [ ] `tauri.conf.json` allowlist, CSP, 파일/프레임 권한 최소화 정책 확정
    - [x] 문서 기준(`docs/tauri-packaging-security-baseline.md`)에 정책/최소 권한 원칙을 수립 완료.
    - [ ] 코드 레벨 증적 미확정: `src-tauri/` 디렉터리 및 `tauri.conf.json` 미생성(9.0 스캐폴딩 미도입 상태).
  - [ ] 환경변수 주입(`RECORDROUTE_*`, 모델 경로, 로그 레벨) 및 비밀 관리 전략 확정
    - [x] 운영 설정 네임스페이스 정렬(`RECORDROUTE_*`)은 `src/main.rs`에서 host/port/큐/동시성/타임아웃/엔진 URL 등 다수 반영.
    - [ ] 문서에서 요구한 모델 경로(`RECORDROUTE_MODEL_ROOT`), 로그 레벨(`RECORDROUTE_LOG_LEVEL`), 비밀 규약(`RECORDROUTE_SECRET_*` 마스킹/비노출) 및 런처 주입 경로 적용 미확정.
  - [ ] 종료 처리(`shutdown`), 장애 재시작, 업그레이드 재진입 경로를 `engine_manager` 상태와 정합성 있게 정의
    - [x] `/shutdown` 경로가 상태 플래그를 올리고 supervisor 종료/정리와 연결되는 기본 플로우는 존재.
    - [ ] `Booting/Ready/Degraded/Stopping/Stopped` 상태 모델과의 정합성 매핑은 문서 기준으로만 정의되어 실제 코드/상태 전이 증적은 미완료.
  - 기준 문서: `docs/tauri-packaging-security-baseline.md`

- [ ] 9.4 설치/배포 자동화 통합
  - [x] 기존 설치/실행 스크립트(`scripts/install_*.sh`, `scripts/run_*.sh`)와 Tauri 배포 플로우를 1개 사용자 플로우로 통합
  - [x] 빌드 산출물 포함/제외(`target`, 앱 패키지 아티팩트) 정책을 `README/배포 문서`와 동기화
  - [ ] 설치 게이트(필수 모델/의존성 확인)와 Tauri installer/업데이트 흐름 연동
    - 현재 상태(부분 완료): `scripts/install_*.sh|bat --check`와 문서 기준은 완비되어 있으나, `src-tauri` 스캐폴딩/installer 진입점 자체가 없어 `--check`를 설치/업데이트의 실제 강제 게이트로 호출하는 실행 경로가 없음.
    - 미해결: `src-tauri` 부재로 `installer/auto-update` 흐름이 실물 경로에서 연결되지 않음.

- [x] 9.5 릴리스 품질 게이트
  - [x] `check_contract_drift`를 CI 필수 게이트로 유지하고 Tauri smoke test(기동 + 최소 기능 경로)와 단일 워크플로에서 결합
    - 산출물 기준: CI 로그에 contract drift 결과 + Tauri smoke 결과 + 아티팩트 경로(`artifacts/tauri-lifecycle/<ts-os-pid>/`)가 함께 남아야 함
  - [x] WBS 1.2 재완료 규칙(`implement path/param 1:1`, path-param `job_id`)과 Tauri 런치 플로우 회귀 감시를 연동
    - 회귀 감시 기준 엔드포인트: `/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}`
  - [x] 1인 실행 스크립트 실패 시 사용자 복구 가이드(`fallback`, `재실행`, `로그 조회`)를 문서화
    - 문서 포함 범위: `scripts/install_*.sh|bat --check` 실패 대응, `scripts/run_*.sh|bat` 재실행 순서, 로그 수집/확인 위치
- [ ] 9.6 Tauri 완전 동시 실행(프론트+백엔드 one-command) 전환
  - [ ] 스캐폴딩 실재화: `src-tauri/` 생성, Tauri 의존성/CLI 경로 확정, `tauri.conf.json` 최소 실행 구성 추가
  - [ ] 프로세스 오케스트레이션: Tauri lifecycle에서 `recordroute-orchestrator`와 `swagger_server`를 앱 시작/종료 훅에 연결
  - [ ] 포트/충돌 처리: API/Swagger 포트 점유 사전 점검 + 대체 포트/실패 가이드 정책 확정
  - [ ] readiness 게이트: 프론트 렌더 이전에 `/healthz`, `/readyz` 준비 완료를 확인하는 런처 대기 로직 도입
  - [ ] 단일 endpoint 계약: `VITE_TAURI_BACKEND_URL` 또는 런처 주입 값과 프론트 `resolveApiBaseUrl`/`resolveWebSocketUrl` 정책을 1:1 고정
  - [ ] 종료/복구 보장: 앱 종료 시 child graceful shutdown 및 타임아웃 강제 종료 fallback 구현
  - [ ] 로그/증적 일원화: `artifacts/tauri-lifecycle/<ts-os-pid>/`와 사용자 로그 조회 경로를 앱 UI/문서에 동일 표기로 고정
  - [ ] 보안/권한 점검: allowlist/CSP/파일 권한이 실제 기동 경로(모델, 로그, 업데이트)와 충돌 없는지 재검증
  - [ ] 설치/업데이트 연동: `scripts/install_*.sh|bat --check` PASS를 installer/auto-update 진입 조건으로 강제
  - [ ] CI 릴리스 게이트 확장: 기존 lifecycle probe에 실제 Tauri 번들 smoke(설치→기동→API 호출→종료)를 추가
  - [ ] DoD(완료 조건) 고정: "사용자가 Tauri 1개 명령으로 앱 실행 시 프론트와 백엔드가 동시 준비"를 3OS에서 재현 가능해야 완료 처리
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

4. **WBS 9.6 완전 동시 실행(one-command) — 신규(미착수)**
   - [ ] Tauri 앱 스캐폴딩(`src-tauri`, `tauri.conf.json`, CLI 경로) 도입
   - [ ] 앱 lifecycle 기반 백엔드 동시 기동/종료 연결(오케스트레이터+Swagger)
   - [ ] readiness/포트충돌/복구/로그 수집 정책을 실행 경로에 내장
   - [ ] 3OS CI에서 실제 Tauri 번들 smoke까지 통과 시 READY 판정

## Tauri 실사용 준비용 TODO 정리(코드베이스 기준)

- 우선순위 정렬: [P0] 기본 실행 불가 요소 우선, [P1] 기능/운영 연동, [P2] 문서/운영 정합성

### [P0] 즉시 조치
- [x] 9.1 체크 헤더 상태 정합성 수정: 하위 항목이 모두 완료된 상태면 9.1 헤더도 완료(`[x]`)로 정리
- [x] Tauri 앱 스캐폴딩 실제 유무 확인 기준을 TODO에 반영 (`src-tauri` + Tauri deps + `tauri` CLI 사용 경로)
  - 점검 기준: `src-tauri/` 존재 여부 + Tauri 의존성 선언 + `tauri` CLI 사용 경로
  - 현재 상태(2026-02-28): 세 기준 모두 미충족(미도입). 9.5/실사용 전환 전 별도 스캐폴딩 도입 태스크로 관리 필요.

### [P1] 실사용 기능/운영 정합성
- [x] 기존 9.5 항목의 실제 구현 반영 정렬(단일 게이트 통합 + 드리프트-런타임 연동 + 실패복구 가이드)
  - [x] 9.5.1 계약 점검(`check_contract_drift`)과 Tauri 라이프사이클 스모크(`tauri_lifecycle_probe`)를 단일 CI 워크플로/게이트로 통합
  - [x] WBS 1.2 기준 엔드포인트(`/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}`)를 Tauri 실행/표시 플로우와 1:1로 연결
  - [x] 스모크/런타임 실패 시 사용자 복구 절차(재시작, 환경 재설치, 로그 수집, 재검증) TODO 문서 항목으로 분리
- [x] 코드 완성도 강화(P0): `src/audio.rs` 정규화 유틸의 미사용 경고 제거 (`normalize_to_wav_mono_16k` 사용 경로 확정)
- [x] 코드 완성도 강화(P0): 업로드/전처리 파이프라인에서 `collect_mono_f32_from_f32`, `linear_resample`, `build_wav_mono_i16`를 실제 호출하도록 연결
- [x] 코드 완성도 강화(P1): `JobStatus::Canceled` 및 `mark_canceled`를 실제 취소 API/취소 이벤트(클라이언트 요청, 타임아웃 전환)와 연동
- [x] 코드 완성도 강화(P1): `EngineManager::shutdown`를 종료 경로와 연결해 `shutdown_tx`, `tasks` 필드의 사용 경로를 실사용으로 전환

### [P2] 장기 안정화
- [ ] 9.x 블록에서 2026-02-28 기준 상태와 목표 상태를 분리 표기(현재 상태/목표 상태 토글)
- [ ] 계약-프론트엔드 API 경로 불일치 해소 여부를 완료 조건(DoD)로 포함
