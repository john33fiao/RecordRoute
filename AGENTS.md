# AGENTS.md - RecordRoute 루트 작업 기준 (Rust 전환)

이 문서는 저장소 루트(`./`) 기준 에이전트 작업 표준입니다.

> 문서 동기화 메모: 2026-03-30 기준 운영 안정화 + 운영 점검 정례화(7.4.1~7.4.5), 2026-02-25 기준 WBS 1.2 재완료 상태, 2026-02-26 기준 TODO(9.2 코드베이스 재점검 코멘트) 반영 상태를 README/CLAUDE/GEMINI/TODO와 정렬.
> 동기화 포인트: runbook 저장 경로를 `docs/operations/weekly-drill/`로 통일, 정례화 정책/템플릿 신규 문서 반영.

> 2026-02-27 업데이트: WBS 9.2 기준 문서 `docs/tauri-frontend-backend-contract-alignment.md`를 추가하고 TODO/README/CLAUDE/GEMINI와 상태를 동기화.

## 1) 프로젝트 구조 인식

- `frontend/`: 운영 중인 프론트엔드 코드
- `docs/`: Rust 전환/설계 문서
- (현재 없음) `deprecated/`: 과거 Python 레거시 코드베이스 경로였으며, 현 저장소에는 포함되지 않습니다.

원칙:
- 신규 구현/문서화는 Rust 전환 목표를 기준으로 작성합니다.
- 현재 저장소에는 `deprecated/` 디렉터리가 없으므로 레거시 실행 경로를 기본 타깃으로 가정하지 않습니다.

## 2) Rust 전환 기본 방향

- 목표 아키텍처: **Rust 오케스트레이터 + C++ 엔진(독립 프로세스)**
- 엔진 연동: FFI 대신 내부 HTTP 계약 우선
- 큐: 단일 큐 금지, 엔진별 큐/동시성 분리
- 운영: 헬스체크/재시작/타임아웃/배압을 Rust 계층에서 명시적으로 관리
- 오디오 전처리: `ffmpeg` 외부 프로세스 대신 Rust `symphonia` 크레이트 기반 변환을 기본값으로 사용
- 자산 추적: `vendor/` 소스는 버전관리, `models/`는 raw 데이터 제외 후 manifest(`manifest.yml|yaml|json`)만 추적

세부 정책은 `docs/rust-cpp-backend-rewrite-plan.md`를 단일 기준으로 따르며, 아키텍처 기준선은 `docs/architecture.md`를 참조합니다.
배포/자산 세부 운영 기준은 `docs/deployment-asset-policy.md`를 참조합니다.
`main.rs` 분할/모듈화 실무 기준은 `docs/main-rs-modularization-guide.md`를 참조합니다.
최종 사용자 조작 절차는 `docs/user-operation-manual.md`를 참조합니다.
운영 점검 정례화 정책은 `docs/operations/weekly-drill/README.md`를 참조합니다.
WBS 1.2 재완료 게이트(구현 라우트/파라미터 ↔ OpenAPI path/param 1:1 매핑) 기준은 `docs/openapi-wbs-1.2-recompletion-gate.md`를 단일 체크리스트로 참조합니다.
7.4.5 완료 조건 누적 추적은 `docs/operations/weekly-drill/STATUS.md`를 기준으로 갱신합니다.
실행 단위/의존성 추적은 `TODO/TODO.md`를 함께 참조합니다.

## 3) 문서 동기화 규칙

아래 파일은 항상 함께 최신화합니다.

- `AGENTS.md` (원본 기준)
- `CLAUDE.md` (요약)
- `GEMINI.md` (요약)
- `README.md` (사용자 관점 개요)

문서 간 충돌 방지 원칙:
- 상세 규칙은 `AGENTS.md`에만 둡니다.
- `CLAUDE.md`, `GEMINI.md`는 링크/체크리스트 중심으로 유지합니다.

## 4) 변경 우선순위

- 2026-02-25 Tauri 전환 상태 점검 기준: WBS 9.0은 착수 단계이며, 완료로 간주 가능한 범위는 프론트 런타임 endpoint 해석 로직(`resolveApiBaseUrl`, `resolveWebSocketUrl`)과 `VITE_TAURI_BACKEND_URL` fallback까지로 제한합니다. lifecycle/보안/패키징/릴리스 게이트는 미완료로 유지합니다.

- 2026-03-30 이후 9.1 검증 자동화 기준: `src/bin/tauri_lifecycle_probe.rs` 및 `.github/workflows/tauri-lifecycle-poc.yml`로 오케스트레이터/Swagger lifecycle 기동·종료, 포트 충돌, 3OS 매트릭스 검증, 로그 아티팩트 수집(`artifacts/tauri-lifecycle/<ts-os-pid>/`)을 수행합니다. 단, 9.0 전체(보안/패키징/릴리스 게이트)는 여전히 미완료로 유지합니다.


정렬 상태 메모:
- WBS `1.2 OpenAPI/API 계약 재정렬`은 구현 라우트/파라미터와 OpenAPI path/param 1:1 매핑 재검증 완료(수동/자동/증적 기록 충족) 상태로 관리합니다.
- 운영 점검 정례화(7.4.1~7.4.4)에는 계약 드리프트 주간 점검(구현↔OpenAPI path/param 대조 + CI 정적 계약 점검 확인 + 경로 파라미터 명칭 일치 검증 + 기준 엔드포인트 세트 `/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}` 고정)을 포함합니다.
- OpenAPI 계약(`docs/openapi.yaml`, `docs/swagger/openapi.yaml`)은 Rust 목표 엔드포인트 기준으로 유지합니다.
- OpenAPI 잡 상태 enum은 `queued|running|completed|failed|timeout|canceled|rejected`를 단일 기준으로 유지합니다.
1. 계약 안정성(API 응답 필드/에러 규약)
2. 운영 안전성(타임아웃, 큐 포화, 헬스체크)
3. 성능 최적화(검색/벡터/read-heavy 경로)
4. 전체 워크플로우 전환(`/process`)은 Go/No-Go 이후

## 5) 검증 규칙

문서 변경만 있을 때 최소 검증:
- Markdown 링크/구조 점검
- 저장소 상태 점검(`git status`)

코드 변경이 포함되면 해당 스택 검증 필수:
- 프론트: `cd frontend && npm run build`
- 레거시 Python(외부/별도 저장소에서만): 해당 저장소 테스트 규칙에 따라 `pytest` 또는 영향 범위 테스트
- Rust 코드 도입 시: `cargo test`, `cargo clippy`(도입 이후 필수)

## 5-1) RTD(배포준비) 스킬 적용 규칙

- 첫 코드 변경 전에 RTD Step 1~4(계획/검토/과도성 제거)를 수행하고, 각 단계 PASS/FAIL과 핵심 근거(최대 5줄)를 남깁니다.
- 구현 완료 후 배포(커밋) 전에 RTD Step 5~18을 순서대로 수행하고, 각 단계 PASS/FAIL과 핵심 근거(최대 5줄)를 남깁니다.
- FAIL이 발생하면 원인을 해결한 뒤 실패 단계부터 재수행합니다.
- 보안 리스크(권한/인증/인가/인젝션 등) 또는 롤백 경로가 불명확하면 PASS 판정을 금지합니다.
- Step 1~18 전체 PASS(READY) 상태에서만 커밋/PR 초안을 진행합니다.

## 6) 레거시 스코프 주의

- 현재 저장소 기준 `deprecated/`는 비어 있거나 존재하지 않을 수 있습니다.
- 향후 `deprecated/`가 재도입되어 하위 파일을 수정할 경우, 해당 경로의 `AGENTS.md`가 있으면 우선 준수합니다.

## 7) 금지/권장

금지:
- 전환 미완료 상태를 완료된 것처럼 문서화
- 존재하지 않는 실행 경로를 기본 사용법으로 제시

권장:
- “현재 상태”와 “목표 상태”를 문서에서 명확히 분리
- 레거시 참조 경로를 명시해 온보딩 혼선을 줄이기

## 8) Rust 빌드(Windows) 트러블슈팅

Windows에서 `cargo build` 실행 시 아래 오류가 발생하면 rustup toolchain/component 불일치 가능성이 큽니다.

```powershell
cargo build
error: the 'rustc.exe' binary ... is not applicable to the 'stable-x86_64-pc-windows-msvc' toolchain
```

복구 절차(순서 고정):

```powershell
rustup show
rustup toolchain uninstall stable-x86_64-pc-windows-msvc
rustup toolchain install stable-x86_64-pc-windows-msvc --profile default
rustup default stable-x86_64-pc-windows-msvc
rustup component add rustc cargo clippy rustfmt --toolchain stable-x86_64-pc-windows-msvc
rustup update
cargo build
```

지속 실패 시 `where rustc`, `rustup which rustc`로 PATH 중복을 점검하고 Visual Studio Build Tools(`MSVC v143`, `Windows 10/11 SDK`) 설치 상태를 확인합니다.
