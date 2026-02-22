# CLAUDE.md - RecordRoute 에이전트 가이드

이 문서는 요약본이며, 원본 기준은 항상 `AGENTS.md`입니다.

## 핵심 정책

1. 기존 Python 백엔드(`sttEngine/*`)는 deprecated 유지 구역입니다.
2. 백엔드 신규 구현은 Rust 신규 백엔드(그린필드) 기준으로 진행합니다.
3. 레거시 코드는 핫픽스/호환성/브리지 목적 외 신규 기능 개발에 사용하지 않습니다.

## 작업 전 확인

- `AGENTS.md`
- `README.md`
- `TODO/TODO.md`, `TODO/rust-migration.md`
- `docs/rust-migration-wbs.md`

## 최소 검증

- `pytest tests/http_api/test_workflow.py tests/http_api/test_search.py tests/server/test_queue.py tests/test_vocab_system.py`
- 프론트 변경 시: `cd frontend && npm run build`

## 문서 동기화 규칙

- `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` 수정은 같은 커밋에서 동기화합니다.
