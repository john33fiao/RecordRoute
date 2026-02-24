# CLAUDE.md - RecordRoute 작업 요약

이 문서는 요약본이며, 상세 기준은 `AGENTS.md`를 따릅니다.


> 문서 동기화: 2026-03-30 기준 운영 안정화 + 운영 점검 정례화(7.4.1~7.4.5) 완료 상태와 정렬됨.

## 우선 확인 순서

1. `AGENTS.md`
2. `README.md`
3. `docs/architecture.md`
4. `docs/rust-cpp-backend-rewrite-plan.md`
5. `docs/deployment-asset-policy.md`
6. `docs/main-rs-modularization-guide.md` (`main.rs` 분할 가이드)
7. `TODO/TODO.md`
8. 레거시는 별도 보관소/브랜치에서만 취급 (현 저장소 `deprecated/` 없음)

## 현재 프로젝트 전제

- WBS `1.2 OpenAPI/API 계약 재정렬`은 구현 라우트/파라미터와 OpenAPI 1:1 매핑 재검증 전까지 재검토 상태로 관리합니다.
- 재완료 게이트는 `docs/openapi-wbs-1.2-recompletion-gate.md` 단일 체크리스트를 기준으로 판정하며, 7개 항목 미충족 시 `NOT READY`를 유지합니다.
- 주간 점검에는 계약 드리프트 점검(구현↔OpenAPI path/param 대조, CI 정적 계약 점검 로그 확인 + 경로 파라미터 명칭 일치 여부 확인 + 기준 엔드포인트 세트 `/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}` 고정)을 필수 항목으로 포함하며, 회차 로그에는 CI 정적 점검 스크립트 산출물 링크/경로를 첨부합니다.

- OpenAPI 계약(`docs/openapi.yaml`, `docs/swagger/openapi.yaml`)은 Rust 목표 엔드포인트(`/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}`) 기준으로 정렬되어 있습니다.
- 상태 전이/에러 코드 설명은 OpenAPI enum을 단일 기준으로 유지합니다(상태: `queued|running|completed|failed|timeout|canceled|rejected`).
- 루트에 Rust 실행 코드(`Cargo.toml`)가 존재하며, 백엔드 전환 구현이 진행 중입니다.
- 운영 중 코드 기준은 `frontend/`(현행) + 루트 Rust 코드입니다.
- 오디오 변환 기본 전략은 외부 `ffmpeg` 호출이 아니라 Rust `symphonia` 크레이트 사용입니다.
- 잡 상태는 `queued|running|completed|failed|timeout|canceled|rejected`로 관리합니다.
- `POST /jobs` 과부하 응답은 `429(queue_full|engine_full)` 규약으로 구분되며 엔진/사유별 리젝션 카운트를 기록합니다.
- `/readyz`는 수용량 압박 시 `503 degraded`로 응답하며, `/metrics`에서 readiness/queue/rejection 스냅샷(JSON)을 노출합니다.
- STT는 `audio_ms` 길이 입력이 있을 때 처리율/버퍼/상하한 기반 timeout budget 산정식을 적용합니다.
- `POST /jobs`는 `audio_ms` query 파라미터(선택)를 받아 STT timeout budget 산정에 사용하며, 미지정 시 기본 timeout을 사용합니다.
- STT payload는 `audio_contract`를 통해 Rust(`recordroute_symphonia`) 전처리 완료/변환 불필요(`conversion_required=false`) 계약을 명시합니다.
- 운영 점검 시나리오(장애/복구/부하) runbook은 `docs/operations-runbook-scenarios.md`를 기준으로 사용합니다.
- 운영 runbook은 인증/인가/비밀관리 참조와 계량 롤백 기준, Owner/Approver, 보안 영향 검토 필드를 포함한 버전을 기준으로 사용합니다.
- 운영 점검 정례화 정책(주기/역할/합격 기준)은 `docs/operations/weekly-drill/README.md`에 고정되어 있습니다.
- 7.4.5 완료 조건 누적 추적은 `docs/operations/weekly-drill/STATUS.md`를 사용합니다.
- 모델 자산은 raw 데이터 미추적, `models/**/manifest.yml|yaml|json` 메타데이터만 추적합니다.
- `vendor/`는 소스 추적을 유지하고 빌드 산출물만 ignore 합니다.

## 작업 체크리스트

- 문서 변경 시 `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `README.md` 동기화
- 코드 작업 전 RTD Step 1~4, 커밋 전 RTD Step 5~18 수행(단계별 PASS/FAIL + 근거 기록)
- 보안 리스크/롤백 경로 불명확 시 PASS 금지, Step 1~18 READY 전 커밋/PR 금지
- 목표/현황 구분 명확화(완료 표현 금지)
- 레거시 수정 시 해당 스코프 문서 지침 우선

## 검증

- 문서 변경: 링크/경로 유효성 + `git status`
- 프론트 변경: `cd frontend && npm run build`
- Rust 도입 이후: `cargo test`, `cargo clippy`
