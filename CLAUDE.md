# CLAUDE.md - RecordRoute 작업 요약

이 문서는 요약본이며, 상세 기준은 `AGENTS.md`를 따릅니다.


> 문서 동기화: 2026-02-23 기준 운영 안정화(고정 worker 동시성 상한, connect+request timeout 관철, job_id 검증/로그 위생, queue depth guard, 429 reason/리젝션 메트릭 분리) 반영 상태와 정렬됨.

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

- OpenAPI 계약(`docs/openapi.yaml`, `docs/swagger/openapi.yaml`)은 Rust 목표 엔드포인트 기준으로 정렬되어 있습니다.
- 루트에 Rust 실행 코드(`Cargo.toml`)가 존재하며, 백엔드 전환 구현이 진행 중입니다.
- 운영 중 코드 기준은 `frontend/`(현행) + 루트 Rust 코드입니다.
- 오디오 변환 기본 전략은 외부 `ffmpeg` 호출이 아니라 Rust `symphonia` 크레이트 사용입니다.
- 잡 상태는 `queued|running|completed|failed|timeout|canceled|rejected`로 관리합니다.
- `POST /jobs` 과부하 응답은 `429(queue_full|engine_full)` 규약으로 구분되며 엔진/사유별 리젝션 카운트를 기록합니다.

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
