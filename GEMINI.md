# GEMINI.md - RecordRoute 작업 요약

최신 원본 기준은 `AGENTS.md`입니다. 이 문서는 실행 요약만 제공합니다.


> 문서 동기화: 2026-03-30 기준 운영 안정화 + 운영 점검 정례화(7.4.1~7.4.5) 완료 상태, 2026-02-26 기준 TODO(9.2 코드베이스 재점검 코멘트) 반영 상태와 정렬됨.

- 2026-02-27: WBS 9.2 프론트-백 계약 정합성 기준 문서(`docs/tauri-frontend-backend-contract-alignment.md`)를 추가하고 TODO 9.2 완료 상태를 동기화했습니다.

## 핵심 컨텍스트

- 2026-02-25 Tauri 전환 상태 점검: WBS 9.0은 착수 단계이며, 완료 범위는 프론트 endpoint 해석 로직 단일화(`resolveApiBaseUrl`, `resolveWebSocketUrl`) + `VITE_TAURI_BACKEND_URL` fallback까지입니다. lifecycle/보안/패키징/릴리스 게이트는 미완료입니다.

- 2026-03-30 이후 9.1 보강: `src/bin/tauri_lifecycle_probe.rs` + `.github/workflows/tauri-lifecycle-poc.yml`로 오케스트레이터/Swagger lifecycle 기동·종료, 포트 충돌, 3OS 매트릭스 검증, 로그 아티팩트 수집 경로(`artifacts/tauri-lifecycle/<ts-os-pid>/`)를 자동화했습니다.

- Windows `cargo build`에서 `rustc.exe ... not applicable` 오류가 나면 rustup toolchain/component 재설치(`stable-x86_64-pc-windows-msvc`) 절차를 우선 적용합니다(상세 커맨드는 `README.md`/`AGENTS.md` 참조).
- WBS `1.2 OpenAPI/API 계약 재정렬`은 구현 라우트/파라미터와 OpenAPI 1:1 매핑 재검증(수동/자동/증적 기록) 완료로 재완료(READY) 상태입니다.
- 재완료 게이트는 `docs/openapi-wbs-1.2-recompletion-gate.md` 단일 체크리스트를 기준으로 판정하며, 현재 7개 항목 충족 상태를 유지합니다.
- 주간 점검에는 계약 드리프트 점검(구현↔OpenAPI path/param 대조, CI 정적 계약 점검 로그 확인 + 경로 파라미터 명칭 일치 여부 확인 + 기준 엔드포인트 세트 `/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}` 고정)을 필수 항목으로 포함하며, 회차 로그에는 CI 정적 점검 스크립트 산출물 링크/경로를 첨부합니다.

- OpenAPI 계약(`docs/openapi.yaml`, `docs/swagger/openapi.yaml`)은 Rust 목표 엔드포인트(`/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}`) 기준으로 정렬되어 있습니다.
- 상태 전이/에러 코드 설명은 OpenAPI enum을 단일 기준으로 유지합니다(상태: `queued|running|completed|failed|timeout|canceled|rejected`).
- 저장소는 Rust 전환 진행 중이며, 레거시 Python 코드는 현 저장소에 포함되어 있지 않습니다.
- 루트에는 Rust 실행 코드가 존재하며, 문서/프론트와 함께 Rust 구현 변경도 작업 대상입니다.
- `POST /jobs` 과부하 응답은 `429(queue_full|engine_full)`로 구분되며, 리젝션은 엔진/사유 라벨 카운트로 관측합니다.
- `/readyz`는 수용량 압박 시 `503 degraded`로 응답하며, `/metrics`에서 readiness/queue/rejection 스냅샷(JSON)을 노출합니다.
- STT는 `audio_ms` 길이 입력이 있을 때 처리율/버퍼/상하한 기반 timeout budget 산정식을 적용합니다.
- `POST /jobs`는 `audio_ms` query 파라미터(선택)를 받아 STT timeout budget 산정에 사용하며, 미지정 시 기본 timeout을 사용합니다.
- 오디오 전처리/변환 기본안은 Rust `symphonia` 크레이트 기반입니다.
- STT payload는 `audio_contract`를 포함하며 whisper-server는 변환 없이 추론만 수행한다는 계약(`conversion_required=false`)을 사용합니다.
- 운영 점검 시나리오(장애/복구/부하) runbook은 `docs/operations-runbook-scenarios.md`를 기준으로 사용합니다.
- 운영 runbook은 인증/인가/비밀관리 참조와 계량 롤백 기준, Owner/Approver, 보안 영향 검토 필드를 포함한 버전을 기준으로 사용합니다.
- 운영 점검 정례화 정책(주기/역할/합격 기준)은 `docs/operations/weekly-drill/README.md`에 고정되어 있습니다.
- 7.4.5 완료 조건 누적 추적은 `docs/operations/weekly-drill/STATUS.md`를 사용합니다.
- 모델 자산은 raw 데이터 미추적, `models/**/manifest.yml|yaml|json` 메타데이터만 추적합니다.
- `vendor/`는 소스 추적을 유지하고 빌드 산출물만 ignore 합니다.

## 우선 참조

- 기준 규칙: `AGENTS.md`
- 사용자 개요: `README.md`
- 사용자 조작 매뉴얼: `docs/user-operation-manual.md`
- 아키텍처 기준선: `docs/architecture.md`
- 전환 설계: `docs/rust-cpp-backend-rewrite-plan.md`
- 배포/자산 정책: `docs/deployment-asset-policy.md`
- `main.rs` 분할 가이드: `docs/main-rs-modularization-guide.md`
- 전환 실행 WBS: `TODO/TODO.md`
- 레거시 지침(필요 시): 별도 레거시 보관소/브랜치의 AGENTS 문서

## 에이전트 작업 규칙 (요약)

1. 상세 규칙은 `AGENTS.md` 단일 기준으로 유지
2. 문서 변경 시 `README.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` 동시 반영
3. 코드 작업 전 RTD Step 1~4, 커밋 전 RTD Step 5~18 수행(단계별 PASS/FAIL + 근거 기록)
4. 보안 리스크/롤백 경로 불명확 시 PASS 금지, Step 1~18 READY 전 커밋/PR 금지
5. 레거시 코드는 별도 보관소 기준 문서를 우선 적용
6. Rust 전환 상태를 과장하지 않고 현재/목표를 분리해 기술

## 검증 가이드

- 문서 작업: 링크/경로 검토 + `git status`
- 프론트 작업: `cd frontend && npm run build`
- Rust 도입 이후: `cargo test`, `cargo clippy`
