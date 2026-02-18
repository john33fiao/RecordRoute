# RecordRoute
RecordRoute는 음성/문서 입력을 STT, 교정, 요약, 임베딩 검색으로 처리하는 통합 워크플로우 시스템입니다.

## 문서 안내
- 사용자/운영 가이드: 이 문서(`README.md`)
- 코딩 에이전트 기준 문서: `AGENTS.md`
- 에이전트별 요약 문서: `CLAUDE.md`, `GEMINI.md`

## 주요 기능
- 음성→텍스트 변환: OpenAI Whisper 기반 STT
- 텍스트 교정: provider(ollama/llamacpp) 기반 교정
- 구조화 요약: 6개 섹션 형식 회의록 요약
- 임베딩/검색: 벡터 검색 + 키워드 검색 + 유사 문서 추천
- 작업 상태 추적: HTTP progress + WebSocket 실시간 업데이트
- 히스토리/레지스트리: UUID 기반 파일 관리와 soft-delete
- 캐싱: 검색 결과 24시간 캐시

## 현재 아키텍처
- HTTP 서버 엔트리: `sttEngine/http_api/app.py`
- 핸들러/라우팅: `sttEngine/http_api/handler.py`, `sttEngine/http_api/routes/*`
- 워크플로우 실행: `sttEngine/http_api/workflow.py`
- LLM/Embedding provider 추상화: `sttEngine/providers/*`, 호환 래퍼 `sttEngine/llm_provider.py`
- WebSocket 서버: `sttEngine/http_api/ws.py` (`ws://localhost:8765`)
- 서버 실행 래퍼: `sttEngine/server.py` (`python -m sttEngine.server`)
- 프론트엔드: React + Vite (`frontend/src`)
- 레거시 프론트 fallback: `frontend/legacy`

## 디렉토리 구조
```text
RecordRoute/
├── README.md
├── AGENTS.md
├── CLAUDE.md
├── GEMINI.md
├── .env.example
├── run.sh
├── run.bat
├── setup.sh
├── setup.bat
├── requirements.txt
├── frontend/
│   ├── src/
│   ├── legacy/
│   └── package.json
├── sttEngine/
│   ├── http_api/
│   ├── server/
│   ├── workflow/
│   ├── config.py
│   ├── embedding_pipeline.py
│   ├── vector_search.py
│   └── server.py
├── DB/
└── tests/
```

## 설치 및 실행

### 1) 자동 설정 (권장)

Windows:
```bash
setup.bat
run.bat
```

macOS/Linux:
```bash
./setup.sh
./run.sh
```

### 2) 수동 실행
```bash
python -m venv venv
./venv/bin/python -m pip install -r sttEngine/requirements.txt
./venv/bin/python -m pip install -r requirements.txt
./venv/bin/python -m sttEngine.server
```
Windows PowerShell:
```powershell
venv\Scripts\python.exe -m pip install -r sttEngine\requirements.txt
venv\Scripts\python.exe -m pip install -r requirements.txt
venv\Scripts\python.exe -m sttEngine.server
```

서버 기본 주소:
- HTTP: `http://localhost:8080`
- WebSocket: `ws://localhost:8765`

## 환경 변수
핵심 항목은 `.env.example`를 참고하세요.

주요 변수:
- `DB_FOLDER_PATH`: 데이터 저장 루트 (미설정 시 프로젝트의 `DB/`)
- `LLM_PROVIDER`: 교정/요약 LLM provider 선택 (`ollama` 기본, `llamacpp` 지원)
- 교정/요약 워크플로우는 provider 중립 옵션(`temperature`, `context_window`, `max_tokens`)을 사용하며 내부에서 provider별 키로 매핑됩니다.
- `EMBEDDING_PROVIDER`: 임베딩 provider 선택 (미지정 시 `LLM_PROVIDER` 상속)
- `LLAMA_CPP_COMMAND`: llama.cpp 실행 커맨드 (기본 `llama-cli`)
- `LLAMA_CPP_MODEL_PATH`: llama.cpp 기본 모델 경로 (`.gguf`)
- `LLAMA_CPP_TIMEOUT`: llama.cpp 호출 타임아웃(초, 기본 300)
- `LLM_TIMEOUT`: 교정/요약 LLM 호출 공통 타임아웃(초, 기본 300, 미설정 시 `OLLAMA_TIMEOUT` fallback)
- `TRANSCRIBE_MODEL_WINDOWS`, `TRANSCRIBE_MODEL_UNIX`
- `SUMMARY_MODEL_WINDOWS`, `SUMMARY_MODEL_UNIX`
- `EMBEDDING_MODEL_WINDOWS`, `EMBEDDING_MODEL_UNIX`
- `TUNNEL_ENABLED`, `CLOUDFLARE_TUNNEL_TOKEN`
- `OBSIDIAN_MCP_ENABLED` 및 관련 변수
- `RECORDROUTE_DESTRUCTIVE_API_SAFE_MODE` (기본 `true`): 파괴적 API 안전 모드
- `RECORDROUTE_DESTRUCTIVE_API_TOKEN`: 파괴적 API 공용 토큰
- `RECORDROUTE_DESTRUCTIVE_API_SESSION_ID`, `RECORDROUTE_DESTRUCTIVE_API_SESSION_TOKEN`: 세션 기반 보호 값
- `RECORDROUTE_SIMILARITY_GRAPH_CACHE_TTL_SECONDS`: 그래프 응답 캐시 TTL(초, 기본 300)
- `RECORDROUTE_SIMILARITY_GRAPH_CACHE_MAX_ENTRIES`: 그래프 응답 캐시 최대 엔트리 수(기본 24)
- `RECORDROUTE_SIMILARITY_INCREMENTAL_STATE_TTL_SECONDS`: 증분 유사도 상태 TTL(초, 기본 900)
- `RECORDROUTE_SIMILARITY_INCREMENTAL_STATE_MAX_ENTRIES`: 증분 유사도 상태 최대 엔트리 수(기본 8)

## API 요약

주요 GET:
- `/`, `/assets/*`, `/download/<uuid_or_path>`
- `/history`, `/tasks`, `/progress/<task_id>` (진행률 %, ETA, 표준 오류 payload 포함)
- `/search`, `/models`, `/cache/stats`, `/cache/cleanup`
- `/api/similarity-graph`, `/api/documents/metadata`
  - `/api/similarity-graph`는 `min_similarity/max_neighbors/max_nodes/sampling/neighbor_strategy(auto|exact|lsh)` + 필터(`doc_types`, `start_date`, `end_date`, `keyword`)를 지원
  - 응답 `meta`에는 `sampling`, `neighbor_strategy(requested/effective)`, `filters`, `incremental(...)` 진단 필드 포함

주요 POST:
- `/upload`, `/process`, `/cancel`, `/shutdown`
- `/reset`, `/update_filename`, `/update_stt_text`
- `/incremental_embedding`, `/check_existing_stt`, `/similar`
- `/delete`, `/delete_records`, `/reset_summary_embedding`, `/reset_all_tasks`

`/process` 요청 예시:
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

## 검색 API 계약 (요약)
- 엔드포인트: `GET /search`
- 주요 파라미터:
  - `query`, `limit`, `start_date`, `end_date`
  - `sort_by=similarity|date` (`uploaded_at`은 `date`로 별칭 처리, 그 외 값은 `similarity`로 정규화)
  - `sort_order=asc|desc` (그 외 값은 `desc`로 정규화)
  - `min_score` (0~1 범위를 벗어나면 `null` 처리)
  - `page`(기본 1), `page_size`(최소 1), `include_timing`
  - `file_type=audio|document|other` (허용값만 반영)
  - `status=completed|pending` (`done/success`→`completed`, `incomplete/todo`→`pending`), `status_task=stt|summary|embedding`
- 응답 핵심 필드:
  - `keywordMatches`, `similarDocuments`
  - `pagination`, `filters`, `cache`, `timing`
  - `contract_version` (`search-v2`)

## 단계별 CLI 실행

STT:
```bash
python sttEngine/workflow/transcribe.py [audio_dir] --model_size large-v3-turbo --language ko
```

교정:
```bash
python sttEngine/workflow/correct.py input.md --model gpt-oss:20b --temperature 0.0
```

요약:
```bash
python sttEngine/workflow/summarize.py input.md --model gpt-oss:20b --temperature 0.0
```

## 테스트
```bash
pytest
```
핵심 회귀:
```bash
pytest tests/http_api/test_workflow.py tests/http_api/test_search.py tests/server/test_queue.py tests/test_vocab_system.py
```

그래프 성능 계측(기본 100/500/1000 문서 기준선 자동 생성):
```bash
node frontend/scripts/benchmark-similarity-graph.mjs
```
대용량 구간(2k/5k) 포함:
```bash
node frontend/scripts/benchmark-similarity-graph.mjs --include-large
```
사용자 정의 구간/반복:
```bash
node frontend/scripts/benchmark-similarity-graph.mjs --dataset-sizes=100,500,1000,2000,5000 --iterations=3
```
결과물: `docs/perf/similarity-graph-baseline.json`, `docs/perf/similarity-graph-baseline.md`

실서버 응답시간까지 함께 계측하려면:
```bash
node frontend/scripts/benchmark-similarity-graph.mjs --api-base-url=http://localhost:8080
```

## Docker
기본:
```bash
docker compose up -d --build
```

NVIDIA GPU:
```bash
docker compose -f docker-compose.yml -f docker-compose.gpu.yml up -d --build
```

기본 포트:
- 앱: `8080`
- Ollama: `11434`

## 트러블슈팅
- Ollama 연결 오류: `ollama serve` 상태 및 모델 설치 확인
- FFmpeg 오류: 시스템 PATH 확인
- GPU 미사용: PyTorch/CUDA 설치 상태 확인
- 프론트 문제: `cd frontend && npm install && npm run build` 재실행

## 참고
- 진행 중 백로그(완료 항목 제외): `TODO/TODO.md`
- 최신 점검 요약: `TODO/STATUS_REVIEW.md`
