# RecordRoute + llama.cpp 초간단 따라하기

> 목표: **"처음 하는 사람도 그대로 복붙해서"** llama.cpp로 RecordRoute를 실행하도록 안내합니다.

이 가이드는 **로컬 실행(비 Docker)** 기준입니다.
Docker로 하고 싶으면 맨 아래 "부록"만 보면 됩니다.

---

## 0) 먼저 알아둘 것 (30초)

RecordRoute에서 llama.cpp는 보통 2가지를 씁니다.

- **교정/요약 LLM**
- **임베딩(검색용)**

둘 다 안정적으로 쓰려면, 가장 단순하게 아래처럼 맞추면 됩니다.

- `LLM_PROVIDER=llamacpp`
- `EMBEDDING_PROVIDER=llamacpp`
- llama.cpp 서버(`llama-server`)를 `8081`로 실행하고 `--embeddings` 옵션 켜기

---

## 1) 준비물

- RecordRoute 프로젝트
- `.gguf` 모델 파일 1개
- `llama-cli`, `llama-server` 실행 가능 환경

### 1-1. 모델 파일 준비

프로젝트 루트에서:

```bash
mkdir -p models
```

`models/` 안에 `.gguf` 파일을 넣으세요. 예:

- `models/model.gguf`

---

## 2) llama.cpp 서버 먼저 실행

아래 명령에서 경로만 본인 환경으로 바꿔서 실행하세요.

```bash
llama-server --host 0.0.0.0 --port 8081 --model /절대경로/models/model.gguf --embeddings
```

정상 실행되면 다음 체크:

```bash
curl -s http://localhost:8081/health
```

응답이 나오면 OK입니다.

---

## 3) `.env` 설정

프로젝트 루트에서 `.env`를 열고(없으면 `.env.example` 복사) 아래를 설정하세요.

```env
LLM_PROVIDER=llamacpp
EMBEDDING_PROVIDER=llamacpp

LLAMA_CPP_COMMAND=llama-cli
LLAMA_CPP_MODEL_PATH=/절대경로/models/model.gguf
LLAMA_CPP_TIMEOUT=300

LLM_BASE_URL=http://localhost:8081
EMBEDDING_BASE_URL=http://localhost:8081
EMBEDDING_TIMEOUT=300
```

핵심은 3개입니다.

1. provider 둘 다 `llamacpp`
2. 모델 경로는 **절대경로**
3. base url 둘 다 `http://localhost:8081`

---

## 4) RecordRoute 실행

```bash
./venv/bin/python -m sttEngine.server
```

서버가 떴다면 모델 상태 확인:

```bash
curl -s http://localhost:8080/models | jq
```

여기서 `default.provider`가 `llamacpp`면 1차 성공입니다.

---

## 5) 실제 처리 요청 (`/process`)

아래 JSON을 기준으로 요청하세요.

```json
{
  "file_path": "DB/uploads/<uuid>/sample.m4a",
  "steps": ["stt", "correct", "summary"],
  "model_settings": {
    "provider": "llamacpp",
    "whisper": "large-v3-turbo",
    "correct": "/절대경로/models/model.gguf",
    "summarize": "/절대경로/models/model.gguf",
    "temperature": 0.2,
    "context_window": 8192,
    "max_tokens": 1024
  }
}
```

진행률은:

```bash
curl -s http://localhost:8080/progress/<task_id> | jq
```

---

## 6) 막히는 지점 빠른 해결

### A. "llama.cpp 실행 파일을 찾을 수 없습니다"

```bash
command -v llama-cli
command -v llama-server
```

둘 중 하나라도 비어 있으면 설치/경로 문제입니다.

### B. "모델 경로가 필요합니다"

- `LLAMA_CPP_MODEL_PATH` 확인
- `/process`의 `correct`, `summarize`에 `.gguf` 절대경로 넣었는지 확인

### C. "임베딩 실패"

- `EMBEDDING_BASE_URL=http://localhost:8081` 확인
- `llama-server` 실행 시 `--embeddings` 넣었는지 확인

### D. `/search` 결과가 이상함

- 임베딩 차원 불일치가 있으면 문서가 제외될 수 있습니다(로그 확인)

---

## 7) 진짜 최소 체크리스트 (이것만 하면 됨)

1. `.gguf` 파일 준비
2. `llama-server ... --embeddings` 실행
3. `.env`에서 provider 2개를 `llamacpp`로 설정
4. RecordRoute 실행
5. `GET /models`에서 `default.provider=llamacpp` 확인
6. `/process` 1건 실행 후 `/progress/<task_id>` 확인

여기까지 되면 **사용 가능한 상태**입니다.

---

## 부록) Docker로 하고 싶다면

```bash
docker compose --profile llamacpp up -d --build
```

- llama.cpp 서버: `http://localhost:8081`
- 백엔드 API: `http://localhost:8080`

주의: Docker로 띄워도 `.env`에서 provider를 `llamacpp`로 바꿔야 실제로 llama.cpp를 사용합니다.
