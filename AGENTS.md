# AGENTS.md - RecordRoute 코딩 에이전트 가이드

이 문서는 RecordRoute 코드베이스를 수정하는 에이전트를 위한 최신 기준 문서입니다.

## 0. 문서 우선순위
- 사용자/운영 안내는 `README.md`를 기준으로 유지
- 에이전트 개발 규칙은 `AGENTS.md`를 기준으로 유지
- `CLAUDE.md`, `GEMINI.md`는 `AGENTS.md`의 요약/진입점 역할

문서 동기화 규칙:
- API, 경로, 실행 방식이 바뀌면 `README.md`와 `AGENTS.md`를 함께 갱신
- 세부 구현 변경(파일 이동, 모듈 분리) 시 `AGENTS.md`를 먼저 갱신

## 1. 현재 구조 요약
- 백엔드 엔트리포인트: `sttEngine/http_api/app.py`
  - `ThreadingHTTPServer` + `UploadHandler`
  - WebSocket 서버는 별도 스레드에서 실행 (`sttEngine/http_api/ws.py`, 포트 `8765`)
- 런처 래퍼: `sttEngine/server.py` (`python -m sttEngine.server`)
- 핵심 HTTP 라우팅: `sttEngine/http_api/handler.py` + 라우트 모듈(`sttEngine/http_api/routes/*`)
- 워크플로우 실행: `sttEngine/http_api/workflow.py`
- 프론트엔드: React + Vite (`frontend/src/*`)
- 레거시 UI: `frontend/legacy/*` (프론트 빌드 실패 시 fallback)

## 2. 빠른 실행 명령
- macOS/Linux 설정: `./setup.sh`
- macOS/Linux 실행: `./run.sh`
- Windows 설정: `setup.bat`
- Windows 실행: `run.bat`

개별 실행:
- 백엔드: `venv/bin/python -m sttEngine.server` (Windows: `venv\\Scripts\\python.exe -m sttEngine.server`)
- 프론트 빌드: `cd frontend && npm install && npm run build`

## 3. 데이터/경로 규칙
- DB 루트는 `DB_FOLDER_PATH` 환경변수 우선, 없으면 프로젝트 루트의 `DB/`
- 주요 파일:
  - 업로드 히스토리: `DB/upload_history.json`
  - 파일 레지스트리(UUID 매핑): `DB/file_registry.json`
  - 임베딩 인덱스: `DB/vector_store/index.json`
- 경로 문자열은 `DB/...` alias 형식으로 저장/정규화됨
  - 관련 모듈: `sttEngine/config.py`, `sttEngine/http_api/paths.py`

## 4. API 계약 (현재 코드 기준)

GET
- `/`, `/assets/*`, `/download/<uuid_or_path>`
- `/history`, `/tasks`, `/progress/<task_id>` (진행률 %, ETA, 표준 오류 payload 포함)
- `/file_search`, `/search`
  - `/search` 파라미터 정규화: `sort_by(similarity|date, uploaded_at→date)`, `sort_order(asc|desc)`, `status(completed|pending, done/success/incomplete/todo 별칭 지원)`, `status_task(stt|summary|embedding)`
  - `min_score`는 0~1 범위만 유효, 응답은 항상 `contract_version: search-v2` 포함
- `/api/similarity-graph`, `/api/documents/metadata`
- `/similar/<uuid_or_path>`, `/models`
- `/cache/stats`, `/cache/cleanup`

POST
- `/upload`, `/process`, `/cancel`, `/shutdown`
- `/reset`, `/update_filename`, `/incremental_embedding`
- `/check_existing_stt`, `/update_stt_text`
- `/reset_summary_embedding`, `/reset_all_tasks`
- `/similar`, `/delete`, `/delete_records`

## 5. /process 요청 스키마 핵심
요청 바디 예시:
```json
{
  "file_path": "DB/uploads/<uuid>/sample.m4a",
  "steps": ["stt", "correct", "summary"],
  "record_id": "...",
  "task_id": "...",
  "model_settings": {
    "whisper": "large-v3-turbo",
    "language": "ko",
    "device": "auto",
    "provider": "ollama",
    "correct": "gpt-oss:20b",
    "summarize": "gpt-oss:20b"
  }
}
```

주의:
- 백엔드 기준 요약 step 키는 `summary`
- STT 모델 키는 `whisper`
- `model_settings.provider`(또는 `llm_provider`)로 교정/요약 LLM provider 선택 가능 (`ollama` 기본, `llamacpp` 지원)
- 실패 응답 필드: `error`, `error_code`, `retryable`, `failed_step`
- 진행률 응답(`/progress/<task_id>`, WebSocket)은 `progress_percent`, `eta_seconds`, `error`(표준 오류 카드용 객체) 포함

## 6. 수정 시 우선 확인할 파일
- 라우트/핸들러: `sttEngine/http_api/handler.py`
- 워크플로우: `sttEngine/http_api/workflow.py`
- 에러 매핑: `sttEngine/server/services/errors.py`
- 히스토리/레지스트리: `sttEngine/http_api/history.py`, `sttEngine/http_api/registry.py`, `sttEngine/http_api/records.py`
- 검색/캐시: `sttEngine/http_api/search.py`, `sttEngine/vector_search.py`, `sttEngine/search_cache.py`
- 프론트 API: `frontend/src/api/client.ts`, `frontend/src/api/types.ts`

## 7. 테스트
기본:
- `pytest`

핵심 회귀 테스트:
- `pytest tests/http_api/test_workflow.py`
- `pytest tests/http_api/test_search.py`
- `pytest tests/server/test_queue.py`
- `pytest tests/test_vocab_system.py`
- 그래프 성능 기준선 생성: `node frontend/scripts/benchmark-similarity-graph.mjs`
  - 출력: `docs/perf/similarity-graph-baseline.json`, `docs/perf/similarity-graph-baseline.md`
  - 실서버 응답 포함: `node frontend/scripts/benchmark-similarity-graph.mjs --api-base-url=http://localhost:8080`

## 8. 에이전트 작업 규칙
- 라우트 추가/변경 시 백엔드 + 프론트 API 클라이언트 + 타입 + 테스트를 함께 업데이트
- 경로는 문자열 조합 대신 `normalize_record_path`, `resolve_record_path`, `to_record_path` 사용
- 워크플로우 에러는 `map_workflow_exception()` 규약을 따라 `error_code/retryable/failed_step` 유지
- 프론트는 `frontend/src`가 기준이며 `frontend/legacy`는 fallback 유지 목적
- 런타임 영향이 있는 변경은 문서(`README.md`, `AGENTS.md`)에 즉시 반영
