# Rust/C++ 백엔드 재작성 WBS (Work Breakdown Structure)

이 문서는 **코드베이스 기준 미완료 항목만** 유지합니다.
완료된 항목은 `TODO/completed-history.md`로 이동했습니다.
세부 설계는 `docs/rust-cpp-backend-rewrite-plan.md`를 단일 기준으로 따릅니다.

## 작업 로그 개요

- 구현 기준 정리: `docs/implementation-notes.md`
- WBS 변경 이력/이력성 로그: `README.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`
- 정적 계약 점검 이력: `docs/openapi-wbs-1.2-recompletion-gate.md`, `docs/openapi-impl-path-param-manual-checklist.md`, `.github/workflows/contract-drift*.yml`, `artifacts/contracts/**`
- 완료 항목 이력: `TODO/completed-history.md`

## WBS 진행 현황 (미완료)

## 4.0 잡 모델/오류 계약

- [ ] 4.1-확장 잡 상태 전이 계약 재정렬 (`timeout` 처리)
  - 현재 상태: 구현 `JobStatus`에는 `timeout`이 존재하지만 OpenAPI enum 단일 기준은 `queued|running|completed|failed|canceled|rejected`입니다.
  - 추가 작업 필요:
    - `timeout`을 별도 상태로 유지할지, `failed` + `job_timeout` 코드로 수렴할지 정책 확정
    - 정책에 따라 구현(`src/domain.rs`, `src/workers.rs`, `src/main.rs`)과 OpenAPI(`docs/openapi.yaml`, `docs/swagger/openapi.yaml`)를 1:1로 정합화
    - 계약 드리프트 점검 및 수동 체크리스트(`docs/openapi-impl-path-param-manual-checklist.md`) 재실행

## 9.0 Tauri 데스크톱 앱 전환

> 정책 고정: WBS 9.0 전체(보안/패키징/릴리스 게이트)는 미완료 상태를 유지합니다.

- [ ] 9.3 패키징/보안 체계 실구현 완결
  - 현재 상태: Tauri에서 백엔드를 `cargo run --bin recordroute-orchestrator`로 직접 스폰하고 있으며 sidecar/배포 바이너리 기준 실행 경로가 고정되지 않았습니다.
  - 추가 작업 필요:
    - sidecar 기반 실행 경로로 전환 및 dev/prod 분기 정책 문서화
    - allowlist/CSP/권한 설정과 실제 모델/로그/업데이트 경로 충돌 재검증

- [ ] 9.4 설치/배포 자동화 통합 게이트 강제
  - 현재 상태: `scripts/install_*.sh|bat --check`는 존재하지만 installer/auto-update 진입 전에 강제되는 코드/CI 게이트는 미흡합니다.
  - 추가 작업 필요:
    - Tauri installer/업데이트 파이프라인에서 `--check` 선행 실패 시 즉시 차단
    - 게이트 결과를 CI artifact로 수집하도록 워크플로 보강

- [ ] 9.6 Tauri 완전 동시 실행(one-command) DoD 충족
  - 현재 상태: 스캐폴딩(`src-tauri/`)은 존재하지만 런타임에서 Swagger 동시 기동/종료, 포트 대체 전략, 단일 실행 완결성이 미흡합니다.
  - 추가 작업 필요:
    - 앱 lifecycle에서 orchestrator + swagger 동시 기동/종료를 one-command로 고정
    - 포트 충돌 시 대체 포트/가이드 정책을 런처 구현으로 일치
    - readiness(`/healthz`, `/readyz`) 및 사용자 가시 로그 경로를 실제 앱 동작과 동일화

## 다음 우선순위 (실행 단위)

1. **WBS 4.1 계약 정합성 복구**
   - [ ] `timeout` 상태 정책 단일화(구현 vs OpenAPI)
   - [ ] 드리프트 점검/수동 체크리스트 재검증
2. **WBS 9.3~9.6 실구현 완결**
   - [ ] sidecar 기반 lifecycle + 설치 게이트 강제 + one-command DoD 재검증
