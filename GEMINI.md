# RecordRoute STT Engine

## 1. 프로젝트 개요 (Project Overview)

이 프로젝트는 오디오/비디오 파일로부터 텍스트를 추출하고, 이를 교정 및 요약하는 자동화된 워크플로우를 제공합니다. STT(Speech-to-Text) 엔진을 중심으로 구성되어 있으며, 다음과 같은 3단계 파이프라인을 통해 작동합니다.

 1.  **음성 -> 텍스트 변환 (Transcribe):** `openai-whisper`를 사용하여 미디어 파일에서 텍스트를 추출합니다.
 2.  **텍스트 교정 (Correct):** `Ollama`를 통해 추출된 텍스트의 오탈자, 문법 등을 교정합니다.
 3.  **텍스트 요약 (Summarize):** 교정된 텍스트를 `Ollama`를 사용해 구조화된 형식으로 요약합니다.

간단한 웹 업로드 페이지를 통해 이러한 작업을 선택적으로 실행할 수 있으며, 작업 큐와 업로드 기록 관리, 결과 오버레이 뷰어, 요약 전 확인 팝업, 업로드 기록 초기화 기능을 제공합니다.

## 2. 기술 스택 (Tech Stack)

-   **언어:** Python 3
-   **음성 인식:** `openai-whisper`
-   **LLM (텍스트 교정/요약):** `Ollama` (gemma3:4b, gemma3:12b-it-qat, gpt-oss:20b 등)
-   **핵심 의존성:**
    -   `openai-whisper`: Python 라이브러리
    -   `ollama`: Python 라이브러리
    -   `ffmpeg`: 시스템 프로그램 (오디오 처리 및 m4a→wav 변환)
    -   `Ollama`: 시스템 서비스 (로컬 LLM 구동을 위해 필요)

## 3. 디렉토리 구조 (Directory Structure)

```
RecordRoute/
├── run.bat                # Windows 웹 서버 실행 스크립트
├── run.command            # macOS/Linux 웹 서버 실행 스크립트
├── server.py              # 업로드 처리 및 워크플로우 실행 서버
├── frontend/
│   └── upload.html        # 웹 업로드 및 작업 관리 UI
├── sttEngine/
│   ├── requirements.txt      # Python 의존성 목록
│   ├── setup.bat             # Windows 설치 스크립트
│   ├── run_workflow.py       # 메인 워크플로우 오케스트레이션 스크립트
│   └── workflow/
│       ├── transcribe.py     # 1단계: 음성 변환 로직
│       ├── correct.py        # 2단계: 텍스트 교정 로직
│       └── summarize.py      # 3단계: 텍스트 요약 로직
├── CLAUDE.md              # Claude AI 전용 프로젝트 가이드
└── GEMINI.md              # Gemini AI 에이전트 및 개발자 가이드
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
2. 브라우저에서 `http://localhost:8080` 에 접속하여 파일 업로드, 단계 선택(STT, 교정, 요약), 작업 큐·업로드 기록 관리, 결과 오버레이 뷰어, 요약 전 확인 팝업, 업로드 기록 초기화 기능을 사용합니다.

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

