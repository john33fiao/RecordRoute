# RecordRoute
RecordRoute는 음성/문서 입력을 STT, 교정, 요약, 임베딩 검색으로 처리하는 통합 워크플로우 시스템입니다.

## 문서 안내
- 사용자/운영 가이드: 이 문서(`README.md`)
- 코딩 에이전트 기준 문서: `AGENTS.md`
- 에이전트별 요약 문서: `CLAUDE.md`, `GEMINI.md`

## 주요 기능
- 음성→텍스트 변환: OpenAI Whisper 기반 STT
- 텍스트 교정: Ollama 모델 기반 교정
- 구조화 요약: 6개 섹션 형식 회의록 요약
- 임베딩/검색: 벡터 검색 + 키워드 검색 + 유사 문서 추천
- 작업 상태 추적: HTTP progress + WebSocket 실시간 업데이트
- 히스토리/레지스트리: UUID 기반 파일 관리와 soft-delete
- 캐싱: 검색 결과 24시간 캐시

## 현재 아키텍처
- HTTP 서버 엔트리: `sttEngine/http_api/app.py`
- 핸들러/라우팅: `sttEngine/http_api/handler.py`
- 워크플로우 실행: `sttEngine/http_api/workflow.py`
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
├── db/
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
- `TRANSCRIBE_MODEL_WINDOWS`, `TRANSCRIBE_MODEL_UNIX`
- `SUMMARY_MODEL_WINDOWS`, `SUMMARY_MODEL_UNIX`
- `EMBEDDING_MODEL_WINDOWS`, `EMBEDDING_MODEL_UNIX`
- `TUNNEL_ENABLED`, `CLOUDFLARE_TUNNEL_TOKEN`
- `OBSIDIAN_MCP_ENABLED` 및 관련 변수

## API 요약

주요 GET:
- `/`, `/assets/*`, `/download/<uuid_or_path>`
- `/history`, `/tasks`, `/progress/<task_id>`
- `/search`, `/models`, `/cache/stats`, `/cache/cleanup`
- `/api/similarity-graph`, `/api/documents/metadata`

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
    "correct": "gpt-oss:20b",
    "summarize": "gpt-oss:20b"
  }
}
```

## 검색 API 계약 (요약)
- 엔드포인트: `GET /search`
- 주요 파라미터:
  - `query`, `limit`, `start_date`, `end_date`
  - `sort_by=similarity|date`, `sort_order=asc|desc`
  - `min_score`, `page`, `page_size`, `include_timing`
  - `file_type`, `status`, `status_task`
- 응답 핵심 필드:
  - `keywordMatches`, `similarDocuments`
  - `pagination`, `filters`, `cache`, `timing`, `contract_version`

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
- 구현 예정 사항: `TODO/TODO.md`
