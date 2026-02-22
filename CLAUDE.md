# CLAUDE.md - RecordRoute 작업 요약

이 문서는 요약본이며, 상세 기준은 `AGENTS.md`를 따릅니다.


> 문서 동기화: 2026-02-22 기준 Phase A(최소 HTTP 서버/헬스 엔드포인트) 반영 상태와 정렬됨.

## 우선 확인 순서

1. `AGENTS.md`
2. `README.md`
3. `docs/architecture.md`
4. `docs/rust-cpp-backend-rewrite-plan.md`
5. `docs/deployment-asset-policy.md`
6. `TODO/TODO.md`
7. 레거시는 별도 보관소/브랜치에서만 취급 (현 저장소 `deprecated/` 없음)

## 현재 프로젝트 전제

- 루트에 Rust 실행 코드(`Cargo.toml`)가 존재하며, 백엔드 전환 구현이 진행 중입니다.
- 운영 중 코드 기준은 `frontend/`(현행) + 루트 Rust 코드입니다.
- 오디오 변환 기본 전략은 외부 `ffmpeg` 호출이 아니라 Rust `symphonia` 크레이트 사용입니다.

## 작업 체크리스트

- 문서 변경 시 `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `README.md` 동기화
- 목표/현황 구분 명확화(완료 표현 금지)
- 레거시 수정 시 해당 스코프 문서 지침 우선

## 검증

- 문서 변경: 링크/경로 유효성 + `git status`
- 프론트 변경: `cd frontend && npm run build`
- Rust 도입 이후: `cargo test`, `cargo clippy`
