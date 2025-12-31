# RecordRoute
음성, 영상, PDF 파일을 텍스트로 변환하고 회의록으로 요약하는 통합 워크플로우 시스템입니다. STT(Speech-to-Text), 텍스트 추출, 요약, 벡터 검색 기능을 제공합니다.

## 주요 기능

- **고성능 Rust 백엔드**: 전체 백엔드가 Rust로 재작성되어 더 빠르고 안정적인 처리를 보장합니다.
- **다양한 오디오 포맷 지원**: MP3, WAV, M4A, MP4 등 FFmpeg가 지원하는 대부분의 오디오/비디오 파일을 처리합니다.
- **음성→텍스트 변환**: `whisper.cpp`를 사용한 고품질 음성 인식.
- **구조화된 요약**: Ollama를 이용해 체계적인 회의록 형태의 요약을 생성합니다.
- **자동화된 워크플로우**: 전사, 요약, 임베딩까지 이어지는 자동화된 처리 파이프라인.
- **실시간 웹 인터페이스**:
    - 파일 업로드 및 단계별 작업 선택.
    - WebSocket을 통한 실시간 작업 진행 상황 모니터링.
    - 작업 취소, 기록 삭제 등 강력한 관리 기능.
- **임베딩 및 RAG**:
    - Ollama를 통한 임베딩 모델로 문서 벡터화 및 시맨틱 검색.
    - 유사 문서 추천 및 키워드 검색.
- **유틸리티**:
    - 한 줄 요약 생성.
    - 키워드 빈도 분석.

## 디렉토리 구조

```
RecordRoute/
├── README.md              # 프로젝트 소개 및 설치 가이드
├── package.json          # Node.js 프로젝트 설정 (Electron)
├── electron/             # Electron 데스크톱 애플리케이션
├── frontend/             # 웹 인터페이스 (HTML/CSS/JS)
├── scripts/              # 빌드 및 실행 스크립트
│
├── recordroute-rs/       # 메인 Rust 백엔드
│   ├── Cargo.toml        # Rust 워크스페이스 설정
│   ├── API.md            # API 상세 문서
│   ├── ARCHITECTURE.md   # 아키텍처 상세 문서
│   └── crates/           # 워크스페이스 크레이트
│       ├── common        # 공통 모듈 (설정, 에러, 로거)
│       ├── llm           # Ollama API 클라이언트 (요약, 임베딩)
│       ├── stt           # STT 엔진 (whisper.cpp)
│       ├── vector        # 벡터 검색 엔진
│       ├── server        # Axum 웹 서버 및 API 라우트
│       └── recordroute   # 실행 바이너리
│
└── sttEngine/            # 레거시 Python 백엔드
    ├── server.py         # 구버전 FastAPI/WebSocket 서버
    └── requirements.txt  # 구버전 Python 의존성
```

## 설치 및 설정

### 1. 사전 요구사항

- **Rust**: `rustup`을 통해 설치하는 것을 권장합니다.
  - [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)
- **FFmpeg**: 시스템 PATH에 등록되어 있어야 합니다.
  - **macOS**: `brew install ffmpeg`
  - **Windows**: `choco install ffmpeg`
  - **Linux**: `sudo apt-get install ffmpeg`
- **Ollama**: 로컬 LLM을 구동하기 위해 설치 및 실행되어 있어야 합니다.
  - [https://ollama.com/](https://ollama.com/)

### 2. 백엔드 실행 (Rust)

```bash
# 1. 저장소 복제
git clone https://github.com/your-repo/RecordRoute.git
cd RecordRoute

# 2. Rust 백엔드 빌드 및 실행 (첫 실행 시 시간이 걸릴 수 있습니다)
# .env 파일에 설정이 없는 경우, 기본값으로 실행됩니다.
cd recordroute-rs
cargo run --release
```
서버가 `http://localhost:8080`에서 실행됩니다.

### 3. Ollama 모델 다운로드

워크플로우에 필요한 모델을 미리 다운로드합니다.
```bash
# 요약 모델 (권장)
ollama pull llama3.2

# 임베딩 모델 (검색용, 권장)
ollama pull nomic-embed-text
```

**다른 권장 모델 옵션**:
```bash
# 요약 모델 대안
ollama pull gemma2      # Google Gemma 2
ollama pull qwen2.5     # Alibaba Qwen 2.5

# 임베딩 모델 대안
ollama pull mxbai-embed-large  # 높은 정확도
```

*사용할 모델은 `.env` 파일 또는 `recordroute-rs/CONFIGURATION.md`를 참고하여 설정할 수 있습니다.*

### 4. Electron 데스크톱 앱

RecordRoute는 Electron 기반 데스크톱 애플리케이션으로도 사용할 수 있습니다.

#### 개발 모드 실행
```bash
# 1. Rust 백엔드를 실행 상태로 둡니다.
#    (cd recordroute-rs && cargo run --release)

# 2. Node.js 의존성 설치
npm install

# 3. Electron 앱 시작
npm start
```
*`npm start`는 `electron/main.js`에서 Rust 백엔드 프로세스를 자동으로 실행하려고 시도할 수 있습니다. 자세한 내용은 `package.json`의 스크립트를 확인하세요.*

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

- **Ollama 연결 오류**: Ollama 서비스가 로컬에서 실행 중인지 확인하세요 (`ollama serve`).
- **FFmpeg 오류**: FFmpeg가 시스템에 설치되고 PATH에 등록되었는지 확인하세요.
- **Cargo 빌드 오류**:
  - Rust toolchain이 최신 버전인지 확인하세요 (`rustup update`).
  - C++ 빌드 도구가 필요할 수 있습니다 (특히 `whisper.cpp` 또는 `llama.cpp`의 의존성 빌드 시).
- **로그 확인**: 문제가 발생하면 `recordroute-rs/data/logs/` 디렉토리 (경로는 설정에 따라 다름)에 생성된 로그 파일을 확인하여 원인을 파악할 수 있습니다.

## 참고사항

- 이 프로젝트는 Python에서 Rust로의 성공적인 마이그레이션 사례 연구를 포함합니다.
- 레거시 Python 코드는 `sttEngine`에 보존되어 있습니다.
- 구현 예정 기능은 [TODO](/TODO/TODO.md) 문서에 정리되어 있습니다.
