# RecordRoute

RecordRoute는 음성 파일을 업로드하면 로컬에서 다음 파이프라인을 자동으로 처리하는 API 서버입니다.

- 오디오 업로드 저장
- `ffmpeg`로 표준 WAV 변환
- `whisper.cpp` 기반 전사
- `llama.cpp` 기반 요약 생성
- `llama.cpp` 기반 임베딩 생성
- SQLite FTS + embedding artifact 기반 검색

이 문서는 빠르게 실행하는 방법에 집중합니다. 내부 구조는 [docs/architecture.md](./docs/architecture.md)를 참고하세요.

## 빠른 시작

가장 먼저 필요한 것은 4가지입니다.

- Rust toolchain
- SQLite를 포함한 로컬 파일 시스템 접근 권한
- `ffmpeg`
- `whisper.cpp`, `llama.cpp` 빌드용 CMake + C/C++ 빌드 도구

권장 순서는 아래와 같습니다.

1. 저장소를 서브모듈까지 함께 클론합니다.
2. `whisper.cpp`, `llama.cpp` 서버 바이너리를 빌드합니다.
3. Whisper 모델 1개, 요약용 GGUF 모델 1개, 임베딩용 GGUF 모델 1개를 준비합니다.
4. 환경 변수를 설정합니다.
5. `rust/`에서 `cargo run`으로 API를 실행합니다.

## 1. 저장소 클론

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

```powershell
cargo --version
```

### SQLite

외부 DB 서버는 필요하지 않습니다. 앱은 `APP_DB_PATH` 위치의 SQLite 파일을 자동 생성합니다.

기본값:

```text
APP_DB_PATH=APP_STORAGE_ROOT/record-route.db
```

즉, 처음 실행 시 필요한 것은 DB 서버가 아니라 쓰기 가능한 `APP_STORAGE_ROOT` 디렉터리입니다.

### ffmpeg

```powershell
ffmpeg -version
```

루트에 `ffmpeg/` 서브모듈이 있지만, 일반적으로는 시스템 `ffmpeg`를 설치해서 쓰는 편이 가장 간단합니다.

### CMake + C/C++ 빌드 도구

```powershell
cmake --version
```

Windows에서는 보통 아래 조합이면 충분합니다.

- Visual Studio Build Tools 또는 Visual Studio C++ workload
- CMake
- Git Bash 또는 WSL

## 3. 사이드카 빌드

### whisper.cpp 빌드

```powershell
cd .\whisper.cpp
cmake -B build
cmake --build build --config Release
cd ..
```

### llama.cpp 빌드

```powershell
cd .\llama.cpp
cmake -B build
cmake --build build --config Release -t llama-server
cd ..
```

실행 파일 경로는 환경에 따라 보통 아래 둘 중 하나입니다.

- `whisper.cpp\build\bin\Release\whisper-server.exe`
- `whisper.cpp\build\bin\whisper-server.exe`
- `llama.cpp\build\bin\Release\llama-server.exe`
- `llama.cpp\build\bin\llama-server.exe`

## 4. 모델 준비

필요한 모델은 3개입니다.

- Whisper 전사용 모델 1개
- 요약용 GGUF 모델 1개
- 임베딩용 GGUF 모델 1개

### Whisper 모델

Git Bash 또는 WSL에서 예시:

```bash
cd whisper.cpp/models
./download-ggml-model.sh base
```

생성 예시:

```text
whisper.cpp/models/ggml-base.bin
```

### 요약용 GGUF 모델

예시:

```powershell
& $LlamaServer -m C:\models\summary-model.gguf --host 127.0.0.1 --port 8081
```

### 임베딩용 GGUF 모델

예시:

```powershell
& $LlamaServer -m C:\models\embedding-model.gguf --embeddings --pooling cls --host 127.0.0.1 --port 8082
```

중요한 점:

- 임베딩 벡터는 DB가 아니라 각 recording의 `artifacts/embedding.json`에 저장됩니다.
- 모델 차원이 바뀌어도 전체 DB 스키마가 깨지지는 않습니다.
- 검색 시 query embedding 차원과 artifact 차원이 다르면 해당 recording만 similarity 계산에서 제외됩니다.

## 5. 환경 변수 설정

기본 템플릿은 `rust/.env.example`에 있습니다.

중요:

- 이 프로젝트는 `.env`를 자동 로드하지 않습니다.
- 반드시 셸 환경 변수로 주입해야 합니다.

PowerShell 예시:

```powershell
$env:APP_BIND_ADDR="127.0.0.1:3000"
$env:APP_STORAGE_ROOT="C:\workspace\RecordRoute\local-storage"
$env:APP_DB_PATH="C:\workspace\RecordRoute\local-storage\record-route.db"
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

## 6. 서버 실행

먼저 `whisper-server`, 요약용 `llama-server`, 임베딩용 `llama-server`를 띄우고 마지막으로 Rust API를 실행합니다.

```powershell
cd .\rust
cargo run
```

정상 실행되면 `http://127.0.0.1:3000`에서 API가 열립니다.

앱 시작 시 자동으로 수행되는 작업:

- SQLite DB 파일 초기화
- storage root 디렉터리 생성
- 백그라운드 job worker 시작
- HTTP API 서버 시작

## 7. 테스트

작업 디렉터리는 `rust/`를 사용합니다.

```powershell
cargo test
```

실사이드카 smoke placeholder:

```powershell
cargo test -- --ignored
```

현재 ignored smoke는 placeholder 수준이며, 빠른 회귀 확인은 `mock_integration.rs`와 `sqlite_repository.rs`가 더 믿을 만합니다.

## 8. 자주 막히는 문제

### `.env.example`를 복사했는데 값이 안 먹는 경우

이 프로젝트는 `.env` 자동 로딩이 없습니다. PowerShell에서 `$env:KEY="value"` 형태로 넣거나 실행 전에 별도 스크립트로 주입해야 합니다.

### 앱이 시작되지 않는 경우

대부분 아래 중 하나입니다.

- `APP_STORAGE_ROOT` 또는 `APP_DB_PATH` 경로에 쓰기 권한이 없음
- `whisper-server`, `llama-server`가 아직 떠 있지 않음
- `ffmpeg`가 `PATH`에 없음

### 업로드가 바로 400으로 떨어지는 경우

다음 중 하나일 가능성이 큽니다.

- 확장자가 `mp3`, `m4a`, `wav`가 아님
- content-type이 허용 목록 밖임
- 업로드 크기가 `MAX_UPLOAD_SIZE_BYTES`를 초과함

### 검색이 느리거나 이상한 경우

- embedding 서버가 실패하면 검색은 자동으로 keyword-only 모드로 떨어집니다.
- embedding artifact가 없거나 깨져 있으면 해당 recording의 similarity만 제외됩니다.
- 초기 설계는 작은 데이터셋 기준이라 similarity는 artifact full-scan 방식으로 계산합니다.

## 9. 체크리스트

실행 전에 아래만 확인하면 됩니다.

- 저장소를 `--recurse-submodules`로 받았는가
- `APP_STORAGE_ROOT`가 쓰기 가능한가
- `ffmpeg`가 `PATH`에 잡히는가
- `whisper-server`, `llama-server`가 각각 8080, 8081, 8082에서 뜨는가
- 환경 변수를 셸에 실제로 주입했는가
