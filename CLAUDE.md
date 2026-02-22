# CLAUDE.md - RecordRoute 작업 요약

이 문서는 요약본이며, 상세 기준은 `AGENTS.md`를 따릅니다.

## 우선 확인 순서

1. `AGENTS.md`
2. `README.md`
3. `docs/rust-cpp-backend-rewrite-plan.md`
4. (레거시 작업 시) `deprecated/AGENTS.md`

## 현재 프로젝트 전제

- Rust 백엔드는 전환 계획 단계이며 루트에 `Cargo.toml`이 아직 없습니다.
- 운영 중 코드 기준은 `frontend/`(현행) + `deprecated/`(레거시 참조)입니다.

## 작업 체크리스트

- 문서 변경 시 `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `README.md` 동기화
- 목표/현황 구분 명확화(완료 표현 금지)
- 레거시 수정 시 해당 스코프 문서 지침 우선

## 검증

- 문서 변경: 링크/경로 유효성 + `git status`
- 프론트 변경: `cd frontend && npm run build`
- Rust 도입 이후: `cargo test`, `cargo clippy`
