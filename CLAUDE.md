# CLAUDE.md - RecordRoute 에이전트 가이드

이 문서는 중복을 줄이기 위해 `AGENTS.md`를 단일 기준으로 사용합니다.

## 사용 순서

1. 먼저 `AGENTS.md`를 읽고 현재 구조/엔드포인트/실행 명령을 따릅니다.
2. 사용자/운영 동작 확인이 필요하면 `README.md`를 함께 확인합니다.
3. 작업 착수 전 백로그를 확인합니다.
   - 공통: `TODO/TODO.md`
   - 트랙별: `TODO/GUI.md`, `TODO/llama-migration.md`, `TODO/rust-migration.md`, `TODO/DOCKER_OPENAPI_DEPLOY_PLAN.md`
4. 이후 변경 범위에 따라 다음 파일을 우선 확인합니다.
   - 백엔드 라우팅: `sttEngine/http_api/handler.py`
   - 워크플로우: `sttEngine/http_api/workflow.py`
   - 프론트 API 연동: `frontend/src/api/client.ts`, `frontend/src/api/types.ts`

## 문서 점검

- 에이전트 원본 규칙은 `AGENTS.md`를 기준으로 유지합니다.
- `TODO/STATUS_REVIEW.md`는 현재 저장소 기준 파일이 아니므로 참조하지 않습니다.
- 에이전트 문서(`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`)를 수정할 때는 세 파일을 함께 동기화합니다.

## 최소 검증

- `pytest tests/http_api/test_workflow.py tests/http_api/test_search.py tests/server/test_queue.py tests/test_vocab_system.py`
- 프론트 변경 시: `cd frontend && npm run build`

## 주의 사항

- 서버 엔트리는 `sttEngine/http_api/app.py`이며, `sttEngine/server.py`는 래퍼입니다.
- 프론트 주 코드베이스는 `frontend/src`입니다. `frontend/legacy`는 fallback 용도입니다.
- API/스키마 변경 시 `AGENTS.md`도 함께 갱신해 문서와 코드 드리프트를 막습니다.

- llama.cpp provider는 `llama-cpp-python` in-process 모드가 기본입니다.
- `LLAMA_CPP_MODEL_PATH` 기본값은 `./models/default_model.gguf`이며, `LLM_BASE_URL`/`EMBEDDING_BASE_URL`/`LLAMA_CPP_COMMAND`는 하위 호환용으로만 유지됩니다.
- Whisper 모델 경로는 `WHISPER_MODEL_DIR`로 오버라이드할 수 있고, 기본값은 프로젝트 루트 `./models`입니다.
