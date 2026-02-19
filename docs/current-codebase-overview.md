# RecordRoute 코드베이스 개요

이 문서는 **현재 코드베이스 구조와 내부 계약**을 정리한 개발/유지보수용 문서입니다.
사용자 설치/운영 가이드는 `README.md`를 참고하세요.

## 1. 시스템 개요
- 백엔드 엔트리포인트: `sttEngine/http_api/app.py`
  - `ThreadingHTTPServer` + `UploadHandler`
  - WebSocket 서버는 별도 스레드(`sttEngine/http_api/ws.py`, 기본 포트 `8765`)
- 런처 래퍼: `sttEngine/server.py` (`python -m sttEngine.server`)
- 핵심 HTTP 라우팅: `sttEngine/http_api/handler.py`, `sttEngine/http_api/routes/*`
- 워크플로우 실행: `sttEngine/http_api/workflow.py`
- Provider 추상화: `sttEngine/providers/*`, 호환 래퍼 `sttEngine/llm_provider.py`
- 프론트엔드: React + Vite (`frontend/src/*`)
- 레거시 UI fallback: `frontend/legacy/*`

## 2. 디렉토리 구조(요약)
```text
RecordRoute/
├── README.md
├── AGENTS.md
├── CLAUDE.md
├── GEMINI.md
├── docs/
│   └── current-codebase-overview.md
├── frontend/
│   ├── src/
│   └── legacy/
├── sttEngine/
│   ├── http_api/
│   ├── providers/
│   ├── workflow/
│   └── server.py
├── DB/
└── tests/
```

## 3. 데이터/경로 규칙
- DB 루트: `DB_FOLDER_PATH` 환경변수 우선, 없으면 프로젝트 루트 `DB/`
- 주요 파일:
  - 업로드 히스토리: `DB/upload_history.json`
  - 파일 레지스트리(UUID 매핑): `DB/file_registry.json`
  - 임베딩 인덱스: `DB/vector_store/index.json`
- 경로 문자열은 `DB/...` alias로 저장/정규화
  - 관련 모듈: `sttEngine/config.py`, `sttEngine/http_api/paths.py`

## 4. API 계약(요약)

### GET
- `/`, `/assets/*`, `/download/<uuid_or_path>`
- `/history`, `/tasks`, `/progress/<task_id>`
- `/segments/<file_identifier>`
- `/file_search`, `/search`
  - `/search`: `sort_by(similarity|date)`, `sort_order(asc|desc)`, `status(completed|pending)`
  - 응답은 `contract_version: search-v2` 포함
- `/api/similarity-graph`, `/api/documents/metadata`
- `/similar/<uuid_or_path>`, `/models`
- `/cache/stats`, `/cache/cleanup`, `/metrics/workflow`

### POST
- `/upload`, `/process`, `/cancel`, `/shutdown`
- `/reset`, `/update_filename`, `/incremental_embedding`
- `/check_existing_stt`, `/update_stt_text`
- `/reset_summary_embedding`, `/reset_all_tasks`
- `/similar`, `/delete`, `/delete_records`

### 파괴적 API 보호
- 기본 안전 모드: `RECORDROUTE_DESTRUCTIVE_API_SAFE_MODE=true`
- 보호 대상 API: `/shutdown`, `/delete`, `/delete_records`, `/reset`, `/reset_all_tasks`, `/reset_summary_embedding`
- 토큰 또는 세션 기반 인증 필요:
  - `RECORDROUTE_DESTRUCTIVE_API_TOKEN`
  - 또는 `RECORDROUTE_DESTRUCTIVE_API_SESSION_ID` + `RECORDROUTE_DESTRUCTIVE_API_SESSION_TOKEN`

## 5. /process 요청 스키마 핵심
예시:
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
    "summarize": "gpt-oss:20b",
    "diarization_provider": "pyannote",
    "num_speakers": 2,
    "min_speakers": 1,
    "max_speakers": 4
  }
}
```

주의사항:
- 요약 step 키는 `summary`
- STT 모델 키는 `whisper`
- `steps`는 소문자/중복 제거 정규화 후 처리(순서 유지)
- 실패 응답 표준 필드: `error`, `error_code`, `retryable`, `failed_step`
- STT 세그먼트 스키마: `{start,end,text,speaker}`

## 6. 핵심 구현 파일
- 라우트/핸들러: `sttEngine/http_api/handler.py`, `sttEngine/http_api/routes/file_routes.py`
- 워크플로우: `sttEngine/http_api/workflow.py`
- Provider: `sttEngine/providers/*`, `sttEngine/llm_provider.py`
- 에러 매핑: `sttEngine/server/services/errors.py`
- 검색/캐시: `sttEngine/http_api/search.py`, `sttEngine/vector_search.py`, `sttEngine/search_cache.py`
- 프론트 API: `frontend/src/api/client.ts`, `frontend/src/api/types.ts`

## 7. 테스트
- 기본: `pytest`
- 핵심 회귀:
  - `pytest tests/http_api/test_workflow.py`
  - `pytest tests/http_api/test_search.py`
  - `pytest tests/server/test_queue.py`
  - `pytest tests/test_vocab_system.py`
- 그래프 성능 기준선:
  - `node frontend/scripts/benchmark-similarity-graph.mjs`
