# llama.cpp 사용 가이드 (모델 다운로드부터 RecordRoute 적용까지)

이 문서는 **RecordRoute에서 llama.cpp provider를 실제로 사용하는 순서**를 정리합니다.
핵심 흐름은 아래와 같습니다.

1. GGUF 모델 파일 준비(다운로드)
2. llama.cpp 서버(or CLI) 실행 환경 준비
3. `.env`에서 provider를 `llamacpp`로 전환
4. RecordRoute 실행 후 `/models`, `/process`로 동작 확인

---

## 1) 사전 개념 정리

RecordRoute의 llama.cpp 연동은 목적에 따라 2가지 경로를 사용합니다.

- **LLM(교정/요약)**: `llama-cli` 실행 기반 (`LLAMA_CPP_COMMAND`, `LLAMA_CPP_MODEL_PATH`)
- **Embedding(검색 임베딩)**: OpenAI 호환 HTTP API (`EMBEDDING_BASE_URL` → `/v1/embeddings`)

즉, **요약/교정과 임베딩이 같은 방식으로 호출되는 구조가 아닙니다.**
실무에서는 아래 중 하나로 맞추는 것을 권장합니다.

- (권장) llama.cpp server를 띄워 embedding API 제공 + LLM은 `llama-cli` 사용
- (대안) embedding provider는 ollama로 유지하고, LLM만 `llamacpp`로 전환

---

## 2) GGUF 모델 다운로드

`docker-compose.yml` 기준 llama.cpp 컨테이너는 `./models`를 `/models`에 마운트합니다.

```bash
mkdir -p models
```

그 다음 GGUF 모델을 `models/`에 넣습니다.

예시(모델 URL은 본인이 사용하는 저장소/HF 페이지의 실제 링크로 교체):

```bash
curl -L "<GGUF_MODEL_URL>" -o models/model.gguf
```

> 파일명이 `model.gguf`가 아니어도 됩니다. 다만 Compose 기본값은 `/models/model.gguf`를 바라보므로, 파일명을 바꾸면 Compose command 또는 환경변수도 함께 맞춰야 합니다.

---

## 3) 실행 방식 선택

## 3-A) Docker Compose + llama.cpp server profile

`llamacpp` 프로필로 실행하면 llama.cpp server가 `8081` 포트로 올라옵니다.

```bash
docker compose --profile llamacpp up -d --build
```

기본 구성:
- llama.cpp server: `http://localhost:8081`
- backend 환경변수의 기본 `LLM_BASE_URL`, `EMBEDDING_BASE_URL`는 `http://llamacpp:8081`

> 단, provider 기본값은 `ollama`이므로 실제 llama.cpp 사용을 위해서는 `.env`에서 `LLM_PROVIDER=llamacpp`(필요 시 `EMBEDDING_PROVIDER=llamacpp`)를 함께 지정해야 합니다.

`docker-compose.yml`의 llama.cpp 서비스 기본 command는 아래 옵션을 포함합니다.
- `--model /models/model.gguf`
- `--embeddings`

즉, **embedding endpoint 사용 가능하도록 기본 세팅**되어 있습니다.

## 3-B) 로컬(비-Docker) 실행

로컬 실행에서는 llama.cpp 바이너리와 모델 경로를 직접 지정합니다.

```bash
# 예시: llama-cli 경로 확인
command -v llama-cli

# 예시: 환경변수 지정
export LLM_PROVIDER=llamacpp
export EMBEDDING_PROVIDER=llamacpp
export LLAMA_CPP_COMMAND=llama-cli
export LLAMA_CPP_MODEL_PATH=/absolute/path/to/model.gguf
export LLM_BASE_URL=http://localhost:8081
export EMBEDDING_BASE_URL=http://localhost:8081
```

그리고 필요 시 별도로 llama.cpp server를 띄워 embedding API를 제공합니다.

```bash
llama-server --host 0.0.0.0 --port 8081 --model /absolute/path/to/model.gguf --embeddings
```

---

## 4) `.env` 설정 (핵심)

프로젝트 루트에서 `.env.example`을 복사해 `.env`를 만들고 아래 항목을 설정합니다.

```env
LLM_PROVIDER=llamacpp
EMBEDDING_PROVIDER=llamacpp

# llama.cpp CLI 호출용
LLAMA_CPP_COMMAND=llama-cli
LLAMA_CPP_MODEL_PATH=/absolute/path/to/model.gguf
LLAMA_CPP_TIMEOUT=300

# embedding/OpenAI 호환 endpoint
# (LLM_BASE_URL은 EMBEDDING_BASE_URL 미지정 시 fallback으로도 사용됨)
LLM_BASE_URL=http://localhost:8081
EMBEDDING_BASE_URL=http://localhost:8081
EMBEDDING_TIMEOUT=300
```

참고:
- `LLM_PROVIDER`는 `llama_cpp`, `llama.cpp` 별칭도 내부적으로 `llamacpp`로 정규화됩니다.
- 모델명을 일반 문자열로 넘겨도 되지만, llama.cpp LLM 경로는 **`.gguf 경로` 전달 시 검증 우회/허용**이 가능합니다.

---

## 5) RecordRoute에서 실제 적용

## 5-A) 서버 실행

```bash
# 로컬
./venv/bin/python -m sttEngine.server

# 또는 Compose
docker compose --profile llamacpp up -d --build
```

## 5-B) provider/model 상태 확인

```bash
curl -s http://localhost:8080/models | jq
```

확인 포인트:
- `default.provider`가 `llamacpp`인지
- `provider_status.llamacpp`가 정상인지
- 모델 리스트가 비어 있어도 healthcheck 통과면 동작 가능(경로 기반 호출)

## 5-C) `/process`에 llama.cpp 모델 적용

요청 시 `model_settings.provider`를 `llamacpp`로 지정하고,
`correct`, `summarize`에 모델 키를 지정합니다.

```json
{
  "file_path": "DB/uploads/<uuid>/sample.m4a",
  "steps": ["stt", "correct", "summary"],
  "model_settings": {
    "provider": "llamacpp",
    "whisper": "large-v3-turbo",
    "correct": "/absolute/path/to/model.gguf",
    "summarize": "/absolute/path/to/model.gguf",
    "temperature": 0.2,
    "context_window": 8192,
    "max_tokens": 1024
  }
}
```

옵션 키 매핑:
- `context_window` → `num_ctx`
- `max_tokens` → `num_predict`

---

## 6) 자주 발생하는 문제

- `llama.cpp 실행 파일을 찾을 수 없습니다`
  - `LLAMA_CPP_COMMAND` 경로/명령어 확인 (`command -v llama-cli`)
- `모델 경로가 필요합니다`
  - `LLAMA_CPP_MODEL_PATH` 또는 `/process`의 `correct`/`summarize`에 `.gguf` 경로 지정
- `embedding 요청 실패`
  - `EMBEDDING_BASE_URL`의 서버가 `/v1/embeddings`를 제공하는지 확인
  - llama.cpp server 실행 시 `--embeddings` 옵션 누락 여부 확인
- `/search`에서 일부 문서가 빠짐
  - 임베딩 차원 불일치 시 해당 문서는 자동 제외됩니다(로그에 기록)

---

## 7) 운영 권장 시나리오 (간단 정리)

1. `models/`에 GGUF 준비
2. llama.cpp server를 `8081`로 실행(`--embeddings` 포함)
3. `.env`를 `LLM_PROVIDER=llamacpp`, `EMBEDDING_PROVIDER=llamacpp`로 설정
4. RecordRoute 재시작
5. `/models` → `/process` → `/search` 순서로 검증

이 순서대로 맞추면 "llama.cpp 전환은 했는데 실제 사용법을 모르겠다"는 상태를 가장 빠르게 해소할 수 있습니다.


---

## 8) RTD 검증 체크리스트 (문서 기준 자체 점검)

아래 순서대로 확인하면 문서대로 설정이 되었는지 빠르게 검증할 수 있습니다.

```bash
# 1) provider 값 확인
python - <<'PY'
import os
print("LLM_PROVIDER=", os.getenv("LLM_PROVIDER"))
print("EMBEDDING_PROVIDER=", os.getenv("EMBEDDING_PROVIDER"))
PY

# 2) llama.cpp server 헬스/임베딩 endpoint 확인
curl -s http://localhost:8081/health
curl -s http://localhost:8081/v1/embeddings \
  -H 'Content-Type: application/json' \
  -d '{"model":"/absolute/path/to/model.gguf","input":"ping"}'

# 3) RecordRoute 모델 상태 확인
curl -s http://localhost:8080/models | jq

# 4) 워크플로우 실행 후 진행률/결과 확인
curl -s http://localhost:8080/progress/<task_id> | jq
```

판정 기준:
- `/models`의 `default.provider`가 `llamacpp`
- `provider_status.llamacpp`가 실패가 아님
- `/process` 실행 시 `correct`/`summary` 단계에서 provider 오류가 발생하지 않음
- `/search`에서 임베딩 차원 불일치 로그가 과도하게 발생하지 않음
