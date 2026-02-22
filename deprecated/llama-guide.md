# RecordRoute + llama.cpp 모델 세팅 가이드 (In-process)

> 목표: RecordRoute에서 `llama-cpp-python` 기반 **in-process llama.cpp**를 안정적으로 설정하고 검증합니다.

이 가이드는 **로컬 실행(비 Docker)** 기준입니다.

---

## 0) 핵심 요약

RecordRoute의 `llamacpp` provider는 외부 `llama-cli`/`llama-server` 호출이 아니라,
백엔드 프로세스 내부에서 `llama_cpp.Llama`를 직접 호출합니다.

필수 포인트:

- `LLM_PROVIDER=llamacpp`
- `EMBEDDING_PROVIDER=llamacpp`
- `LLAMA_CPP_MODEL_PATH` 설정 (미설정 시 기본값 `./models/default_model.gguf`)
- `pip install -r requirements.txt`로 `llama-cpp-python` 설치

하위 호환용 환경 변수(`LLM_BASE_URL`, `EMBEDDING_BASE_URL`, `LLAMA_CPP_COMMAND`)는
남겨둘 수 있지만 in-process 모드에서 실제 호출에는 사용되지 않으며 warning 로그만 출력됩니다.

---

## 1) 준비물

- RecordRoute 프로젝트
- `.gguf` 모델 파일
- Python 가상환경 (`venv`) + `requirements.txt` 설치

### 1-1. 모델 파일 준비

프로젝트 루트에서:

```bash
mkdir -p models
```

`models/` 안에 `.gguf` 파일을 넣으세요. 예:

- `models/default_model.gguf`

---

## 2) 의존성 설치

```bash
./venv/bin/python -m pip install -r requirements.txt
```

> `llama-cpp-python` 빌드에 시간이 걸릴 수 있습니다.

---

## 3) `.env` 설정

프로젝트 루트에서 `.env`를 열고(없으면 `.env.example` 복사) 아래를 설정하세요.

```env
LLM_PROVIDER=llamacpp
EMBEDDING_PROVIDER=llamacpp

# 미설정 시 기본값: ./models/default_model.gguf
LLAMA_CPP_MODEL_PATH=./models/default_model.gguf

# 선택 튜닝
# LLAMA_CPP_N_CTX=8192
# LLAMA_CPP_N_THREADS=8
# LLAMA_CPP_N_BATCH=512
# LLAMA_CPP_N_GPU_LAYERS=35
# LLAMA_CPP_CHAT_FORMAT=chatml
```

legacy 변수는 선택적으로 남겨둘 수 있으나 in-process 모드에서는 무시됩니다.

```env
# 아래 값들은 하위 호환용(무시됨)
# LLM_BASE_URL=http://localhost:8081
# EMBEDDING_BASE_URL=http://localhost:8081
# LLAMA_CPP_COMMAND=llama-cli
```

---

## 4) RecordRoute 실행

```bash
./venv/bin/python -m sttEngine.server
```

서버가 올라오면 모델 상태 확인:

```bash
curl -s http://localhost:8080/models | jq
```

`default.provider`가 `llamacpp`면 1차 설정 성공입니다.

---

## 5) 실제 처리 요청 (`/process`)

```json
{
  "file_path": "DB/uploads/<uuid>/sample.m4a",
  "steps": ["stt", "correct", "summary"],
  "model_settings": {
    "provider": "llamacpp",
    "whisper": "large-v3-turbo",
    "correct": "./models/default_model.gguf",
    "summarize": "./models/default_model.gguf",
    "temperature": 0.2,
    "context_window": 8192,
    "max_tokens": 1024
  }
}
```

진행률 확인:

```bash
curl -s http://localhost:8080/progress/<task_id> | jq
```

---

## 6) 트러블슈팅

### A. `llama-cpp-python` import 실패

- `./venv/bin/python -m pip install -r requirements.txt` 재실행
- Python/컴파일러 환경 확인

### B. `llama.cpp 모델 파일을 찾을 수 없습니다`

- `LLAMA_CPP_MODEL_PATH` 값 확인
- 상대경로 기준은 **프로젝트 루트**

### C. 응답이 느림 / 메모리 사용량 큼

- 큰 모델일수록 첫 로딩이 오래 걸립니다(이후는 캐시 재사용)
- `LLAMA_CPP_N_THREADS`, `LLAMA_CPP_N_GPU_LAYERS`, `LLAMA_CPP_N_BATCH` 튜닝

### D. 임베딩/검색 결과가 비정상

- 임베딩 차원 불일치 문서는 검색에서 제외될 수 있음(서버 로그 확인)

---

## 7) 최소 체크리스트

1. `.gguf` 파일 준비 (`models/default_model.gguf`)
2. `.env`에서 provider 2개를 `llamacpp`로 설정
3. `LLAMA_CPP_MODEL_PATH` 확인
4. 서버 실행 후 `GET /models` 확인
5. `/process` 1건 실행 + `/progress/<task_id>` 확인

---

## 부록) Docker

```bash
docker compose --profile llamacpp up -d --build
```

- 프론트: `http://localhost:3000`
- 백엔드 API: `http://localhost:8080`

주의: Docker 환경에서도 `.env`에서 provider를 `llamacpp`로 설정해야 llama.cpp 경로가 사용됩니다.
