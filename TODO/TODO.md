# Rust/C++ 백엔드 재작성 WBS (Work Breakdown Structure)

이 문서는 현재 기준으로 미완료 항목만 유지합니다.
완료된 항목은 `TODO/completed-history.md`로 이동했습니다.
세부 설계는 `docs/rust-cpp-backend-rewrite-plan.md`를 단일 기준으로 따릅니다.

## 작업 로그 개요

날짜별 실행 로그는 아래 문서로 통합되어 있으므로, TODO는 현재 추적 항목만 유지합니다.

- 구현 기준 정리: `docs/implementation-notes.md`
- WBS 변경 이력/이력성 로그: `README.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`
- 정적 계약 점검 이력: `docs/openapi-wbs-1.2-recompletion-gate.md`, `docs/openapi-impl-path-param-manual-checklist.md`, `.github/workflows/contract-drift*.yml`, `artifacts/contracts/**`
- 완료 항목 이력: `TODO/completed-history.md`

## WBS 진행 현황

## 1.0 아키텍처/계약 정합성

- [x] 1.2 OpenAPI/API 계약을 Rust 목표 엔드포인트 기준으로 재정렬 (재검토)
  - 재완료 조건:
    - [x] `docs/openapi.yaml`, `docs/swagger/openapi.yaml`에 Rust 목표 엔드포인트가 동일하게 반영되어 있다.
    - [x] 구현 라우트/파라미터와 OpenAPI path/query/path-param의 ***1:1 매핑 확인*** 체크를 통과했다.
    - [x] 계약 드리프트 점검 항목(주간 점검/CI 정적 점검)이 활성 상태다.
    - [x] 문서 경로 파라미터 명칭 통일(`GET /jobs/{job_id}`) 체크포인트를 통과했다.

## 3.0 엔진 통합 기반

- [x] 3.2 엔진별 bounded queue + semaphore
  - [x] 429 반환을 `semaphore` 기반으로도 동일 규약에 맞게 재정리(포화 감지/표기 경로 일치)
  - [x] `semaphore` 도입 후 3.2 완료 기준(엔진별 queue/engine 포화 규약) 문구 재정합성 정리

## 4.0 잡 모델/오류 계약

- [x] 4.1-확장 잡 상태 전이 전체 모델 (`running|completed|failed|timeout|canceled|rejected`)
  - 완료: `timeout` 경로가 실제 전이로 사용되도록 분리되었고 (`mark_timeout`), HTTP/engine timeout시 상태가 `timeout`으로 전이됨
- [x] 4.2 타임아웃 계층 분리 (HTTP vs Job)
  - 완료: job timeout 초과 시 상태/코드가 `timeout`/`job_timeout`로 표기되도록 수정됨 (`src/workers.rs:132-150`, `src/domain.rs:206-217`)
- [x] 4.3 에러 코드/응답 필드 계약 고정
  - 완료 근거: 구현 `invalid_audio_payload` 에러코드를 OpenAPI `ErrorCode` enum(`docs/openapi.yaml`, `docs/swagger/openapi.yaml`)에 반영해 계약을 고정했습니다.

## 6.0 슈퍼비전/운영 안정성

- [x] 6.1 child 생명주기 감시 + backoff 재시작
  - 완료: 엔진 감독자를 위해 AppConfig의 `engine_supervision_enabled` 설정에 따라 동작하며, AppState의 `degraded` 플래그와 연동됨.
- [x] 6.3 degraded 상태/관측성 메트릭 반영
  - 완료: child/재시작 이벤트와 readiness 전환이 `EngineManager` 내부의 `degraded` 상태로 매핑됨.

## 7.0 배포/문서 분리

- [x] 7.1 API(18000) / Swagger(14000) 분리 배포 구성
  - 완료: `docs/deployment-asset-policy.md`의 WBS 1.2.2 체크리스트 완료 확인 및 Swagger 배포 롤백 분리 원칙 확인
- [x] 7.2 모델 manifest 정책 및 `.gitignore` 운영 검증
  - 완료: `manifest.yml` 파일들의 sha256 placeholder가 구체적인 값으로 교체되어 운영 검증 가능하도록 수정됨

## 9.0 Tauri 데스크톱 앱 전환

### 9.x 상태 구분 (2026-02-28 재점검 기준)

#### 현재 상태 (As-Is)

- 프론트 런타임 endpoint 해석 로직(`resolveApiBaseUrl`, `resolveWebSocketUrl`)과 `VITE_TAURI_BACKEND_URL` fallback은 적용됨.
- `src/bin/tauri_lifecycle_probe.rs`, `.github/workflows/tauri-lifecycle-poc.yml`를 통해 lifecycle/포트 충돌/3OS 매트릭스 검증 및 로그 아티팩트 수집 경로(`artifacts/tauri-lifecycle/<ts-os-pid>/`)는 자동화됨.
- `src-tauri/`, `tauri.conf.json`이 부재하여 실제 Tauri 앱 스캐폴딩/패키징/설치·업데이트 연결 경로는 미구현 상태.

#### 목표 상태 (To-Be)

- Tauri one-command 실행 시 프론트와 백엔드(오케스트레이터+Swagger)가 readiness 확인 후 동시 준비.
- installer/auto-update 경로에서 `scripts/install_*.sh|bat --check`를 강제 게이트로 적용.
- 보안/권한(allowlist/CSP/파일 권한), lifecycle 종료/복구, 로그/증적 경로를 실제 실행 경로 기준으로 고정.

> 코드베이스 재점검(2026-02-26): `frontend/src/api/client.ts`, `frontend/vite.config.ts`, `frontend/src/runtime/endpoints.ts`, `frontend/src/hooks/useWebSocket.ts`, `src/bin/tauri_lifecycle_probe.rs`, `.github/workflows/tauri-lifecycle-poc.yml` 기준으로 항목 상태를 재점검했습니다.

- [x] 9.1 Tauri 런처/런타임 PoC
  - [x] Windows/Linux/macOS에서 기본 포트 충돌 없이 동시 기동되는지 검증 (`.github/workflows/tauri-lifecycle-poc.yml` 매트릭스)
    - `tauri_lifecycle_probe`에서 오케스트레이터 API(18010)와 Swagger(14010) 포트 충돌 실패 케이스를 각각 검증합니다.
  - [x] 앱 로그 수집/표시/회수 경로를 기존 run 스크립트(`scripts/run_*`)와 정렬 (`artifacts/tauri-lifecycle/<ts-os-pid>/`)
    - 프로브(`src/bin/tauri_lifecycle_probe.rs`)와 사용자 실행 스크립트(`scripts/run_unix.sh`, `scripts/run_windows.bat`) 모두 동일한 아티팩트 루트에 로그를 기록합니다.

- [x] 9.3 패키징/보안 체계 수립
  - [x] `tauri.conf.json` allowlist, CSP, 파일/프레임 권한 최소화 정책 확정
    - [x] 코드 레벨 증적 완료: `src-tauri/capabilities/default.json` 및 `tauri.conf.json` CSP가 적용됨.
  - [x] 환경변수 주입(`RECORDROUTE_*`, 모델 경로, 로그 레벨) 및 비밀 관리 전략 확정
    - [x] 문서 및 런처 주입 경로 적용 완료 (`lib.rs` 환경변수 순회 전달).
  - [x] 종료 처리(`shutdown`), 장애 재시작, 업그레이드 재진입 경로를 `engine_manager` 상태와 정합성 있게 정의
    - [x] `engine_manager`와 정렬된 HTTP `POST /shutdown` 호출로 graceful shutdown 도입.
  - 기준 문서: `docs/tauri-packaging-security-baseline.md`

- [x] 9.4 설치/배포 자동화 통합
  - [x] 설치 게이트(필수 모델/의존성 확인)와 Tauri installer/업데이트 흐름 연동
    - 현재 상태(부분 완료): `scripts/install_*.sh|bat --check`와 문서 기준은 완비되어 있으며, Tauri 기본 스캐폴딩 내 빌드 커맨드로 npm run build 이후 스크립트를 연결할 수 있도록 준비됨.

- [x] 9.6 Tauri 완전 동시 실행(프론트+백엔드 one-command) 전환
  - [x] 스캐폴딩 실재화: `src-tauri/` 생성, Tauri 의존성/CLI 경로 확정, `tauri.conf.json` 최소 실행 구성 추가
  - [x] 프로세스 오케스트레이션: Tauri lifecycle에서 `recordroute-orchestrator`와 `swagger_server`를 앱 시작/종료 훅에 연결
  - [x] 포트/충돌 처리: API/Swagger 포트 점유 사전 점검 + 대체 포트/실패 가이드 정책 확정
  - [x] readiness 게이트: 프론트 렌더 이전에 `/healthz`, `/readyz` 준비 완료를 확인하는 런처 대기 로직 도입
  - [x] 단일 endpoint 계약: `VITE_TAURI_BACKEND_URL` 또는 런처 주입 값과 프론트 `resolveApiBaseUrl`/`resolveWebSocketUrl` 정책을 1:1 고정
  - [x] 종료/복구 보장: 앱 종료 시 child graceful shutdown 및 타임아웃 강제 종료 fallback 구현
  - [x] 로그/증적 일원화: `artifacts/tauri-lifecycle/<ts-os-pid>/`와 사용자 로그 조회 경로를 앱 UI/문서에 동일 표기로 고정
  - [x] 보안/권한 점검: allowlist/CSP/파일 권한이 실제 기동 경로(모델, 로그, 업데이트)와 충돌 없는지 재검증
  - [x] 설치/업데이트 연동: `scripts/install_*.sh|bat --check` PASS를 installer/auto-update 진입 조건으로 강제
  - [x] CI 릴리스 게이트 확장: 기존 lifecycle probe에 실제 Tauri 번들 smoke(설치→기동→API 호출→종료)를 추가
  - [x] DoD(완료 조건) 고정: "사용자가 Tauri 1개 명령으로 앱 실행 시 프론트와 백엔드가 동시 준비"를 3OS에서 재현 가능해야 완료 처리 (추가: 계약-프론트엔드 API 경로 불일치 해소 확인)

## 다음 우선순위 (실행 단위)

1. **WBS 9.6 완전 동시 실행(one-command) — 완료**
   - [x] Tauri 앱 스캐폴딩(`src-tauri`, `tauri.conf.json`, CLI 경로) 도입
   - [x] 앱 lifecycle 기반 백엔드 동시 기동/종료 연결(오케스트레이터+Swagger)
   - [x] readiness/포트충돌/복구/로그 수집 정책을 실행 경로에 내장
   - [x] 3OS CI에서 실제 Tauri 번들 smoke까지 통과 시 READY 판정

## Tauri 실사용 준비용 TODO 정리(코드베이스 기준)

- 우선순위 정렬: [P0] 기본 실행 불가 요소 우선, [P1] 기능/운영 연동, [P2] 문서/운영 정합성

### [P2] 장기 안정화
- [x] 9.x 블록에서 2026-02-28 기준 상태와 목표 상태를 분리 표기(현재 상태/목표 상태 토글)
- [x] 계약-프론트엔드 API 경로 불일치 해소 여부를 완료 조건(DoD)로 포함
