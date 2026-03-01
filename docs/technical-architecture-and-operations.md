# RecordRoute 기술 개요 (아키텍처/운영/배포)

이 문서는 README에서 분리한 **기술 기준** 문서입니다.

## 프로젝트 구조

- `frontend/`: 운영 중인 웹 프론트엔드
- `docs/`: Rust 전환/설계/운영 문서
- `deprecated/`: 현재 저장소에는 없음(레거시는 별도 보관소 기준)

## 현재 상태/목표 상태

### 현재 상태

- Rust 중심 전환 진행 중이며, 핵심 API는 `/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}` 기준으로 관리합니다.
- 프론트 런타임 endpoint 해석(`resolveApiBaseUrl`, `resolveWebSocketUrl`)과 `VITE_TAURI_BACKEND_URL` fallback 정책은 반영되어 있습니다.
- Tauri lifecycle/보안/패키징/릴리스 게이트는 미완료 상태입니다.

### 목표 상태

- 아키텍처: Rust 오케스트레이터 + C++ 엔진(독립 프로세스)
- 엔진 연동: FFI 대신 내부 HTTP 계약 우선
- 큐 전략: 엔진별 bounded queue + worker 분리
- 오디오 전처리: 외부 `ffmpeg`보다 Rust `symphonia` 기반 변환 우선

## API/계약 기준

- OpenAPI 기준 문서:
  - `docs/openapi.yaml`
  - `docs/swagger/openapi.yaml`
- 잡 상태 enum: `queued|running|completed|failed|canceled|rejected`
- 과부하 응답 규약: `429(queue_full|engine_full)`
- `POST /jobs`는 `audio_ms` query parameter를 받아 timeout budget 산정에 활용
- `GET /metrics`는 readiness(ready/degraded), 엔진별 queue/running, rejection 정보를 노출

## 운영 안정화/WBS 메모

- WBS 1.2 재완료 게이트: `docs/openapi-wbs-1.2-recompletion-gate.md`
- 운영 점검 정례화(7.4.x): `docs/operations/weekly-drill/README.md`
- 7.4.5 완료 조건 누적 추적: `docs/operations/weekly-drill/STATUS.md`
- 운영 runbook 시나리오: `docs/operations-runbook-scenarios.md`

## 설치/배포/자산 정책

- 설치/배포 통합 플로우: `docs/tauri-install-deploy-unified-flow.md`
- 패키징/보안 기준: `docs/tauri-packaging-security-baseline.md`
- 배포/자산 정책: `docs/deployment-asset-policy.md`
- 모델 자산 정책: raw 데이터 미추적, `manifest.yml|yaml|json`만 추적

## 개발/검증 참고

- 구현 기준 노트: `docs/implementation-notes.md`
- 아키텍처 기준선: `docs/architecture.md`
- Rust+C++ 전환 계획: `docs/rust-cpp-backend-rewrite-plan.md`
- `main.rs` 모듈화 가이드: `docs/main-rs-modularization-guide.md`
- 실행 단위/WBS 추적: `TODO/TODO.md`
- RTD(배포준비) 절차: `docs/RTD.md`

## Windows Rust 빌드 트러블슈팅

`cargo build` 시 아래 오류(`rustc.exe ... not applicable`)가 나면 toolchain/component 불일치 가능성이 큽니다.

```powershell
rustup show
rustup toolchain uninstall stable-x86_64-pc-windows-msvc
rustup toolchain install stable-x86_64-pc-windows-msvc --profile default
rustup default stable-x86_64-pc-windows-msvc
rustup component add rustc cargo clippy rustfmt --toolchain stable-x86_64-pc-windows-msvc
rustup update
cargo build
```

지속 실패 시 `where rustc`, `rustup which rustc`로 PATH 중복과 Visual Studio Build Tools(`MSVC v143`, `Windows 10/11 SDK`)를 점검합니다.
