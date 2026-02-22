# GEMINI.md - RecordRoute 에이전트 가이드

이 문서는 요약본이며, 상세 기준은 `AGENTS.md`를 따릅니다.

## 핵심 정책

- Python 레거시 백엔드(`sttEngine/*`)는 deprecated 유지 대상
- 신규 백엔드 작업은 Rust 그린필드 구축 기준으로 수행
- 레거시 수정은 핫픽스/호환성/전환 브리지에 한정

## 우선 참고 문서

- `AGENTS.md`
- `README.md`
- `TODO/TODO.md`, `TODO/rust-migration.md`
- `docs/rust-migration-wbs.md`

## 권장 검증

- `pytest tests/http_api/test_workflow.py tests/http_api/test_search.py tests/server/test_queue.py tests/test_vocab_system.py`
- UI 변경 시: `cd frontend && npm run build`

## 문서 동기화

- 에이전트 문서(`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`)는 항상 같은 커밋으로 동기화합니다.
