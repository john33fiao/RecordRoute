# CLAUDE.md - RecordRoute 에이전트 가이드

이 문서는 중복을 줄이기 위해 `AGENTS.md`를 단일 기준으로 사용합니다.

## 사용 순서

1. 먼저 `AGENTS.md`를 읽고 현재 구조/엔드포인트/실행 명령을 따릅니다.
2. 이후 변경 범위에 따라 다음 파일을 우선 확인합니다.
   - 백엔드 라우팅: `sttEngine/http_api/handler.py`
   - 워크플로우: `sttEngine/http_api/workflow.py`
   - 프론트 API 연동: `frontend/src/api/client.ts`, `frontend/src/api/types.ts`

## 문서 점검

- 백로그는 `TODO/TODO.md`(실행 항목만 유지), 최신 점검 결과는 `TODO/STATUS_REVIEW.md`를 우선 참고합니다.

## 최소 검증

- `pytest tests/http_api/test_workflow.py tests/http_api/test_search.py tests/server/test_queue.py tests/test_vocab_system.py`
- 프론트 변경 시: `cd frontend && npm run build`

## 주의 사항

- 서버 엔트리는 `sttEngine/http_api/app.py`이며, `sttEngine/server.py`는 래퍼입니다.
- 프론트 주 코드베이스는 `frontend/src`입니다. `frontend/legacy`는 fallback 용도입니다.
- API/스키마 변경 시 `AGENTS.md`도 함께 갱신해 문서와 코드 드리프트를 막습니다.
