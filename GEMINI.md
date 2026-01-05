# RecordRoute STT Engine

## 1. 프로젝트 개요 (Project Overview)

이 프로젝트는 오디오/비디오 파일로부터 텍스트를 추출하고, 이를 교정 및 요약하는 자동화된 워크플로우를 제공합니다. STT(Speech-to-Text) 엔진을 중심으로 구성되어 있으며, 다음과 같은 3단계 파이프라인을 통해 작동합니다.

 1.  **음성 -> 텍스트 변환 (Transcribe):** `openai-whisper`를 사용하여 미디어 파일에서 텍스트를 추출합니다.
 2.  **텍스트 교정 (Correct):** `Ollama`를 통해 추출된 텍스트의 오탈자, 문법 등을 교정합니다.
 3.  **텍스트 요약 (Summarize):** 교정된 텍스트를 `Ollama`를 사용해 구조화된 형식으로 요약합니다.

간단한 웹 업로드 페이지를 통해 이러한 작업을 선택적으로 실행할 수 있으며, 작업 큐와 업로드 기록 관리, 결과 오버레이 뷰어, 요약 전 확인 팝업, 업로드 기록 초기화 기능을 제공합니다.
추가로 문서 임베딩과 벡터 검색, 한 줄 요약 기능을 통해 결과 활용성을 높였습니다.

## 2. 기술 스택 (Tech Stack)

-   **언어:** Python 3
-   **음성 인식:** `openai-whisper`
-   **LLM (텍스트 교정/요약):** `Ollama` (gemma3:4b, gemma3:12b-it-qat, gpt-oss:20b 등)
-   **임베딩 모델:** `bge-m3:latest` (Ollama)
-   **핵심 의존성:**
    -   `openai-whisper`: Python 라이브러리
    -   `ollama`: Python 라이브러리
    -   `torch`, `torchaudio`, `torchvision`: PyTorch (GPU/CUDA 지원)
    -   `websockets`: WebSocket 실시간 통신
    -   `sentence-transformers`: 벡터 임베딩
    -   `ffmpeg`: 시스템 프로그램 (오디오 처리 및 m4a→wav 변환)
    -   `Ollama`: 시스템 서비스 (로컬 LLM 구동을 위해 필요)

## 3. 디렉토리 구조 (Directory Structure)

```
RecordRoute/
├── README.md                  # 프로젝트 소개 및 설치 가이드
├── TODO/                      # 기능 구현 계획 디렉토리
├── LICENSE                    # 라이선스 정보
├── CLAUDE.md                  # Claude AI 전용 프로젝트 가이드
├── GEMINI.md                  # Gemini AI 에이전트 및 개발자 가이드
├── .env.example               # 환경변수 템플릿
├── run.sh                     # Unix 웹 서버 실행 스크립트
├── run.bat                    # Windows 웹 서버 실행 스크립트
├── setup.sh                   # Unix 설정 스크립트
├── setup.bat                  # Windows 설정 스크립트
├── requirements.txt           # Python 의존성 목록
├── frontend/
│   ├── upload.html            # 웹 업로드 및 작업 관리 UI
│   ├── upload.js              # 프론트엔드 로직
│   └── upload.css             # 프론트엔드 스타일
└── sttEngine/
    ├── config.py              # 환경변수 기반 설정 관리, DB 경로 관리
    ├── logger.py              # 로깅 시스템 (자동 롤오버)
    ├── vocabulary_manager.py  # STT 정확도 향상용 어휘 관리
    ├── keyword_frequency.py   # 키워드 빈도 분석 유틸리티
    ├── search_cache.py        # 검색 결과 캐싱 (24시간)
    ├── embedding_pipeline.py  # 문서 임베딩 및 벡터 생성 (bge-m3)
    ├── ollama_utils.py        # Ollama 서버 및 모델 관리 유틸리티
    ├── one_line_summary.py    # 한 줄 요약 유틸리티
    ├── run_workflow.py        # CLI 워크플로우 통합 실행기
    ├── server.py              # HTTP/WebSocket 서버, 업로드 처리
    ├── vector_search.py       # 벡터 검색 기능
    └── workflow/
        ├── transcribe.py      # 1단계: 음성 변환 로직
        ├── correct.py         # 2단계: 텍스트 교정 로직
        └── summarize.py       # 3단계: 텍스트 요약 로직
```

## 4. 설치 및 설정 (Setup)

프로젝트를 처음 사용하거나 의존성을 업데이트할 때 사용합니다.

1.  `sttEngine` 디렉토리로 이동합니다.
2.  `setup.bat` 스크립트를 실행합니다.

이 스크립트는 다음 작업을 자동으로 수행합니다.
-   `venv`라는 이름의 Python 가상환경 생성
-   `requirements.txt`에 명시된 Python 라이브러리 설치
-   시스템에 `ffmpeg`과 `ollama`가 설치되어 있는지 확인하고, 없을 경우 설치 안내 메시지 출력

**AI 에이전트 명령어:**
```
run_shell_command(command="cd sttEngine && setup.bat")
```

## 5. 실행 방법 (How to Run)

루트 디렉토리에서 제공하는 실행 스크립트로 웹 서버를 시작합니다.

1. 의존성 설치 후 실행 스크립트를 호출합니다.
2. 브라우저에서 `http://localhost:8080` 에 접속하여 파일을 업로드하고 작업을 선택합니다.
3. 작업은 백그라운드에서 비동기적으로 처리되며, UI는 주기적으로 서버에 진행 상태를 문의하여 업데이트됩니다.

**AI 에이전트 명령어:**

**Windows:**
```
run_shell_command(command="run.bat")
```

**macOS/Linux:**
```
run_shell_command(command="./run.command")
```

**플랫폼별 기본 모델:**
- Windows: `gemma3:4b`
- macOS/Linux: 교정 `gemma3:12b-it-qat`, 요약 `gpt-oss:20b`

*참고: 기존 `sttEngine/run_workflow.py`는 CLI용으로 남아 있으며 대화형 입력을 요구합니다.*

## 6. AI 에이전트 활용 가이드 (Gemini Usage Guide)

다음은 AI 에이전트가 이 프로젝트에서 수행할 수 있는 주요 작업 목록입니다.

### 의존성 관리

-   **새로운 Python 라이브러리 추가:**
    1.  `sttEngine/requirements.txt` 파일에 라이브러리(예: `new-library==1.0.0`)를 추가합니다.
    2.  설치 스크립트를 다시 실행하여 라이브러리를 설치합니다.

    **예시 명령어:**
    ```
    // 1. 파일에 내용 추가
    replace(
        file_path="/path/to/RecordRoute/sttEngine/requirements.txt",
        old_string="ollama>=0.1.0",
        new_string="ollama>=0.1.0\nnew-library==1.0.0"
    )
    // 2. Windows 설치 스크립트 실행
    run_shell_command(command="sttEngine\setup.bat")
    
    // 2-1. Unix/macOS/Linux에서는 직접 pip 설치
    run_shell_command(command="./venv/bin/pip install new-library==1.0.0")
    ```

### 워크플로우 스크립트 수정

-   **요약 프롬프트 변경:** `sttEngine/workflow/summarize.py` 파일의 `BASE_PROMPT` 변수를 수정합니다.
-   **교정 프롬프트 변경:** `sttEngine/workflow/correct.py` 파일의 `SYSTEM_PROMPT` 변수를 수정합니다.
-   **Whisper 모델 변경:** `sttEngine/workflow/transcribe.py`의 `--model_size` 인자 기본값을 변경합니다.

**예시 명령어 (요약 프롬프트 수정):**
```
// 1. 파일 읽기
read_file(absolute_path="/path/to/RecordRoute/sttEngine/workflow/summarize.py")
// 2. 내용 확인 후 replace 실행 (old_string, new_string은 파일 내용에 맞게 구성)
replace(...)
```

### 웹 UI 및 비동기 작업 디버깅

웹 UI 관련 문제나 작업 처리 중 멈춤 현상 발생 시 다음을 확인합니다.

1.  **서버 로직 확인 (`sttEngine/server.py`):**
    -   `/process`: 작업을 시작하는 API 엔드포인트. `run_workflow` 함수를 백그라운드 스레드에서 실행하는지 확인합니다.
    -   `/progress/<task_id>`: 작업 진행 상태를 반환하는 엔드포인트.
    -   `/history`: 작업 완료 기록을 반환하는 엔드포인트.

2.  **프론트엔드 로직 확인 (`frontend/upload.js`):**
    -   `processFile()`: `/process` API를 호출하고 서버로부터 `task_id`를 받는지 확인합니다.
    -   `pollProgress()`: `task_id`를 사용해 주기적으로 `/progress/<task_id>`를 호출하여 UI를 업데이트하는지 확인합니다.
    -   브라우저의 개발자 도구(F12) 콘솔에서 자바스크립트 오류가 발생하는지 확인하는 것이 매우 중요합니다.

3.  **비동기 작업 흐름 이해:**
    -   사용자가 웹 UI에서 "처리 시작"을 누르면, `server.py`는 작업을 즉시 백그라운드로 넘기고 `202 Accepted`와 `task_id`를 응답합니다.
    -   `upload.js`는 이 `task_id`를 받아 `pollProgress` 함수를 통해 작업이 완료될 때까지 주기적으로 서버에 상태를 묻습니다.
    -   이 과정에서 문제가 발생하면, 서버 로그와 브라우저 콘솔 로그를 함께 분석하여 원인을 찾아야 합니다.
