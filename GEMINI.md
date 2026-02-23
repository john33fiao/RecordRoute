# GEMINI.md - RecordRoute 작업 요약

최신 원본 기준은 `AGENTS.md`입니다. 이 문서는 실행 요약만 제공합니다.


> 문서 동기화: 2026-02-23 기준 운영 안정화(고정 worker 동시성 상한, connect+request timeout 관철, job_id 검증/로그 위생, queue depth guard) 반영 상태와 정렬됨.

## 핵심 컨텍스트

- 저장소는 Rust 전환 진행 중이며, 레거시 Python 코드는 현 저장소에 포함되어 있지 않습니다.
- 루트에는 Rust 실행 코드가 존재하며, 문서/프론트와 함께 Rust 구현 변경도 작업 대상입니다.
- 오디오 전처리/변환 기본안은 Rust `symphonia` 크레이트 기반입니다.

## 우선 참조

- 기준 규칙: `AGENTS.md`
- 사용자 개요: `README.md`
- 아키텍처 기준선: `docs/architecture.md`
- 전환 설계: `docs/rust-cpp-backend-rewrite-plan.md`
- 배포/자산 정책: `docs/deployment-asset-policy.md`
- 전환 실행 WBS: `TODO/TODO.md`
- 레거시 지침(필요 시): 별도 레거시 보관소/브랜치의 AGENTS 문서

## 에이전트 작업 규칙 (요약)

1. 상세 규칙은 `AGENTS.md` 단일 기준으로 유지
2. 문서 변경 시 `README.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` 동시 반영
3. 레거시 코드는 별도 보관소 기준 문서를 우선 적용
4. Rust 전환 상태를 과장하지 않고 현재/목표를 분리해 기술

## 검증 가이드

- 문서 작업: 링크/경로 검토 + `git status`
- 프론트 작업: `cd frontend && npm run build`
- Rust 도입 이후: `cargo test`, `cargo clippy`
