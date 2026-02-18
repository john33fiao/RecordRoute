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
  - 파일/정적 서빙: `sttEngine/http_api/routes/file_routes.py`
  - 유사문서 응답 조합: `sttEngine/http_api/routes/similar_documents.py`
- 워크플로우 실행: `sttEngine/http_api/workflow.py`
- Provider 추상화: `sttEngine/providers/*` + 호환 래퍼 `sttEngine/llm_provider.py`
- 프론트엔드: React + Vite (`frontend/src/*`)
- 레거시 UI: `frontend/legacy/*` (프론트 빌드 실패 시 fallback)

## 2. 빠른 실행 명령
- macOS/Linux 설정: `./setup.sh`
- macOS/Linux 실행: `./run.sh`
- Windows 설정: `setup.bat`
- Windows 실행: `run.bat`

개별 실행:
- 백엔드: `venv/bin/python -m sttEngine.server` (Windows: `venv\\Scripts\\python.exe -m sttEngine.server`)
- Ollama provider 사용 시 추가 의존성: `pip install -r requirements-ollama.txt`
- `.env`의 `LLM_PROVIDER`/`EMBEDDING_PROVIDER`가 `ollama`가 아니면 setup/run 스크립트의 Ollama 점검 단계는 자동 skip
- 프론트 빌드: `cd frontend && npm install && npm run build`

Docker Compose provider profile:
- 앱 단독: `docker compose up -d --build`
- Ollama 포함: `docker compose --profile ollama up -d --build`
- llama.cpp 포함: `docker compose --profile llamacpp up -d --build`

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
- 검색 시 쿼리 임베딩과 문서 임베딩의 벡터 차원이 다르면 해당 문서는 자동 제외되며 로그에 차원 불일치가 기록됩니다.
- `/api/similarity-graph`, `/api/documents/metadata`
  - `/api/similarity-graph`는 `min_similarity/max_neighbors/max_nodes/sampling/neighbor_strategy(auto|exact|lsh)` + 필터(`doc_types`, `start_date`, `end_date`, `keyword`)를 지원
  - 응답 `meta`는 `sampling`, `neighbor_strategy(requested/effective)`, `filters`, `incremental(...)` 진단 정보를 포함
- `/similar/<uuid_or_path>`, `/models`
  - `/models` 응답은 `models`(요청 provider 기준) + `models_by_provider` + `provider_status` + `default.provider`를 포함
  - 선택 쿼리: `provider=ollama|llamacpp` (미지정 시 `LLM_PROVIDER` 기준)
- `/cache/stats`, `/cache/cleanup`

POST
- `/upload`, `/process`, `/cancel`, `/shutdown`
- 파괴적 API 보호: `/shutdown`, `/delete`, `/delete_records`, `/reset`, `/reset_all_tasks`, `/reset_summary_embedding`는 기본 안전 모드(`RECORDROUTE_DESTRUCTIVE_API_SAFE_MODE=true`)에서 토큰(`RECORDROUTE_DESTRUCTIVE_API_TOKEN`) 또는 세션(`RECORDROUTE_DESTRUCTIVE_API_SESSION_ID` + `RECORDROUTE_DESTRUCTIVE_API_SESSION_TOKEN`) 필요
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
- 교정/요약 워크플로우 옵션은 provider 중립 키(`temperature`, `context_window`, `max_tokens`)를 우선 사용하고 provider별 키로 매핑
- 실패 응답 필드: `error`, `error_code`, `retryable`, `failed_step`
- 진행률 응답(`/progress/<task_id>`, WebSocket)은 `progress_percent`, `eta_seconds`, `error`(표준 오류 카드용 객체) 포함

## 6. 수정 시 우선 확인할 파일
- 라우트/핸들러: `sttEngine/http_api/handler.py`, `sttEngine/http_api/routes/file_routes.py`
- 워크플로우: `sttEngine/http_api/workflow.py`
- Provider: `sttEngine/providers/*`, `sttEngine/llm_provider.py`
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
  - 대용량 구간(2k/5k) 포함: `node frontend/scripts/benchmark-similarity-graph.mjs --include-large`
  - 사용자 정의 구간/반복: `node frontend/scripts/benchmark-similarity-graph.mjs --dataset-sizes=100,500,1000,2000,5000 --iterations=3`
  - 출력: `docs/perf/similarity-graph-baseline.json`, `docs/perf/similarity-graph-baseline.md`
  - 실서버 응답 포함: `node frontend/scripts/benchmark-similarity-graph.mjs --api-base-url=http://localhost:8080`

## 8. 에이전트 작업 규칙
- 라우트 추가/변경 시 백엔드 + 프론트 API 클라이언트 + 타입 + 테스트를 함께 업데이트
- 경로는 문자열 조합 대신 `normalize_record_path`, `resolve_record_path`, `to_record_path` 사용
- 워크플로우 에러는 `map_workflow_exception()` 규약을 따라 `error_code/retryable/failed_step` 유지
- 프론트는 `frontend/src`가 기준이며 `frontend/legacy`는 fallback 유지 목적
- 런타임 영향이 있는 변경은 문서(`README.md`, `AGENTS.md`)에 즉시 반영

## 9. 스킬 호출 키워드
- 배포 준비 워크플로우 스킬 이름: `배포준비`
- 호출 키워드(alias): `RTD`
- 스킬 본문 경로: `.agents/skills/RTD.md`
- 인터페이스/기본 프롬프트 설정: `.agents/skills/openai.yaml`
