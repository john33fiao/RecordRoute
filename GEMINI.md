# GEMINI.md - RecordRoute 에이전트 가이드

최신 기준 문서는 `AGENTS.md`입니다. 이 파일은 Gemini 작업 시 필요한 요약만 제공합니다.

## 우선 참고

- 구조/실행/API/테스트 기준: `AGENTS.md`
- 백엔드 핵심: `sttEngine/http_api/handler.py`, `sttEngine/http_api/workflow.py`
- 프론트 핵심: `frontend/src/*`

## 문서 점검

- 백로그는 `TODO/TODO.md`(실행 항목만 유지), 최신 점검 결과는 `TODO/STATUS_REVIEW.md`를 우선 참고합니다.

## 작업 체크리스트

1. 백엔드 라우트/스키마 변경 시 `frontend/src/api/client.ts`와 `frontend/src/api/types.ts` 동기화
2. 경로 처리 시 `sttEngine/http_api/paths.py` 유틸 사용
3. 실패 응답은 `error`, `error_code`, `retryable`, `failed_step` 규약 유지

## 권장 검증

- `pytest tests/http_api/test_workflow.py tests/http_api/test_search.py tests/server/test_queue.py tests/test_vocab_system.py`
- UI 변경 시: `cd frontend && npm run build`
