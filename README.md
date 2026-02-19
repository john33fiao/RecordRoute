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
├── requirements-ollama.txt
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

> 참고: `.env`에서 `LLM_PROVIDER`/`EMBEDDING_PROVIDER`를 `ollama`가 아닌 값으로 설정하면 setup/run 스크립트의 Ollama 점검/자동시작 단계는 자동으로 건너뜁니다.

### 2) 수동 실행
```bash
python -m venv venv
./venv/bin/python -m pip install -r sttEngine/requirements.txt
./venv/bin/python -m pip install -r requirements.txt
# Ollama provider 사용 시에만 추가 설치
./venv/bin/python -m pip install -r requirements-ollama.txt
./venv/bin/python -m sttEngine.server
```
Windows PowerShell:
```powershell
venv\Scripts\python.exe -m pip install -r sttEngine\requirements.txt
venv\Scripts\python.exe -m pip install -r requirements.txt
# Ollama provider 사용 시에만 추가 설치
venv\Scripts\python.exe -m pip install -r requirements-ollama.txt
venv\Scripts\python.exe -m sttEngine.server
```

서버 기본 주소:
- HTTP: `http://localhost:8080`
- WebSocket: `ws://localhost:8765`

## 환경 변수
핵심 항목은 `.env.example`를 참고하세요.

주요 변수:
- `DB_FOLDER_PATH`: 데이터 저장 루트 (미설정 시 프로젝트의 `DB/`)
- `LLM_PROVIDER`: 교정/요약 LLM provider 선택 (`ollama` 기본, `llamacpp`/`llama_cpp` 지원)
- 교정/요약 워크플로우는 provider 중립 옵션(`temperature`, `context_window`, `max_tokens`)을 사용하며 내부에서 provider별 키로 매핑됩니다.
- `EMBEDDING_PROVIDER`: 임베딩 provider 선택 (미지정 시 `LLM_PROVIDER` 상속)
- `LLM_BASE_URL`: llama.cpp(OpenAI 호환) LLM endpoint 기본 URL (기본 `http://localhost:8081`)
- `EMBEDDING_BASE_URL`: llama.cpp(OpenAI 호환) 임베딩 endpoint 기본 URL (기본 `http://localhost:8081`)
- `EMBEDDING_TIMEOUT`: 임베딩 provider 호출 타임아웃(초, 기본 `LLAMA_CPP_TIMEOUT` 또는 300)
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
  - `/models` 응답은 `models`(요청 provider 기준) + `models_by_provider` + `provider_status` + `default.provider`를 포함
  - 선택 쿼리: `provider=ollama|llamacpp` (미지정 시 `LLM_PROVIDER` 기준)
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
    "summarize": "gpt-oss:20b",
    "diarization_provider": "pyannote",
    "num_speakers": 2,
    "min_speakers": 1,
    "max_speakers": 4
  }
}
```


`/process` model_settings diarization 필드:
- `diarization_provider`: 화자 분리 provider 식별자. 미지정/빈 값이면 기본값 `"pyannote"`가 자동 적용됩니다.
- `num_speakers`: 전체 화자 수를 고정할 때 사용(정수 1~20).
- `min_speakers`, `max_speakers`: 화자 수 범위를 지정할 때 사용(각각 정수 1~20, `min_speakers <= max_speakers` 제약).

`/process` 실패 응답 계약:
- 필드: `error`, `error_code`, `retryable`, `failed_step`
- diarization 초안 오류 코드(`failed_step == "diarize"`):
  - `diarization_model_unavailable`
  - `diarization_timeout`
  - `diarization_invalid_audio`

`/process` diarization 동작(초안):
- `steps`에 `diarize` 포함 시 화자 분리 단계를 실행
- 오디오 입력은 `results.diarize = { status, input_file_type, duration, segments[] }` 구조로 응답
- STT 출력은 `*.segments.json` 사이드카 파일로 `segments[].{start,end,text,speaker}`를 저장하며, `/process` 응답에 `stt_segments`로 노출됩니다. diarization 실행 시 overlap 기준으로 화자 라벨을 align하고 미할당 구간은 기본 `SPEAKER_00`을 사용합니다.
- 텍스트/PDF 등 비오디오 입력은 표준 실패 대신 `results.diarize = { status: "skipped", reason: "non_audio_input", input_file_type, segments: [] }`로 스킵 처리
- diarization이 STT 이후 실패하면 워크플로우는 STT 텍스트를 유지하고, `results.diarize = { status: "failed", input_file_type, segments: [], error, error_code, retryable, failed_step }`를 반환하며 `stt_segments[].speaker`는 `null`로 비활성화됩니다.
- `steps`는 서버에서 소문자/중복 제거 정규화 후 처리(예: `" STT "`, `"stt"` → `"stt"`)

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

화자 분리 KPI 문서: [`docs/diarization/kpi.md`](docs/diarization/kpi.md)

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
기본(앱만 실행):
```bash
docker compose up -d --build
```

Ollama provider 포함 실행:
```bash
docker compose --profile ollama up -d --build
```

llama.cpp provider 포함 실행:
```bash
docker compose --profile llamacpp up -d --build
```

NVIDIA GPU:
```bash
docker compose -f docker-compose.yml -f docker-compose.gpu.yml up -d --build
```

기본 포트:
- 앱: `8080`
- Ollama(profile `ollama`): `11434`
- llama.cpp(profile `llamacpp`): `8081`

## 트러블슈팅
- Provider 연결 오류: `LLM_PROVIDER`/`EMBEDDING_PROVIDER`, `LLM_BASE_URL`/`EMBEDDING_BASE_URL` 설정을 확인
- Ollama 연결 오류: `ollama serve` 상태 및 모델 설치 확인
- FFmpeg 오류: 시스템 PATH 확인
- GPU 미사용: PyTorch/CUDA 설치 상태 확인
- 프론트 문제: `cd frontend && npm install && npm run build` 재실행

## 참고
- 진행 중 백로그(완료 항목 제외): `TODO/TODO.md`
- 최신 점검 요약: `TODO/STATUS_REVIEW.md`
