# RecordRoute
음성, 영상, PDF 파일을 텍스트로 변환하고 회의록으로 요약하는 통합 워크플로우 시스템입니다. STT(Speech-to-Text), 텍스트 추출, 요약, 벡터 검색 기능을 제공합니다.

## 주요 기능

- **고성능 Rust 백엔드**: 전체 백엔드가 Rust로 재작성되어 더 빠르고 안정적인 처리를 보장합니다.
- **다양한 오디오 포맷 지원**: MP3, WAV, M4A, MP4 등 FFmpeg가 지원하는 대부분의 오디오/비디오 파일을 처리합니다.
- **음성→텍스트 변환**: `whisper.cpp`를 사용한 고품질 음성 인식.
- **구조화된 요약**: llama.cpp를 이용해 체계적인 회의록 형태의 요약을 생성합니다.
- **자동화된 워크플로우**: 전사, 요약, 임베딩까지 이어지는 자동화된 처리 파이프라인.
- **실시간 웹 인터페이스**:
    - 파일 업로드 및 단계별 작업 선택.
    - WebSocket을 통한 실시간 작업 진행 상황 모니터링.
    - 작업 취소, 기록 삭제 등 강력한 관리 기능.
- **임베딩 및 RAG**:
    - llama.cpp를 통한 임베딩 모델로 문서 벡터화 및 시맨틱 검색.
    - 유사 문서 추천 및 키워드 검색.
- **유틸리티**:
    - 한 줄 요약 생성.
    - 키워드 빈도 분석.

## 디렉토리 구조

```
RecordRoute/
├── README.md                 # 프로젝트 소개 및 설치 가이드
├── package.json              # NPM Workspaces 루트 설정
├── .gitignore                # Git 제외 파일 목록
├── .gitmodules               # Git 서브모듈 설정
│
├── configs/                  # 환경 설정 템플릿
├── data/                     # 실행 데이터 저장소 (.gitignore)
├── models/                   # AI 모델 저장소 (.gitignore)
│
├── electron/                 # Electron 데스크톱 애플리케이션
│   ├── main.js               # Electron 메인 프로세스
│   ├── preload.js            # Preload 스크립트
│   └── package.json          # Electron 워크스페이스 설정
│
├── frontend/                 # 웹 인터페이스 (HTML/CSS/JS)
│   ├── upload.html/css/js    # 메인 UI
│   └── package.json          # Frontend 워크스페이스 설정
│
├── legacy/                   # 레거시 Python 백엔드 (참고용)
│   └── python-backend/       # 구버전 Python 코드
│       ├── server.py         # 구버전 FastAPI/WebSocket 서버
│       ├── requirements.txt  # 구버전 Python 의존성
│       └── workflow/         # Python 워크플로우 모듈
│
├── third-party/              # 외부 의존성 (서브모듈)
│   └── llama.cpp/            # llama.cpp 서브모듈
│       └── build/            # 빌드 출력 (.gitignore)
│
├── tools/                    # 개발 도구 및 스크립트
│   ├── scripts/              # 빌드 및 실행 스크립트
│   │   ├── setup.sh          # 초기 설정 스크립트 (Linux/macOS)
│   │   ├── setup.bat         # 초기 설정 스크립트 (Windows)
│   │   ├── build-all.sh      # 전체 빌드 스크립트 (Linux/macOS)
│   │   ├── build-all.bat     # 전체 빌드 스크립트 (Windows)
│   │   ├── build-llama.sh    # llama.cpp 빌드 스크립트
│   │   ├── build-llama.bat   # llama.cpp 빌드 (Windows)
│   │   ├── build-backend.sh  # Python 백엔드 빌드
│   │   ├── download-whisper-model.sh  # Whisper 모델 다운로드 (Linux/macOS)
│   │   ├── download-whisper-model.bat # Whisper 모델 다운로드 (Windows)
│   │   ├── start.bat         # Windows 실행 스크립트
│   │   └── run.command       # macOS/Linux 실행 스크립트
│   └── dev-env/              # 개발 환경 설정
│
└── recordroute-rs/           # 메인 Rust 백엔드
    ├── Cargo.toml            # Rust 워크스페이스 설정
    ├── API.md                # API 상세 문서
    ├── ARCHITECTURE.md       # 아키텍처 상세 문서
    └── crates/               # 워크스페이스 크레이트
        ├── common            # 공통 모듈 (설정, 에러, 로거)
        ├── llm               # llama.cpp API 클라이언트 (요약, 임베딩)
        ├── stt               # STT 엔진 (whisper.cpp)
        ├── vector            # 벡터 검색 엔진
        ├── server            # Axum 웹 서버 및 API 라우트
        └── recordroute       # 실행 바이너리
```

## 설치 및 설정

### 1. 사전 요구사항

- **Rust**: `rustup`을 통해 설치하는 것을 권장합니다.
  - [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)
- **CMake**: llama.cpp 빌드에 필요합니다.
  - **macOS**: `brew install cmake`
  - **Windows**: [CMake 공식 사이트](https://cmake.org/download/)에서 다운로드
  - **Linux**: `sudo apt-get install cmake build-essential`
- **FFmpeg**: 시스템 PATH에 등록되어 있어야 합니다.
  - **macOS**: `brew install ffmpeg`
  - **Windows**: `choco install ffmpeg`
  - **Linux**: `sudo apt-get install ffmpeg`

**참고**: llama.cpp는 서브모듈로 포함되어 있어 별도 설치가 필요하지 않습니다. 프로젝트 클론 시 자동으로 포함되며, 아래 빌드 단계에서 함께 빌드됩니다.

### 2. 저장소 클론 및 초기 설정

```bash
# 1. 저장소 복제 (서브모듈 포함)
git clone --recursive https://github.com/your-repo/RecordRoute.git
cd RecordRoute

# 2. Node.js 의존성 설치
# 프로젝트는 npm workspaces를 사용하므로 루트에서 한 번만 실행
npm install

# 3. electron-builder 의존성 설치
npm run install-deps
```

**중요**: npm workspaces를 사용하므로 모든 의존성 설치는 **프로젝트 루트에서만** 실행해야 합니다. 개별 워크스페이스(`electron/`, `frontend/`) 폴더에서 `npm install`을 실행하지 마세요.

**환경 설정 (선택 사항)**:
`.env` 파일은 프로젝트 루트에 생성할 수 있습니다. `.env` 파일이 없는 경우, 기본값으로 실행됩니다. 자세한 내용은 `recordroute-rs/CONFIGURATION.md`를 참고하세요.

### 빠른 시작

초기 설정이 완료되면 바로 실행할 수 있습니다:

```bash
# 개발 모드로 실행 (Electron 앱)
npm start

# 또는 Rust 백엔드만 실행
cd recordroute-rs
cargo run --release
```

서버가 `http://localhost:8080`에서 실행됩니다.

**환경 설정 위치**: `.env` 파일은 **프로젝트 루트** (`RecordRoute/.env`)에 생성해야 합니다.
RecordRoute는 자동으로 프로젝트 루트를 찾아 `.env` 파일을 로드합니다.

### 3. Whisper 모델 다운로드 (필수)

음성 인식을 위한 Whisper 모델을 미리 다운로드해야 합니다. **이 단계를 생략하면 `cargo run` 실행 시 "Model file not found" 오류가 발생합니다.**

**중요**: 모델 파일은 **프로젝트 루트의 `models/` 폴더**에 배치해야 합니다.

#### 모델 다운로드 방법

```bash
# 프로젝트 루트로 이동
cd RecordRoute

# models 디렉토리 생성
mkdir -p models
cd models

# Base 모델 다운로드 (권장, 균형잡힌 성능)
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin

# 또는 curl 사용
curl -L -o ggml-base.bin https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin
```

**모델 선택 가이드**:
- `ggml-tiny.bin` - 가장 빠름, 낮은 정확도 (~75MB)
- `ggml-base.bin` - 균형잡힌 성능 (권장, ~142MB)
- `ggml-small.bin` - 높은 정확도 (~466MB)
- `ggml-medium.bin` - 매우 높은 정확도 (~1.5GB)
- `ggml-large-v3.bin` - 최고 정확도, 한국어 최적화 (~2.9GB)

모델을 다운로드한 후, 프로젝트 루트의 `.env` 파일에서 경로를 설정할 수 있습니다:
```bash
# RecordRoute/.env
WHISPER_MODEL=./models/ggml-base.bin
```

**참고**: 모든 상대 경로는 프로젝트 루트 기준입니다. RecordRoute는 자동으로 `.git` 디렉토리를 찾아 프로젝트 루트를 결정합니다.

*자세한 모델 옵션과 성능 비교는 `recordroute-rs/CONFIGURATION.md`를 참고하세요.*

### 6. LLM 모델 다운로드 및 llama-server 실행

워크플로우에 필요한 GGUF 형식의 모델을 HuggingFace에서 다운로드합니다.

**요약 모델 (권장)**:
- [Llama 3.2 3B GGUF](https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF)
- [Qwen2.5 7B GGUF](https://huggingface.co/Qwen/Qwen2.5-7B-Instruct-GGUF)
- [Gemma 2 2B GGUF](https://huggingface.co/bartowski/gemma-2-2b-it-GGUF)
- [Gemma 3 4B](https://huggingface.co/google/gemma-3-4b-it) - 최신 Gemma 모델

**임베딩 모델 (검색용, 권장)**:
- [nomic-embed-text GGUF](https://huggingface.co/nomic-ai/nomic-embed-text-v1.5-GGUF)
- [mxbai-embed-large GGUF](https://huggingface.co/mixedbread-ai/mxbai-embed-large-v1)

#### Gemma 3 4B 모델 다운로드 방법

**방법 1: Hugging Face CLI 사용 (권장)**

```bash
# Hugging Face CLI 설치
pip install -U "huggingface_hub[cli]"

# 모델 전체 다운로드
huggingface-cli download google/gemma-3-4b-it --local-dir ./models/gemma-3-4b-it

# 특정 파일만 다운로드 (예: GGUF 파일)
hf download bartowski/google_gemma-3-4b-it-GGUF --include "*bf16.gguf" --local-dir ./models/gemma-3-4b-it
```

**방법 2: Git LFS 사용**

```bash
# Git LFS 설치 (아직 설치하지 않은 경우)
# macOS: brew install git-lfs
# Linux: sudo apt-get install git-lfs
# Windows: https://git-lfs.github.com/

# Git LFS 초기화
git lfs install

# 모델 저장소 클론
cd models
git clone https://huggingface.co/google/gemma-3-4b-it
```

**방법 3: 특정 파일만 wget/curl로 다운로드**

```bash
# models 디렉토리 생성
mkdir -p models/gemma-3-4b-it
cd models/gemma-3-4b-it

# 필요한 파일을 개별적으로 다운로드
# (파일 목록은 https://huggingface.co/google/gemma-3-4b-it/tree/main 참조)
wget https://huggingface.co/google/gemma-3-4b-it/resolve/main/config.json
wget https://huggingface.co/google/gemma-3-4b-it/resolve/main/tokenizer.json
# 필요한 다른 파일들도 동일한 방식으로 다운로드
```

다운로드한 모델 파일(`.gguf`)을 적절한 디렉토리에 저장하고, 빌드된 llama-server를 실행할 때 모델 경로를 지정합니다.

```bash
# llama-server 실행 예시 (Linux/macOS)
./third-party/llama.cpp/build/bin/llama-server -m /path/to/model.gguf --host 127.0.0.1 --port 8081

# Windows
third-party\llama.cpp\build\bin\Release\llama-server.exe -m C:\path\to\model.gguf --host 127.0.0.1 --port 8081
```

**팁**: 요약용 서버와 임베딩용 서버를 각각 다른 포트에서 실행할 수 있습니다.
- 요약용: `--port 8081`
- 임베딩용: `--port 8082`

*사용할 모델 경로와 서버 설정은 `.env` 파일 또는 `recordroute-rs/CONFIGURATION.md`를 참고하여 설정할 수 있습니다.*

### 7. Electron 데스크톱 앱

RecordRoute는 Electron 기반 데스크톱 애플리케이션으로도 사용할 수 있습니다.

#### 개발 모드 실행
```bash
# Electron 앱 실행 (개발 모드)
npm start
```

*`npm start`는 Electron 앱을 시작합니다. Rust 백엔드는 별도로 실행해야 할 수 있습니다.*

#### 프로덕션 빌드
```bash
# 현재 플랫폼용 빌드
npm run build

# 특정 플랫폼용 빌드
npm run build:mac     # macOS용 빌드
npm run build:win     # Windows용 빌드
npm run build:linux   # Linux용 빌드
```

빌드된 파일은 `dist/` 폴더에 생성됩니다.

---

## 사용법

### 1. 웹 인터페이스

- **실행**: `recordroute-rs` 디렉토리에서 `cargo run --release`를 실행합니다.
- **접속**: 웹 브라우저에서 `http://localhost:8080`으로 접속합니다.
- **기능**:
    - 파일을 드래그 앤 드롭하거나 선택하여 업로드합니다.
    - 원하는 작업(STT, 요약, 임베딩)을 선택하고 처리 시작 버튼을 누릅니다.
    - 작업 현황은 실시간으로 업데이트되며, 완료된 기록은 목록에서 관리할 수 있습니다.
    - 결과물 보기, 다운로드, 삭제 등 다양한 작업을 수행할 수 있습니다.
    - 검색창을 통해 저장된 모든 문서에 대해 시맨틱 검색을 수행할 수 있습니다.

---

## 주요 API 엔드포인트

Rust 백엔드는 `recordroute-rs/API.md`에 문서화된 REST API를 제공합니다.

- `POST /upload`: 파일 업로드.
- `POST /process`: STT, 요약, 임베딩 등 워크플로우 실행.
- `GET /tasks`: 현재 진행 중인 작업 목록 조회.
- `POST /cancel`: 진행 중인 작업 취소.
- `GET /history`: 처리 완료된 기록 목록 조회.
- `POST /delete`: 하나 이상의 기록을 삭제.
- `GET /download/{filename}`: 결과물 파일 다운로드.
- `GET /search`: 키워드 및 벡터 검색.
- `GET /search/stats`: 벡터 인덱스 통계 조회.
- `WebSocket`: `http://localhost:8080`의 WebSocket을 통해 실시간 작업 진행 상황을 전송합니다.

*자세한 요청/응답 형식은 `recordroute-rs/API.md` 문서를 참고하세요.*

---

## 트러블슈팅

- **npm install 오류**: `Cannot compute electron version` 에러가 발생하면:
  - **중요**: npm workspaces를 사용하므로 **프로젝트 루트에서만** 설치를 실행하세요:
    ```bash
    # 프로젝트 루트에서 실행
    npm install
    npm run install-deps
    ```
  - 개별 워크스페이스 폴더(`electron/`, `frontend/`)에서 `npm install`을 실행하지 마세요.
  - 이 문제는 워크스페이스 의존성이 제대로 설치되지 않았거나 electron-builder가 electron이 설치되기 전에 실행되어 발생합니다.
  - 참고: NPM workspaces의 호이스팅 메커니즘 때문에 각 워크스페이스 디렉토리에서 직접 `npm install`을 실행해야 합니다.

- **Whisper 모델 오류**: `Error: STT error: Model file not found`가 발생하면:
  - Whisper 모델을 다운로드했는지 확인하세요 (위 "3. Whisper 모델 다운로드" 참조).
  - 모델 파일 경로가 올바른지 확인하세요 (`ls recordroute-rs/models/ggml-base.bin`).
  - 프로젝트 루트의 `.env` 파일 (`RecordRoute/.env`)에서 `WHISPER_MODEL` 경로 설정을 확인하세요.
  - 자세한 내용은 `recordroute-rs/CONFIGURATION.md`를 참고하세요.
- **llama.cpp 빌드 오류**:
  - CMake가 설치되어 있는지 확인하세요 (`cmake --version`).
  - C++ 컴파일러가 설치되어 있는지 확인하세요 (gcc, clang, MSVC 등).
  - 서브모듈이 올바르게 초기화되었는지 확인하세요 (`git submodule update --init --recursive`).
- **llama.cpp 연결 오류**: 빌드된 llama-server가 실행 중인지 확인하세요:
  ```bash
  # Linux/macOS
  ./third-party/llama.cpp/build/bin/llama-server -m /path/to/model.gguf --host 127.0.0.1 --port 8081

  # Windows
  third-party\llama.cpp\build\bin\Release\llama-server.exe -m C:\path\to\model.gguf --host 127.0.0.1 --port 8081
  ```
- **FFmpeg 오류**: FFmpeg가 시스템에 설치되고 PATH에 등록되었는지 확인하세요.
- **Cargo 빌드 오류**:
  - Rust toolchain이 최신 버전인지 확인하세요 (`rustup update`).
  - C++ 빌드 도구가 필요할 수 있습니다 (특히 `whisper.cpp` 의존성 빌드 시).
- **로그 확인**: 문제가 발생하면 `recordroute-rs/data/logs/` 디렉토리 (경로는 설정에 따라 다름)에 생성된 로그 파일을 확인하여 원인을 파악할 수 있습니다.

## 참고사항

- 이 프로젝트는 Python에서 Rust로의 성공적인 마이그레이션 사례 연구를 포함합니다.
- 레거시 Python 코드는 `legacy/python-backend`에 보존되어 있습니다 (더 이상 유지보수하지 않음).
- NPM Workspaces 구조로 전환되어 `electron`과 `frontend`가 독립적인 워크스페이스로 관리됩니다.
- 구현 예정 기능은 [TODO](/TODO/TODO.md) 문서에 정리되어 있습니다.
