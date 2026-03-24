# RecordRoute

RecordRoute는 음성 파일을 업로드하면 자동으로 다음 작업을 처리하는 로컬 중심 API 서버입니다.

- 오디오 업로드 저장
- `ffmpeg`로 표준 WAV 변환
- `whisper.cpp` 기반 전사
- `llama.cpp` 기반 요약 생성
- `llama.cpp` 기반 임베딩 생성
- 키워드 + 벡터 검색

이 문서는 "처음 클론한 뒤 바로 띄워보는 방법"에 집중합니다. 내부 아키텍처 설명은 별도 문서로 분리하는 것을 전제로 작성했습니다.

## 빠른 시작

가장 먼저 필요한 것은 4가지입니다.

- Rust toolchain
- PostgreSQL + `pgvector`
- `ffmpeg`
- `whisper.cpp`, `llama.cpp` 빌드용 CMake + C/C++ 빌드 도구

권장 순서는 아래와 같습니다.

1. 저장소를 서브모듈까지 함께 클론합니다.
2. PostgreSQL 데이터베이스를 준비합니다.
3. `whisper.cpp`, `llama.cpp` 서버 바이너리를 빌드합니다.
4. Whisper 모델 1개, 요약용 GGUF 모델 1개, 임베딩용 GGUF 모델 1개를 준비합니다.
5. 환경 변수를 설정합니다.
6. `rust/`에서 `cargo run`으로 API를 실행합니다.

## 1. 저장소 클론

처음부터 클론하는 경우:

```powershell
git clone --recurse-submodules <repository-url>
cd RecordRoute
```

이미 클론한 저장소라면:

```powershell
git submodule update --init --recursive
```

루트에는 외부 서브모듈이 포함되어 있습니다.

- `whisper.cpp`
- `llama.cpp`
- `ffmpeg`

실제 애플리케이션 코드는 `rust/` 아래에 있습니다.

## 2. 필수 준비물

### Rust

`cargo`가 동작해야 합니다.

```powershell
cargo --version
```

### PostgreSQL + pgvector

PostgreSQL 서버가 먼저 떠 있어야 하고, `vector` extension을 사용할 수 있어야 합니다.

예시:

```sql
CREATE DATABASE recordroute;
```

`RecordRoute`는 시작 시 migration을 자동 실행하며, migration 안에 아래 SQL이 포함되어 있습니다.

```sql
CREATE EXTENSION IF NOT EXISTS vector;
```

즉, 애플리케이션이 extension 생성 권한을 가질 수 있어야 합니다. 만약 여기서 실패하면 PostgreSQL 서버에 `pgvector`가 설치되지 않은 상태입니다.

### ffmpeg

가장 쉬운 방법은 시스템에 `ffmpeg`를 설치하고 `PATH`에 잡히게 하는 것입니다.

```powershell
ffmpeg -version
```

루트에 `ffmpeg/` 서브모듈이 있지만, 일반 사용자 입장에서는 이 서브모듈을 직접 빌드하기보다 시스템 `ffmpeg`를 설치해서 쓰는 편이 훨씬 간단합니다.

### CMake + C/C++ 빌드 도구

`whisper.cpp`, `llama.cpp`를 빌드할 수 있어야 합니다.

```powershell
cmake --version
```

Windows에서는 보통 아래 조합이면 충분합니다.

- Visual Studio Build Tools 또는 Visual Studio C++ workload
- CMake
- Git Bash 또는 WSL

Git Bash/WSL은 Whisper 모델 다운로드 스크립트(`.sh`)를 그대로 쓸 때 특히 편합니다.

## 3. PostgreSQL 준비

예시 접속 정보는 아래와 같이 맞추면 바로 사용할 수 있습니다.

- DB 이름: `recordroute`
- 사용자: `postgres`
- 비밀번호: `postgres`
- 주소: `localhost:5432`

이 경우 `DATABASE_URL`은 아래처럼 됩니다.

```text
postgres://postgres:postgres@localhost:5432/recordroute
```

## 4. 사이드카 빌드

### 4-1. whisper.cpp 빌드

PowerShell에서:

```powershell
cd .\whisper.cpp
cmake -B build
cmake --build build --config Release
cd ..
```

실행 파일 위치는 빌드 환경에 따라 둘 중 하나입니다.

- `whisper.cpp\build\bin\Release\whisper-server.exe`
- `whisper.cpp\build\bin\whisper-server.exe`

### 4-2. llama.cpp 빌드

PowerShell에서:

```powershell
cd .\llama.cpp
cmake -B build
cmake --build build --config Release -t llama-server
cd ..
```

실행 파일 위치는 보통 둘 중 하나입니다.

- `llama.cpp\build\bin\Release\llama-server.exe`
- `llama.cpp\build\bin\llama-server.exe`

### 4-3. PowerShell에서 실행 파일 경로 변수 잡기

이후 명령을 편하게 실행하려면 아래처럼 변수로 잡아 두는 것이 편합니다.

```powershell
$WhisperServer = if (Test-Path ".\whisper.cpp\build\bin\Release\whisper-server.exe") {
  ".\whisper.cpp\build\bin\Release\whisper-server.exe"
} else {
  ".\whisper.cpp\build\bin\whisper-server.exe"
}

$LlamaServer = if (Test-Path ".\llama.cpp\build\bin\Release\llama-server.exe") {
  ".\llama.cpp\build\bin\Release\llama-server.exe"
} else {
  ".\llama.cpp\build\bin\llama-server.exe"
}
```

둘 다 없다면 빌드가 아직 끝나지 않은 상태입니다.

## 5. 모델 준비

RecordRoute가 바로 동작하려면 모델이 3개 필요합니다.

- Whisper 전사용 모델 1개
- 요약용 GGUF 모델 1개
- 임베딩용 GGUF 모델 1개

### 5-1. Whisper 모델 다운로드

가장 쉬운 방법은 `whisper.cpp`의 공식 다운로드 스크립트를 쓰는 것입니다.

Git Bash 또는 WSL에서:

```bash
cd whisper.cpp/models
./download-ggml-model.sh base
```

그러면 보통 아래 파일이 생깁니다.

```text
whisper.cpp/models/ggml-base.bin
```

처음에는 `base` 또는 `small` 정도로 시작하는 것이 무난합니다.

- `base`: 가볍고 시작하기 쉬움
- `small`: 조금 더 무겁지만 품질 개선 여지 있음

Windows에서 `.sh` 스크립트를 쓰기 어렵다면, `whisper.cpp/models/README.md`에 안내된 경로에서 GGML 모델을 직접 내려받아도 됩니다.

중요한 점은 최종적으로 `WHISPER_MODEL` 환경 변수에 실제 파일 경로를 넣어야 한다는 것입니다.

예시:

```text
C:\workspace\RecordRoute\whisper.cpp\models\ggml-base.bin
```

### 5-2. 요약용 GGUF 모델 준비

요약 서버는 `llama.cpp`의 OpenAI 호환 `chat completions` API를 사용합니다. 즉, "대화/지시 수행이 가능한 GGUF 모델" 1개가 필요합니다.

준비 방법은 2가지입니다.

#### 방법 A. `llama-server`가 첫 실행 시 Hugging Face에서 자동 다운로드

```powershell
& $LlamaServer -hf <huggingface-user>/<summary-model-repo>:Q4_K_M --port 8081
```

장점:

- 별도 수동 다운로드가 없어 가장 간단합니다.
- `llama.cpp` 캐시에 자동 저장됩니다.

#### 방법 B. GGUF 파일을 직접 받아서 로컬 경로로 실행

```powershell
& $LlamaServer -m C:\models\summary-model.gguf --port 8081
```

요약용 모델은 아래 조건을 만족하면 됩니다.

- GGUF 형식
- `llama-server`에서 로드 가능
- 채팅/지시 응답이 가능한 모델

### 5-3. 임베딩용 GGUF 모델 준비

임베딩 서버는 `llama.cpp`의 `/v1/embeddings` API를 사용합니다. 즉, "임베딩용 GGUF 모델"을 별도 포트에서 띄워야 합니다.

자동 다운로드 예시:

```powershell
& $LlamaServer -hf <huggingface-user>/<embedding-model-repo>:Q8_0 --embeddings --pooling cls --port 8082
```

로컬 파일 예시:

```powershell
& $LlamaServer -m C:\models\embedding-model.gguf --embeddings --pooling cls --port 8082
```

주의:

- 임베딩 모델은 요약 모델과 별도로 준비하는 것이 안전합니다.
- 모델을 바꿔 임베딩 차원이 달라지면 기존 `recording_embeddings` 테이블과 충돌할 수 있습니다.
- 한 번 저장한 뒤에는 같은 차원의 임베딩 모델을 계속 쓰는 것이 좋습니다.

### 5-4. `SUMMARY_MODEL`, `EMBED_MODEL` 값은 무엇을 넣어야 하나

기본 예시는 아래처럼 단순 식별자를 써도 됩니다.

```text
SUMMARY_MODEL=summary-model
EMBED_MODEL=embedding-model
```

이 저장소의 기본 실행 방식은 "포트별로 모델 서버를 하나씩 띄우는 방식"이라서, 보통 이 값은 단순 라벨로 두어도 충분합니다.

다만 `llama-server`를 router mode로 운영하면서 여러 모델을 한 서버에서 라우팅할 계획이라면, 실제 요청에 사용되는 model id와 맞춰 주는 편이 좋습니다.

## 6. 환경 변수 설정

기본 템플릿은 `rust/.env.example`에 있습니다.

중요:

- 이 프로젝트는 `.env`를 자동 로드하지 않습니다.
- 파일만 만들어 두고 `cargo run`을 실행하면 적용되지 않습니다.
- 반드시 셸 환경 변수로 주입해야 합니다.

PowerShell 예시:

```powershell
$env:APP_BIND_ADDR="127.0.0.1:3000"
$env:APP_STORAGE_ROOT="C:\workspace\RecordRoute\local-storage"
$env:DATABASE_URL="postgres://postgres:postgres@localhost:5432/recordroute"
$env:FFMPEG_BIN="ffmpeg"

$env:WHISPER_BASE_URL="http://127.0.0.1:8080"
$env:LLAMA_SUMMARY_BASE_URL="http://127.0.0.1:8081"
$env:LLAMA_EMBED_BASE_URL="http://127.0.0.1:8082"

$env:WHISPER_MODEL="C:\workspace\RecordRoute\whisper.cpp\models\ggml-base.bin"
$env:SUMMARY_MODEL="summary-model"
$env:EMBED_MODEL="embedding-model"

$env:SIDECAR_TIMEOUT_SECS="300"
$env:WORKER_POLL_INTERVAL_MS="1000"
$env:MAX_UPLOAD_SIZE_BYTES="104857600"
$env:MAX_JOB_ATTEMPTS="3"
$env:JOB_RETRY_BACKOFF_SECS="15"
$env:MAX_SEARCH_LIMIT="50"
```

기본값/필수값은 `rust/.env.example`를 기준으로 확인하면 됩니다.

## 7. 서버 실행 순서

먼저 `whisper-server`, `llama-server` 2개를 띄우고, 마지막으로 Rust API를 실행하면 됩니다.

### 7-1. Whisper 서버 실행

```powershell
& $WhisperServer --host 127.0.0.1 --port 8080
```

이 애플리케이션은 Whisper 서버에 먼저 `/load`를 호출하면서 `WHISPER_MODEL` 경로를 전달합니다. 즉, `whisper-server`를 실행할 때 꼭 모델 경로를 커맨드라인에 넣을 필요는 없습니다.

### 7-2. 요약용 llama 서버 실행

로컬 GGUF 파일을 쓰는 경우:

```powershell
& $LlamaServer -m C:\models\summary-model.gguf --host 127.0.0.1 --port 8081
```

자동 다운로드를 쓰는 경우:

```powershell
& $LlamaServer -hf <huggingface-user>/<summary-model-repo>:Q4_K_M --host 127.0.0.1 --port 8081
```

### 7-3. 임베딩용 llama 서버 실행

```powershell
& $LlamaServer -m C:\models\embedding-model.gguf --embeddings --pooling cls --host 127.0.0.1 --port 8082
```

또는:

```powershell
& $LlamaServer -hf <huggingface-user>/<embedding-model-repo>:Q8_0 --embeddings --pooling cls --host 127.0.0.1 --port 8082
```

### 7-4. Rust API 실행

작업 디렉터리를 `rust/`로 이동해서 실행합니다.

```powershell
cd .\rust
cargo run
```

정상 실행되면 `http://127.0.0.1:3000`에서 API가 열립니다.

애플리케이션 시작 시 자동으로 수행되는 작업:

- PostgreSQL 연결
- migration 실행
- storage root 디렉터리 생성
- 백그라운드 job worker 시작
- HTTP API 서버 시작

즉, 별도의 워커 프로세스를 따로 띄울 필요는 없습니다.

## 8. 사용 방법

지원 업로드 형식:

- `mp3`
- `m4a`
- `wav`

### 8-1. 녹음 업로드

```powershell
curl.exe -X POST http://127.0.0.1:3000/v1/recordings `
  -F "file=@C:\path\to\sample.wav"
```

응답 예시:

```json
{
  "recording_id": "8cf4d53c-8f3e-4ea0-a4cc-2b4e7c0d2c35",
  "job_id": "8ff6ef59-b8a2-451d-a7c7-c9d74c3dd8b8",
  "status": "queued",
  "step": "upload_saved"
}
```

### 8-2. 작업 상태 확인

```powershell
curl.exe http://127.0.0.1:3000/v1/jobs/<job_id>
```

상태 값:

- `queued`
- `processing`
- `completed`
- `failed`

단계 값:

- `upload_saved`
- `wav_ready`
- `transcribed`
- `summarized`
- `embedded`

### 8-3. 녹음 상세 조회

```powershell
curl.exe http://127.0.0.1:3000/v1/recordings/<recording_id>
```

여기서 transcript, summary, language, last_error 등을 확인할 수 있습니다.

### 8-4. 녹음 목록 조회

전체 목록:

```powershell
curl.exe http://127.0.0.1:3000/v1/recordings
```

상태 필터:

```powershell
curl.exe "http://127.0.0.1:3000/v1/recordings?status=completed"
```

### 8-5. 검색

```powershell
curl.exe "http://127.0.0.1:3000/v1/search?query=회의&limit=10"
```

검색 동작은 아래처럼 이해하면 됩니다.

- 임베딩 생성이 가능하면 hybrid 검색
- 임베딩 생성이 실패하면 keyword-only 검색으로 자동 fallback

## 9. 저장 파일 위치

기본적으로 `APP_STORAGE_ROOT` 아래에 녹음별 디렉터리가 생성됩니다.

구조는 아래와 같습니다.

```text
APP_STORAGE_ROOT/
  <recording_id>/
    original/
      <sanitized filename>
    wav/
      standard.wav
    artifacts/
      whisper-vjson.json
      summary.json
      embedding.json
```

파일명은 영숫자, `.`, `_`, `-`만 유지되고 나머지는 `_`로 치환됩니다.

## 10. 테스트

빠른 테스트:

```powershell
cd .\rust
cargo test
```

실사이드카 smoke placeholder:

```powershell
cargo test -- --ignored
```

현재 ignored smoke는 실제 환경 검사용 placeholder 수준입니다. 빠른 회귀 확인은 `mock_integration.rs` 기반 테스트가 더 믿을 만합니다.

## 11. 자주 막히는 문제

### `.env.example`를 복사했는데 값이 안 먹는 경우

이 프로젝트는 `.env` 자동 로딩이 없습니다. PowerShell에서 `$env:KEY="value"` 형태로 넣거나, 실행 전에 별도 스크립트로 환경 변수를 주입해야 합니다.

### `failed to run database migrations` 또는 `vector` 관련 오류

대부분 PostgreSQL에 `pgvector`가 설치되지 않았거나, `CREATE EXTENSION vector` 권한이 없는 경우입니다.

### 업로드가 바로 400으로 떨어지는 경우

다음 중 하나일 가능성이 큽니다.

- 확장자가 `mp3`, `m4a`, `wav`가 아님
- content-type이 허용 목록 밖임
- 업로드 크기가 `MAX_UPLOAD_SIZE_BYTES`를 초과함

### 검색이 느리거나 이상한 경우

임베딩 서버가 실패하면 검색은 자동으로 keyword-only 모드로 떨어집니다. 먼저 `LLAMA_EMBED_BASE_URL` 서버가 정상인지 확인하세요.

### 임베딩 모델을 바꾼 뒤 에러가 나는 경우

이미 저장된 `recording_embeddings` 테이블은 첫 저장 시점의 벡터 차원으로 고정됩니다. 다른 차원의 모델로 바꾸면 런타임 에러가 날 수 있습니다.

## 12. 권장 시작 조합

처음에는 아래 정도로 시작하는 것이 가장 단순합니다.

- Whisper: `base`
- Summary: 작은 instruction/chat GGUF 모델 1개
- Embedding: 전용 embedding GGUF 모델 1개
- `ffmpeg`: 시스템 설치 버전 사용

즉, 처음 목표는 "최고 품질"보다 "전체 파이프라인이 끝까지 한 번 성공하는지"를 먼저 확인하는 것입니다.

## 13. 체크리스트

실행 전에 아래 6개만 확인하면 됩니다.

- 저장소를 `--recurse-submodules`로 받았는가
- PostgreSQL이 떠 있는가
- `pgvector`를 사용할 수 있는가
- `ffmpeg`가 `PATH`에 잡히는가
- `whisper-server`, `llama-server`가 각각 8080, 8081, 8082에서 뜨는가
- 환경 변수를 셸에 실제로 주입했는가
