# RecordRoute
RecordRoute는 음성/문서 입력을 STT, 교정, 요약, 임베딩 검색으로 처리하는 워크플로우 서비스입니다.

## 문서 안내
- 설치/모델 설정/사용 방법: 이 문서(`README.md`)
- 현재 코드베이스 구조/개발 참고: `docs/current-codebase-overview.md`
- llama.cpp 실사용 가이드: `docs/llama-guide.md`
- 코딩 에이전트 기준 문서: `AGENTS.md`
- 에이전트 요약 문서: `CLAUDE.md`, `GEMINI.md`

## 1) 빠른 설치 및 실행

### macOS/Linux (권장)
```bash
./setup.sh
./run.sh
```

### Windows (권장)
```bat
setup.bat
run.bat
```

> `.env`에서 `LLM_PROVIDER`/`EMBEDDING_PROVIDER`를 `ollama`가 아닌 값으로 설정하면 setup/run 스크립트의 Ollama 점검/자동시작 단계는 자동으로 건너뜁니다.

## 2) 수동 설치

## 2-A) Docker 분리 배포 (backend + frontend)

```bash
docker compose up -d --build
```

프로필 사용:
```bash
# Ollama 포함
docker compose --profile ollama up -d --build

# llama.cpp 포함
docker compose --profile llamacpp up -d --build
```

기본 포트:
- 프론트엔드: `http://localhost:3000`
- 백엔드 API: `http://localhost:8080`
- WebSocket: `ws://localhost:8765`

프론트 빌드 변수(Compose build args):
- `VITE_API_BASE_URL` (기본 `/api`, 프론트 Nginx가 backend:8080으로 프록시)
- `VITE_WS_URL` (기본 비움. 비어 있으면 브라우저 origin 기준 `/ws` 사용)

백엔드 헬스체크:
- `GET /health` → `{ "status": "ok" }`

프론트 Nginx 프록시 경로:
- `/api/*` -> `backend:8080/*`
- `/ws` -> `backend:8765` (WebSocket 업그레이드)


### Python 가상환경 + 의존성 설치
macOS/Linux:
```bash
python -m venv venv
./venv/bin/python -m pip install -r sttEngine/requirements.txt
./venv/bin/python -m pip install -r requirements.txt
# Ollama provider를 사용할 때만 추가 설치
./venv/bin/python -m pip install -r requirements-ollama.txt
```

Windows PowerShell:
```powershell
python -m venv venv
venv\Scripts\python.exe -m pip install -r sttEngine\requirements.txt
venv\Scripts\python.exe -m pip install -r requirements.txt
# Ollama provider를 사용할 때만 추가 설치
venv\Scripts\python.exe -m pip install -r requirements-ollama.txt
```

### 서버 실행
macOS/Linux:
```bash
./venv/bin/python -m sttEngine.server
```

Windows PowerShell:
```powershell
venv\Scripts\python.exe -m sttEngine.server
```

기본 접속 주소:
- HTTP(API): `http://localhost:8080`
- WebSocket: `ws://localhost:8765`
- (Docker 분리 배포) Frontend: `http://localhost:3000`
- Windows `run.bat` 실행 시 `8080` 바인딩이 불가하면 자동으로 `18080`으로 대체됩니다.

## 3) 모델/Provider 설정 방법

모델 설정은 `.env` 파일에서 관리합니다(`.env.example` 참고).

### 3-1. LLM/Embedding Provider 선택
- `LLM_PROVIDER`: 교정/요약 provider (`ollama` 기본, `llamacpp`/`llama_cpp` 지원)
- `EMBEDDING_PROVIDER`: 임베딩 provider (미지정 시 `LLM_PROVIDER` 상속)

### 3-2. Ollama 사용 시
1. Ollama 서버 실행
2. 사용할 모델 pull (예: `gpt-oss:20b`)
3. 필요 시 `.env`에 Ollama 주소/타임아웃 설정

### 3-3. llama.cpp(In-process / `llama-cpp-python`) 사용 시
- `pip install -r requirements.txt`로 `llama-cpp-python`을 함께 설치합니다.
- `LLAMA_CPP_MODEL_PATH` (`.gguf` 모델 경로, 기본값 `./models/default_model.gguf`)
- 선택 성능 튜닝: `LLAMA_CPP_N_CTX`, `LLAMA_CPP_N_THREADS`, `LLAMA_CPP_N_BATCH`, `LLAMA_CPP_N_GPU_LAYERS`, `LLAMA_CPP_CHAT_FORMAT`
- 하위 호환용으로 `LLM_BASE_URL`, `EMBEDDING_BASE_URL`, `LLAMA_CPP_COMMAND`를 남겨둘 수 있으나, in-process 모드에서는 무시되며 warning 로그가 남습니다.

### 3-4. 워크플로우 모델 키(`model_settings`)
`/process` 요청 시 주로 아래 키를 사용합니다.
- STT: `whisper`
- 교정: `correct`
- 요약: `summarize`
- 공통: `provider`(또는 `llm_provider`), `temperature`, `context_window`, `max_tokens`
- 화자 분리: `diarization_provider`, `num_speakers`, `min_speakers`, `max_speakers`

## 4) 사용 방법

## 4-1. 파일 업로드
- 웹 UI에서 업로드하거나 `/upload` API를 사용합니다.
- 업로드 후 레코드 경로(`DB/uploads/...`) 또는 `record_id`를 기준으로 처리합니다.

## 4-2. 워크플로우 실행(`/process`)
요청 예시:
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

실행 상태 확인:
- `GET /progress/<task_id>`
- WebSocket 실시간 이벤트 (`ws://localhost:8765`)

## 4-3. 결과 조회
- 히스토리: `GET /history`
- 검색: `GET /search`
- 유사 문서: `GET /similar/<uuid_or_path>` 또는 `POST /similar`
- 다운로드: `GET /download/<uuid_or_path>`

## 5) 자주 쓰는 운영 명령

### 테스트
```bash
pytest
```

핵심 회귀 테스트:
```bash
pytest tests/http_api/test_workflow.py tests/http_api/test_search.py tests/server/test_queue.py tests/test_vocab_system.py
```

### 프론트 빌드
```bash
cd frontend
npm install
npm run build
```

## 6) 문제 해결 체크포인트
- 모델 호출 실패 시: provider 실행 상태와 `.env`의 base URL/timeout을 먼저 확인
- Ollama 미사용 구성인데 setup/run에서 Ollama 확인이 필요해 보이면: `.env`의 `LLM_PROVIDER`, `EMBEDDING_PROVIDER` 값 점검
- 파괴적 API 호출 실패 시: 안전 모드 토큰/세션 환경변수 설정 여부 확인
