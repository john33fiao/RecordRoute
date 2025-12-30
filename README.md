# RecordRoute
음성, 영상, PDF 파일을 텍스트로 변환하고 회의록으로 요약하는 통합 워크플로우 시스템입니다. STT(Speech-to-Text), 텍스트 추출, 요약, 벡터 검색 기능을 제공합니다.

## 주요 기능

- **다중 포맷 지원**: 오디오, 비디오, PDF 등 다양한 형식의 파일을 처리합니다.
- **음성→텍스트 변환**: OpenAI Whisper를 사용한 고품질 음성 인식.
- **구조화된 요약**: LLM(Ollama)을 이용해 체계적인 회의록 형태의 요약을 생성합니다.
- **통합 워크플로우**: STT부터 요약, 임베딩까지 이어지는 자동화된 처리 파이프라인.
- **실시간 웹 인터페이스**:
    - 파일 업로드 및 단계별 작업 선택.
    - WebSocket을 통한 실시간 작업 진행 상황 모니터링.
    - 작업 취소, 기록 삭제, 결과물 수정 등 강력한 관리 기능.
- **임베딩 및 RAG**:
    - 문서 벡터화를 통한 시맨틱 검색.
    - 유사 문서 추천 및 키워드 검색.
- **유틸리티**:
    - 한 줄 요약 생성.
    - 키워드 빈도 분석.

## 디렉토리 구조

```
RecordRoute/
├── README.md              # 프로젝트 소개 및 설치 가이드
├── CLAUDE.md             # Claude AI 전용 프로젝트 가이드
├── GEMINI.md             # Gemini AI 에이전트 및 개발자 가이드
├── package.json          # Node.js 프로젝트 설정 (Electron)
├── main.js               # Electron 메인 프로세스
├── preload.js            # Electron preload 스크립트 (보안)
├── RecordRouteAPI.spec   # PyInstaller 빌드 스펙
├── build-backend.sh      # Python 백엔드 빌드 스크립트 (Unix)
├── build-backend.bat     # Python 백엔드 빌드 스크립트 (Windows)
├── build-all.sh          # 전체 빌드 스크립트
├── run.bat               # Windows 웹 서버 실행 스크립트
├── run.command           # macOS/Linux 웹 서버 실행 스크립트
├── frontend/             # 웹 인터페이스
│   ├── upload.html       # 업로드 및 작업 관리 UI
│   ├── upload.js         # 프론트엔드 로직
│   └── upload.css        # 프론트엔드 스타일
└── sttEngine/            # 엔진 및 서버 모듈
    ├── bootstrap.py         # 모델 부트스트래핑 스크립트
    ├── config.py            # 환경변수 기반 설정 관리
    ├── logger.py            # 파일 기반 로깅 유틸리티
    ├── ollama_utils.py      # Ollama 서버 상태 확인 및 모델 관리
    ├── embedding_pipeline.py  # 문서 임베딩 및 벡터 생성
    ├── one_line_summary.py    # 한 줄 요약 유틸리티
    ├── keyword_frequency.py   # 키워드 빈도 분석 유틸리티
    ├── search_cache.py        # 검색 결과 캐싱
    ├── requirements.txt       # Python 의존성
    ├── run_workflow.py        # CLI 기반 워크플로우 실행기
    ├── server.py              # HTTP/WebSocket 서버 및 API 엔드포인트
    ├── vector_search.py       # 벡터 검색 기능
    └── workflow/              # 핵심 처리 모듈
        ├── transcribe.py      # 음성→텍스트 변환
        ├── correct.py         # (현재 비활성화) 텍스트 교정
        └── summarize.py       # 텍스트 요약
```

## 설치 및 설정

### 1. 사전 요구사항

- **Python 3.8 이상**
- **FFmpeg**:
  - **개발 환경**: 시스템 PATH에 등록되어 있어야 합니다.
    - Windows: `choco install ffmpeg` 또는 [공식 사이트](https://ffmpeg.org/download.html#build-windows)
    - macOS: `brew install ffmpeg`
    - Linux: `sudo apt-get install ffmpeg` (Ubuntu/Debian)
  - **프로덕션 빌드**: 플랫폼별 FFmpeg 바이너리를 `bin/ffmpeg/` 디렉토리에 배치해야 합니다. 자세한 내용은 `bin/ffmpeg/README.md` 참조
- **Ollama**: 로컬 LLM을 구동하기 위해 설치 및 실행되어 있어야 합니다.

### 2. 자동 설치 (권장)

#### Windows
```bash
# 1단계: 가상환경 생성 및 의존성 설치
setup.bat

# 2단계: 웹 서버 실행
run.bat
```

#### macOS/Linux
```bash
# 1단계: 의존성 설치
pip install -r sttEngine/requirements.txt

# 2단계: 웹 서버 실행 (.env 파일에서 환경변수 자동 로드)
./run.command
```

### 3. Ollama 모델 다운로드

워크플로우에 필요한 모델을 미리 다운로드합니다.
```bash
# 요약 모델 (플랫폼 공통)
ollama pull gemma2:9b

# 임베딩 모델 (RAG 및 검색용)
ollama pull mxbai-embed-large
```
*사용자 환경에 따라 `config.py` 또는 `.env` 파일에서 다른 모델을 지정할 수 있습니다.*

### 4. Electron 데스크톱 앱 (Phase 3 완료)

RecordRoute는 Electron 기반 데스크톱 애플리케이션으로도 사용할 수 있습니다.

#### 개발 모드 실행
```bash
# Node.js 의존성 설치
npm install

# Electron 앱 시작 (Python 백엔드 자동 실행)
npm start
```

#### 프로덕션 빌드
```bash
# 1단계: Python 백엔드 빌드 (PyInstaller)
./build-backend.sh    # Unix/macOS
build-backend.bat     # Windows

# 2단계: Electron 앱 빌드
npm run build         # 현재 플랫폼용 빌드
npm run build:win     # Windows 설치 파일 생성
npm run build:mac     # macOS DMG 생성
npm run build:linux   # Linux AppImage 생성

# 또는 전체 빌드 한 번에
./build-all.sh --target win    # Windows
./build-all.sh --target mac    # macOS
./build-all.sh --target linux  # Linux
```

**빌드 출력:**
- Python 백엔드: `bin/RecordRouteAPI/`
- Electron 앱: `dist/`

**주의사항:**
- Python 백엔드 빌드 전에 가상환경을 활성화해야 합니다
- PyInstaller 필요: `pip install pyinstaller`
- **FFmpeg 바이너리 필요**: 프로덕션 빌드를 위해서는 플랫폼별 FFmpeg 바이너리를 `bin/ffmpeg/` 디렉토리에 배치해야 합니다. 자세한 내용은 `bin/ffmpeg/README.md` 참조
- 빌드 프로세스는 플랫폼에 따라 시간이 걸릴 수 있습니다

## 사용법

### 1. 웹 인터페이스

- **실행**: `run.bat`(Windows) 또는 `./run.command`(macOS/Linux)를 실행합니다.
- **접속**: 웹 브라우저에서 `http://localhost:8080`으로 접속합니다.
- **기능**:
    - 파일을 드래그 앤 드롭하거나 선택하여 업로드합니다.
    - 원하는 작업(STT, 요약, 임베딩)을 선택하고 처리 시작 버튼을 누릅니다.
    - 작업 현황은 실시간으로 업데이트되며, 완료된 기록은 목록에서 관리할 수 있습니다.
    - 결과물 보기, 수정, 다운로드, 삭제, 초기화 등 다양한 작업을 수행할 수 있습니다.
    - 검색창을 통해 저장된 모든 문서에 대해 키워드 및 시맨틱 검색을 수행할 수 있습니다.

### 2. CLI 워크플로우 실행 (레거시)

```bash
python sttEngine/run_workflow.py
```
대화형 프롬프트를 통해 파일을 지정하고 단계를 선택하여 실행할 수 있습니다.

## 주요 API 엔드포인트

`sttEngine/server.py`는 다음과 같은 API를 제공합니다.

- `POST /upload`: 파일 업로드.
- `POST /process`: STT, 요약 등 선택된 워크플로우 실행.
- `GET /history`: 처리 완료된 기록 목록 조회.
- `GET /download/<file_uuid>`: UUID로 결과 파일 다운로드.
- `POST /cancel`: 진행 중인 작업 취소.
- `POST /delete_records`: 기록 및 관련 파일 영구 삭제.
- `GET /search?q=<query>`: 키워드 및 벡터 검색.
- `POST /similar`: 특정 문서와 유사한 문서 검색.
- `POST /update_stt_text`: 변환된 텍스트 결과 수정.
- `GET /models`: 사용 가능한 Ollama 모델 목록 조회.
- `ws://localhost:8765`: 실시간 작업 진행 상황을 전송하는 WebSocket 엔드포인트.

## 트러블슈팅

- **Ollama 연결 오류**: Ollama 서비스가 로컬에서 실행 중인지 확인하세요 (`ollama serve`).
- **FFmpeg 오류**: FFmpeg가 시스템에 설치되고 PATH에 등록되었는지 확인하세요.
- **로그 확인**: 문제가 발생하면 `db/log/` 디렉토리에 생성된 로그 파일을 확인하여 원인을 파악할 수 있습니다.

## 참고사항

- 이 프로젝트는 개인적인 학습 및 포트폴리오 목적으로 진행되었습니다.
- 대부분의 코드는 LLM(Claude, Gemini 등)의 도움을 받아 작성되었습니다.
- 구현 예정 기능은 [TODO](/TODO/TODO.md) 문서에 정리되어 있습니다.
