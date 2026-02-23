# GEMINI.md - RecordRoute 작업 요약

최신 원본 기준은 `AGENTS.md`입니다. 이 문서는 실행 요약만 제공합니다.


> 문서 동기화: 2026-02-23 기준 운영 안정화(고정 worker 동시성 상한, connect+request timeout 관철, 길이/예산 기반 job timeout 산정식, job_id 검증/로그 위생, queue depth guard, 429 reason/리젝션 메트릭 분리, readyz degraded + /metrics 노출) 반영 상태와 정렬됨.

## 핵심 컨텍스트

- OpenAPI 계약(`docs/openapi.yaml`, `docs/swagger/openapi.yaml`)은 Rust 목표 엔드포인트 기준으로 정렬되어 있습니다.
- 저장소는 Rust 전환 진행 중이며, 레거시 Python 코드는 현 저장소에 포함되어 있지 않습니다.
- 루트에는 Rust 실행 코드가 존재하며, 문서/프론트와 함께 Rust 구현 변경도 작업 대상입니다.
- `POST /jobs` 과부하 응답은 `429(queue_full|engine_full)`로 구분되며, 리젝션은 엔진/사유 라벨 카운트로 관측합니다.
- `/readyz`는 수용량 압박 시 `503 degraded`로 응답하며, `/metrics`에서 readiness/queue/rejection 스냅샷(JSON)을 노출합니다.
- STT는 `audio_ms` 길이 입력이 있을 때 처리율/버퍼/상하한 기반 timeout budget 산정식을 적용합니다.
- 오디오 전처리/변환 기본안은 Rust `symphonia` 크레이트 기반입니다.

## 우선 참조

- 기준 규칙: `AGENTS.md`
- 사용자 개요: `README.md`
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
