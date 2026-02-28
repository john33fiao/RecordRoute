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
    - [ ] 계약 드리프트 점검 항목(주간 점검/CI 정적 점검)이 활성 상태다.
    - [x] 문서 경로 파라미터 명칭 통일(`GET /jobs/{job_id}`) 체크포인트를 통과했다.

## 2.0 런타임 스캐폴딩

- [ ] 2.3 설정 로더(포트/타임아웃/큐 크기) 도입 (포트/호스트 최소값)

## 3.0 엔진 통합 기반

- [ ] 3.2 엔진별 bounded queue + semaphore
  - [ ] 429 반환을 `semaphore` 기반으로도 동일 규약에 맞게 재정리(포화 감지/표기 경로 일치)
  - [ ] `semaphore` 도입 후 3.2 완료 기준(엔진별 queue/engine 포화 규약) 문구 재정합성 정리

## 4.0 잡 모델/오류 계약

- [ ] 4.1-확장 잡 상태 전이 전체 모델 (`running|completed|failed|timeout|canceled|rejected`)
  - 미완료: `timeout` 경로가 실제 전이로 사용되지 않으며, 현재 worker timeout은 `mark_canceled`로 수렴해 상태가 `canceled`/`job_canceled`로 종료됨 (`src/workers.rs:132-150`, `src/domain.rs:206-217`)
- [ ] 4.2 타임아웃 계층 분리 (HTTP vs Job)
  - 미완료: job timeout 초과 시 상태/코드가 `timeout`/`job_timeout`로 표기되지 않고 `canceled`/`job_canceled`로 종료됨 (`src/workers.rs:132-150`, `src/main.rs:1396-1412`)
- [ ] 4.3 에러 코드/응답 필드 계약 고정
  - 미완료: 구현에서 발생하는 `invalid_audio_payload` 에러코드가 OpenAPI `ErrorCode` enum에 미포함 (`src/main.rs:614-623`, `docs/openapi.yaml:116-138`)

## 6.0 슈퍼비전/운영 안정성

- [ ] 6.1 child 생명주기 감시 + backoff 재시작
  - 미완료: 기본 실행은 `RECORDROUTE_ENGINE_SUPERVISION_ENABLED=false`로 시작되어 감독 기능이 기본 off 상태다. (`/src/main.rs:221~224`, `/src/main.rs:416~425`)
- [ ] 6.3 degraded 상태/관측성 메트릭 반영
  - 미완료: child/재시작 이벤트가 readiness/degraded 상태 전환에 직접 연결되지 않아, 엔진 장애 복구 상태가 항상 자동으로 degraded 반영되는 흐름이 보장되지 않는다.

## 7.0 배포/문서 분리

- [ ] 7.1 API(18000) / Swagger(14000) 분리 배포 구성
  - 미완료:
    - `docs/deployment-asset-policy.md`의 WBS 1.2.2 체크리스트에서 `API(:18000)`와 `Swagger(:14000)` 독립 배포 정의 항목이 현재 미체크 상태여서, 기준 문서의 완료 조건이 최종적으로 충족되지 않음.
    - 조치: 체크리스트와 배포 정의의 완료 판정 기준을 동기화해 READY 상태로 갱신 필요.
- [ ] 7.2 모델 manifest 정책 및 `.gitignore` 운영 검증
  - 미완료:
    - 현재 manifest에 `checksum_sha256: "REPLACE_WITH_REAL_SHA256"` placeholder가 남아 있어 운영 실사용 검증용 메타데이터 신뢰도를 완성하지 못함
    - 조치: 실제 checksum/버전 메타 반영 및 산출물 검증 절차를 문서/파이프라인에 반영 필요

## 9.0 Tauri 데스크톱 앱 전환

> 코드베이스 재점검(2026-02-26): `frontend/src/api/client.ts`, `frontend/vite.config.ts`, `frontend/src/runtime/endpoints.ts`, `frontend/src/hooks/useWebSocket.ts`, `src/bin/tauri_lifecycle_probe.rs`, `.github/workflows/tauri-lifecycle-poc.yml` 기준으로 항목 상태를 재점검했습니다.

- [ ] 9.1 Tauri 런처/런타임 PoC
  - [ ] Windows/Linux/macOS에서 기본 포트 충돌 없이 동시 기동되는지 검증 (`.github/workflows/tauri-lifecycle-poc.yml` 매트릭스)
    - `tauri_lifecycle_probe`에서 오케스트레이터 API 포트 충돌(18010/기본값)에 대한 실패 케이스 검증은 완료됨.
    - Swagger 포트(14010) 충돌/경합 검증이 별도 항목으로 없습니다.
  - [ ] 앱 로그 수집/표시/회수 경로를 기존 run 스크립트(`scripts/run_*`)와 정렬 (`artifacts/tauri-lifecycle/<ts-os-pid>/`)
    - 프로브(`src/bin/tauri_lifecycle_probe.rs`)는 `artifacts/tauri-lifecycle/<ts-os-pid>/`에 로그·요약 저장을 수행.
    - 기존 `scripts/run_unix.sh`, `scripts/run_windows.bat`는 로그 파일 아티팩트 수집 기능이 없어 사용자 실행 경로와 정렬되지 않음.

- [ ] 9.3 패키징/보안 체계 수립
  - [ ] `tauri.conf.json` allowlist, CSP, 파일/프레임 권한 최소화 정책 확정
    - [ ] 코드 레벨 증적 미확정: `src-tauri/` 디렉터리 및 `tauri.conf.json` 미생성(9.0 스캐폴딩 미도입 상태).
  - [ ] 환경변수 주입(`RECORDROUTE_*`, 모델 경로, 로그 레벨) 및 비밀 관리 전략 확정
    - [ ] 문서에서 요구한 모델 경로(`RECORDROUTE_MODEL_ROOT`), 로그 레벨(`RECORDROUTE_LOG_LEVEL`), 비밀 규약(`RECORDROUTE_SECRET_*` 마스킹/비노출) 및 런처 주입 경로 적용 미확정.
  - [ ] 종료 처리(`shutdown`), 장애 재시작, 업그레이드 재진입 경로를 `engine_manager` 상태와 정합성 있게 정의
    - [ ] `Booting/Ready/Degraded/Stopping/Stopped` 상태 모델과의 정합성 매핑은 문서 기준으로만 정의되어 실제 코드/상태 전이 증적은 미완료.
  - 기준 문서: `docs/tauri-packaging-security-baseline.md`

- [ ] 9.4 설치/배포 자동화 통합
  - [ ] 설치 게이트(필수 모델/의존성 확인)와 Tauri installer/업데이트 흐름 연동
    - 현재 상태(부분 완료): `scripts/install_*.sh|bat --check`와 문서 기준은 완비되어 있으나, `src-tauri` 스캐폴딩/installer 진입점 자체가 없어 `--check`를 설치/업데이트의 실제 강제 게이트로 호출하는 실행 경로가 없음.
    - 미해결: `src-tauri` 부재로 `installer/auto-update` 흐름이 실물 경로에서 연결되지 않음.

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

1. **WBS 9.6 완전 동시 실행(one-command) — 신규(미착수)**
   - [ ] Tauri 앱 스캐폴딩(`src-tauri`, `tauri.conf.json`, CLI 경로) 도입
   - [ ] 앱 lifecycle 기반 백엔드 동시 기동/종료 연결(오케스트레이터+Swagger)
   - [ ] readiness/포트충돌/복구/로그 수집 정책을 실행 경로에 내장
   - [ ] 3OS CI에서 실제 Tauri 번들 smoke까지 통과 시 READY 판정

## Tauri 실사용 준비용 TODO 정리(코드베이스 기준)

- 우선순위 정렬: [P0] 기본 실행 불가 요소 우선, [P1] 기능/운영 연동, [P2] 문서/운영 정합성

### [P2] 장기 안정화
- [ ] 9.x 블록에서 2026-02-28 기준 상태와 목표 상태를 분리 표기(현재 상태/목표 상태 토글)
- [ ] 계약-프론트엔드 API 경로 불일치 해소 여부를 완료 조건(DoD)로 포함
