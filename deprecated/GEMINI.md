# GEMINI.md - RecordRoute 에이전트 가이드

> 레거시 문서 안내: 이 파일은 `deprecated/` 코드베이스 전용입니다.
> Rust 전환 기준/신규 작업 규칙은 루트 `AGENTS.md`를 우선 확인하세요.


최신 기준 문서는 `AGENTS.md`입니다. 이 파일은 Gemini 작업 시 필요한 요약만 제공합니다.

## 우선 참고

- 구조/실행/API/테스트 기준: `AGENTS.md`
- 사용자/운영 문서: `README.md`
- 작업 백로그: `TODO/TODO.md`, `TODO/GUI.md`, `TODO/llama-migration.md`, `TODO/rust-migration.md`, `TODO/DOCKER_OPENAPI_DEPLOY_PLAN.md`
- 백엔드 핵심: `sttEngine/http_api/handler.py`, `sttEngine/http_api/workflow.py`
- 프론트 핵심: `frontend/src/*`

## 문서 점검

- 원본 기준은 `AGENTS.md`이며, 이 파일은 요약만 유지합니다.
- `TODO/STATUS_REVIEW.md`는 현재 저장소 기준 파일이 아니므로 참조하지 않습니다.
- 에이전트 문서(`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`)를 수정할 때는 세 파일을 함께 동기화합니다.

## 작업 체크리스트

1. 백엔드 라우트/스키마 변경 시 `frontend/src/api/client.ts`와 `frontend/src/api/types.ts` 동기화
2. 경로 처리 시 `sttEngine/http_api/paths.py` 유틸 사용
3. 실패 응답은 `error`, `error_code`, `retryable`, `failed_step` 규약 유지

## 권장 검증

- `pytest tests/http_api/test_workflow.py tests/http_api/test_search.py tests/server/test_queue.py tests/test_vocab_system.py`
- UI 변경 시: `cd frontend && npm run build`

- llama.cpp provider는 `llama-cpp-python` in-process 모드를 기본으로 사용합니다.
- `LLAMA_CPP_MODEL_PATH` 기본값은 `./models/default_model.gguf`이며, 기존 HTTP/CLI 관련 환경 변수는 하위 호환용으로 유지됩니다.
- 로컬 GGUF 파일이 없으면 `HF_MODEL_REPO_ID`/`HF_MODEL_FILENAME`(선택 `HF_MODEL_REVISION`) 기반으로 Hugging Face 자동 다운로드를 시도하며 `HF_TOKEN`이 필요합니다.
- Hugging Face 캐시 경로는 `LLAMA_CPP_MODEL_CACHE_DIR`(기본 `./models`)로 제어합니다.
- Whisper 모델 경로는 `WHISPER_MODEL_DIR`로 오버라이드할 수 있고, 기본값은 프로젝트 루트 `./models`입니다.
