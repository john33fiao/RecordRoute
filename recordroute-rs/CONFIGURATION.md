# RecordRoute 환경 설정 가이드

RecordRoute를 실행하기 위한 환경 변수 및 설정 파일 가이드입니다.

## 설정 방법

RecordRoute는 다음 순서로 설정을 로드합니다:

1. `.env` 파일 (프로젝트 루트에서 자동 검색)
2. 환경 변수
3. 기본값

**중요**: `.env` 파일은 **프로젝트 최상위 폴더** (`.git` 디렉토리가 있는 곳)에 생성해야 합니다.
RecordRoute는 자동으로 프로젝트 루트를 찾아 `.env` 파일을 로드합니다.

```
RecordRoute/           # 프로젝트 루트
├── .env              # ← 여기에 .env 파일 생성
├── .git/
├── recordroute-rs/
└── ...
```

## 필수 설정

### Whisper 모델

```bash
# Whisper 모델 파일 경로 (ggml 형식)
WHISPER_MODEL=./models/ggml-base.bin
```

**권장 모델**:
- `ggml-tiny.bin` - 가장 빠름, 낮은 정확도 (~75MB)
- `ggml-base.bin` - 균형잡힌 성능 (~142MB)
- `ggml-small.bin` - 높은 정확도 (~466MB)
- `ggml-medium.bin` - 매우 높은 정확도 (~1.5GB)
- `ggml-large-v3.bin` - 최고 정확도 (~2.9GB)

**다운로드**:
```bash
# 프로젝트 루트로 이동
cd RecordRoute

# models 폴더 생성
mkdir -p models
cd models

# Base 모델 (권장)
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin

# 또는 한국어 최적화 모델
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin
```

**중요**: 모델 파일은 **프로젝트 루트의 `models/` 폴더**에 배치해야 합니다 (`RecordRoute/models/`).

### Ollama 설정

```bash
# Ollama API 서버 URL
OLLAMA_BASE_URL=http://localhost:11434

# LLM 모델 (요약용)
LLM_MODEL=llama3.2

# 임베딩 모델 (검색용)
EMBEDDING_MODEL=nomic-embed-text
```

**모델 설치**:
```bash
ollama pull llama3.2
ollama pull nomic-embed-text
```

**권장 LLM 모델**:
- `llama3.2` - 빠른 응답, 한국어 지원
- `gemma2` - Google Gemma 2
- `qwen2.5` - Alibaba Qwen 2.5

**권장 임베딩 모델**:
- `nomic-embed-text` - 최고 성능 (권장)
- `mxbai-embed-large` - 높은 정확도

## 선택적 설정

### 데이터 경로

```bash
# 데이터베이스 기본 경로
DB_BASE_PATH=./data

# 파일 업로드 디렉토리
UPLOAD_DIR=./uploads

# 벡터 인덱스 파일
VECTOR_INDEX_PATH=./data/vector_index.json
```

### 서버 설정

```bash
# 서버 바인드 주소
SERVER_HOST=0.0.0.0

# 서버 포트
SERVER_PORT=8080
```

**프로덕션 환경**:
```bash
# HTTPS 리버스 프록시 뒤에서 실행하는 경우
SERVER_HOST=127.0.0.1
SERVER_PORT=8080
```

### 로그 설정

```bash
# 로그 디렉토리
LOG_DIR=./logs

# 로그 레벨 (trace, debug, info, warn, error)
LOG_LEVEL=info
```

**로그 레벨 설명**:
- `trace` - 모든 로그 (디버깅용)
- `debug` - 디버그 정보
- `info` - 일반 정보 (기본값)
- `warn` - 경고
- `error` - 에러만

## 전체 .env 예시

```bash
# ===========================================
# RecordRoute 환경 설정
# ===========================================

# --- Whisper STT ---
WHISPER_MODEL=./models/ggml-base.bin

# --- Ollama ---
OLLAMA_BASE_URL=http://localhost:11434
LLM_MODEL=llama3.2
EMBEDDING_MODEL=nomic-embed-text

# --- 데이터 경로 ---
DB_BASE_PATH=./data
UPLOAD_DIR=./uploads
VECTOR_INDEX_PATH=./data/vector_index.json

# --- 서버 ---
SERVER_HOST=0.0.0.0
SERVER_PORT=8080

# --- 로그 ---
LOG_DIR=./logs
LOG_LEVEL=info
```

## 환경별 설정

### 개발 환경

```bash
# .env.development
LOG_LEVEL=debug
SERVER_HOST=127.0.0.1
SERVER_PORT=8080
```

실행:
```bash
ln -s .env.development .env
cargo run
```

### 프로덕션 환경

```bash
# .env.production
LOG_LEVEL=warn
SERVER_HOST=127.0.0.1
SERVER_PORT=8080

# 더 큰 모델 사용
WHISPER_MODEL=./models/ggml-large-v3.bin
LLM_MODEL=llama3.2:70b
```

실행:
```bash
ln -s .env.production .env
cargo run --release
```

## 디렉토리 구조

RecordRoute 실행 시 다음 디렉토리가 자동 생성됩니다:

```
RecordRoute/                # 프로젝트 루트
├── .env                    # 환경 설정 파일 (여기에 생성)
└── recordroute-rs/
    ├── data/               # 데이터 저장소
    │   ├── upload_history.json # 히스토리 DB
    │   ├── vector_index.json   # 벡터 인덱스
    │   ├── embeddings/         # 임베딩 파일
    │   └── whisper_output/     # STT 결과
    │       ├── {uuid}.txt      # 전사 텍스트
    │       ├── {uuid}_segments.json
    │       ├── {uuid}_summary.txt
    │       └── {uuid}_oneline.txt
    ├── uploads/            # 업로드된 파일
    │   └── {uuid}.{ext}
    ├── logs/               # 로그 파일
    │   └── recordroute.log
    └── models/             # AI 모델
        └── ggml-base.bin
```

## 성능 튜닝

### Whisper 모델 선택

**빠른 처리 우선**:
```bash
WHISPER_MODEL=./models/ggml-tiny.bin
# 속도: ★★★★★ | 정확도: ★★☆☆☆
```

**균형잡힌 설정** (권장):
```bash
WHISPER_MODEL=./models/ggml-base.bin
# 속도: ★★★★☆ | 정확도: ★★★★☆
```

**최고 정확도**:
```bash
WHISPER_MODEL=./models/ggml-large-v3.bin
# 속도: ★★☆☆☆ | 정확도: ★★★★★
```

### Whisper GPU 가속

RecordRoute는 STT(음성 인식)에 GPU 가속을 지원합니다. 빌드 시 feature flag를 사용하여 활성화할 수 있습니다.

**NVIDIA GPU (CUDA)**:
```bash
cd recordroute-rs
cargo build --release --features cuda
```

**Apple Silicon (Metal)**:
```bash
cd recordroute-rs
cargo build --release --features metal
```

**CPU만 사용** (기본값):
```bash
cd recordroute-rs
cargo build --release
```

**작동 방식**:
- CUDA feature가 활성화되면 NVIDIA GPU 사용 시도
- Metal feature가 활성화되면 Apple GPU 사용 시도
- GPU 초기화 실패 시 자동으로 CPU로 fallback
- GPU 사용 시 전사 속도가 3-10배 향상될 수 있습니다

**요구사항**:
- CUDA: NVIDIA GPU + CUDA 툴킷 설치 필요
- Metal: macOS + Apple Silicon (M1/M2/M3) 또는 AMD GPU

### Ollama 성능

**GPU 사용** (권장):
```bash
# NVIDIA GPU
export OLLAMA_GPU=nvidia

# AMD GPU
export OLLAMA_GPU=rocm

# Apple Silicon
# 자동으로 Metal 사용
```

**메모리 제한**:
```bash
# Ollama 컨텍스트 윈도우 설정
ollama run llama3.2 --ctx-size 4096
```

### 동시 처리

RecordRoute는 Tokio 기반으로 무제한 동시 요청을 처리할 수 있습니다. 하지만 Whisper와 Ollama는 순차적으로 처리됩니다.

**워커 수 증가** (미래 기능):
```bash
# 추후 추가 예정
WORKER_THREADS=4
```

## 트러블슈팅

### Whisper 모델 로딩 실패

```
Error: STT error: Failed to load Whisper model
```

**해결책**:
1. 모델 파일 존재 확인:
   ```bash
   ls -lh ./models/ggml-base.bin
   ```
2. 경로가 올바른지 확인
3. 파일 권한 확인:
   ```bash
   chmod 644 ./models/ggml-base.bin
   ```

### Ollama 연결 실패

```
Error: Failed to connect to Ollama
```

**해결책**:
1. Ollama 실행 확인:
   ```bash
   ollama serve
   ```
2. URL 확인:
   ```bash
   curl http://localhost:11434/api/tags
   ```
3. 방화벽 확인

### 메모리 부족

```
Error: Out of memory
```

**해결책**:
1. 더 작은 Whisper 모델 사용:
   ```bash
   WHISPER_MODEL=./models/ggml-tiny.bin
   ```
2. Ollama 모델 크기 줄이기:
   ```bash
   LLM_MODEL=llama3.2:3b  # 대신 llama3.2:70b
   ```

### 디스크 공간 부족

**정리 스크립트**:
```bash
# 30일 이상된 파일 삭제
find ./uploads -type f -mtime +30 -delete
find ./data/whisper_output -type f -mtime +30 -delete
find ./logs -type f -mtime +7 -delete
```

## 보안 설정

### 프로덕션 체크리스트

- [ ] `SERVER_HOST=127.0.0.1` (리버스 프록시 사용)
- [ ] HTTPS 설정 (Nginx/Caddy)
- [ ] 파일 업로드 크기 제한
- [ ] CORS 설정
- [ ] 로그 레벨 `warn` 이상
- [ ] 정기적인 로그 로테이션
- [ ] 디스크 공간 모니터링

### Nginx 리버스 프록시 예시

```nginx
server {
    listen 80;
    server_name recordroute.example.com;

    client_max_body_size 500M;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

## 관련 문서

- [README.md](./README.md) - 프로젝트 개요
- [API.md](./API.md) - API 문서
- [ARCHITECTURE.md](./ARCHITECTURE.md) - 아키텍처 문서
