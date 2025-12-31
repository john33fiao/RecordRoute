# RecordRoute 개발 환경 설정 가이드

이 문서는 RecordRoute의 개발 환경을 수동으로 설정하는 방법을 설명합니다.

## 빠른 시작

```bash
# 1. 저장소 클론 (서브모듈 포함)
git clone --recursive https://github.com/john33fiao/RecordRoute.git
cd RecordRoute

# 2. Node.js 의존성 설치
npm install

# 3. electron-builder 의존성 설치
npm run install-deps

# 4. Electron 앱 실행
npm start
```

## 프로젝트 아키텍처

RecordRoute는 **Rust 기반 백엔드**와 Electron 데스크톱 애플리케이션으로 구성됩니다:

- **백엔드**: Rust (`recordroute-rs/`) - STT, LLM, 벡터 검색을 위한 고성능 백엔드
- **프론트엔드**: Electron (`electron/`) + Web UI (`frontend/`) - 데스크톱 애플리케이션 인터페이스
- **레거시**: Python 백엔드 (`legacy/python-backend/`) - 참고용으로만 보관, 현재 사용하지 않음

**주의**: Python 백엔드는 더 이상 사용되지 않습니다. 모든 백엔드 기능은 Rust로 구현되어 있습니다.

## 시스템 요구사항

### 필수 도구

1. **Node.js 18+** 및 **npm** - Electron 앱용
2. **Rust** (rustup을 통한 설치 권장) - 백엔드용
3. **CMake** - llama.cpp 빌드용
4. **FFmpeg** - 오디오/비디오 처리용 (시스템 PATH에 등록 필요)

### 설치 방법

#### macOS
```bash
brew install cmake ffmpeg
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### Windows
```powershell
# Chocolatey 사용
choco install cmake ffmpeg

# Rust 설치
# https://rustup.rs/ 에서 rustup-init.exe 다운로드 후 실행
```

#### Linux (Ubuntu/Debian)
```bash
sudo apt-get update
sudo apt-get install cmake build-essential ffmpeg
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## 상세 설정 단계

### 1. 저장소 클론

```bash
# 서브모듈 포함하여 클론
git clone --recursive https://github.com/john33fiao/RecordRoute.git
cd RecordRoute

# 이미 클론한 경우 서브모듈 업데이트
git submodule update --init --recursive
```

### 2. Node.js 의존성 설치

**중요**: RecordRoute는 **npm workspaces**를 사용합니다. 모든 설치는 **프로젝트 루트에서만** 실행해야 합니다.

```bash
# 프로젝트 루트에서 실행
npm install

# electron-builder 의존성 설치
npm run install-deps
```

**절대 하지 말 것**:
```bash
# ❌ 잘못된 방법 - 개별 워크스페이스에서 설치하지 마세요
cd electron && npm install  # 잘못됨!
cd frontend && npm install  # 잘못됨!
```

이렇게 하면 `Cannot compute electron version` 오류가 발생합니다.

### 3. Rust 백엔드 빌드 (선택 사항)

개발 모드에서는 필수가 아니지만, 백엔드 기능을 테스트하려면 빌드가 필요합니다:

```bash
cd recordroute-rs
cargo build --release
cd ..
```

### 4. llama.cpp 빌드 (LLM 기능 사용 시)

요약 및 임베딩 기능을 사용하려면 llama.cpp가 필요합니다:

```bash
npm run build:llama
```

또는 수동으로:
```bash
cd third-party/llama.cpp
mkdir -p build
cd build
cmake ..
cmake --build . --config Release
cd ../../..
```

### 5. Whisper 모델 다운로드 (STT 기능 사용 시)

음성 인식 기능을 사용하려면 Whisper 모델이 필요합니다:

```bash
# models 디렉토리 생성
mkdir -p models
cd models

# Base 모델 다운로드 (권장)
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin

# 또는 curl 사용
curl -L -o ggml-base.bin https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin

cd ..
```

모델 선택:
- `ggml-tiny.bin` - 가장 빠름 (~75MB)
- `ggml-base.bin` - 균형잡힌 성능 (권장, ~142MB)
- `ggml-small.bin` - 높은 정확도 (~466MB)
- `ggml-large-v3.bin` - 최고 정확도, 한국어 최적화 (~2.9GB)

## 개발 워크플로우

### Electron 앱 실행

```bash
# 개발 모드로 실행
npm start
```

앱이 자동으로 Rust 백엔드에 연결을 시도합니다.

### Rust 백엔드 별도 실행

```bash
cd recordroute-rs
cargo run --release
```

서버는 `http://localhost:8080`에서 실행됩니다.

### 프로덕션 빌드

```bash
# 현재 플랫폼용 빌드
npm run build

# 특정 플랫폼용 빌드
npm run build:mac     # macOS
npm run build:win     # Windows
npm run build:linux   # Linux
```

빌드 결과물은 `dist/` 폴더에 생성됩니다.

## 프로젝트 구조

```
RecordRoute/
├── electron/              # Electron 앱 (워크스페이스)
│   ├── main.js            # 메인 프로세스
│   ├── preload.js         # Preload 스크립트
│   └── package.json       # Electron 의존성
│
├── frontend/              # Web UI (워크스페이스)
│   ├── upload.html        # 메인 UI
│   └── package.json       # Frontend 의존성
│
├── recordroute-rs/        # 메인 Rust 백엔드
│   ├── Cargo.toml         # Rust 워크스페이스
│   └── crates/            # Rust 모듈
│       ├── common/        # 공통 모듈
│       ├── stt/           # STT 엔진 (whisper.cpp)
│       ├── llm/           # LLM 통합 (llama.cpp)
│       ├── vector/        # 벡터 검색
│       └── server/        # 웹 서버
│
├── third-party/
│   └── llama.cpp/         # LLM 엔진 (서브모듈)
│
├── legacy/
│   └── python-backend/    # 구버전 Python 코드 (참고용)
│
├── models/                # AI 모델 저장소 (.gitignore)
├── data/                  # 런타임 데이터 (.gitignore)
└── package.json           # npm workspaces 루트 설정
```

## 자주 사용하는 명령어

| 명령어 | 설명 |
|--------|------|
| `npm start` | Electron 앱 실행 (개발 모드) |
| `npm run build` | 현재 플랫폼용 프로덕션 빌드 |
| `npm run build:rust` | Rust 백엔드 빌드 |
| `npm run build:llama` | llama.cpp 빌드 |
| `npm run build:all` | llama.cpp + Rust 백엔드 빌드 |
| `cargo run --release` | Rust 백엔드만 실행 (recordroute-rs/ 에서) |

## 문제 해결

### "Cannot compute electron version" 오류

**원인**: Electron이 워크스페이스에 설치되지 않음

**해결**:
```bash
# 프로젝트 루트에서 실행
npm install
npm run install-deps
```

### "Model file not found" 오류

**원인**: Whisper 모델이 다운로드되지 않음

**해결**: 위의 "5. Whisper 모델 다운로드" 섹션 참조

### npm install이 작동하지 않음

**원인**: 개별 워크스페이스 폴더에서 실행

**해결**: 항상 프로젝트 루트에서 `npm install` 실행

### Rust 빌드 오류

**원인**: Rust 또는 CMake가 설치되지 않음

**해결**: 위의 "시스템 요구사항" 섹션에서 필수 도구 설치

## 추가 리소스

- [README.md](README.md) - 프로젝트 개요 및 기능 설명
- [recordroute-rs/API.md](recordroute-rs/API.md) - 백엔드 API 문서
- [recordroute-rs/ARCHITECTURE.md](recordroute-rs/ARCHITECTURE.md) - 시스템 아키텍처 문서

## 기여하기

기여를 환영합니다! 다음 사항을 확인해 주세요:

1. `npm start`로 변경사항을 테스트하세요
2. Rust 백엔드가 성공적으로 빌드되는지 확인하세요
3. 대상 플랫폼(Windows, macOS, Linux)에서 테스트하세요
4. 필요한 경우 문서를 업데이트하세요
