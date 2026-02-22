# GEMINI.md - RecordRoute 작업 요약

최신 원본 기준은 `AGENTS.md`입니다. 이 문서는 실행 요약만 제공합니다.

## 핵심 컨텍스트

- 저장소는 Rust 전환 진행 중이며, 레거시 Python 코드는 `deprecated/`에 보관됩니다.
- 현재 루트에는 Rust 실행 코드가 아직 없으므로, 문서/프론트/전환 설계 중심으로 작업합니다.
- 오디오 전처리/변환 기본안은 Rust `symphonia` 크레이트 기반입니다.

## 우선 참조

- 기준 규칙: `AGENTS.md`
- 사용자 개요: `README.md`
- 전환 설계: `docs/rust-cpp-backend-rewrite-plan.md`
- 전환 실행 WBS: `docs/rust-migration-wbs.md`
- 레거시 지침(필요 시): `deprecated/AGENTS.md`

## 에이전트 작업 규칙 (요약)

1. 상세 규칙은 `AGENTS.md` 단일 기준으로 유지
2. 문서 변경 시 `README.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` 동시 반영
3. 레거시 경로 수정 시 `deprecated/` 스코프 문서 우선 적용
4. Rust 전환 상태를 과장하지 않고 현재/목표를 분리해 기술

## 검증 가이드

- 문서 작업: 링크/경로 검토 + `git status`
- 프론트 작업: `cd frontend && npm run build`
- Rust 도입 이후: `cargo test`, `cargo clippy`
