# AGENTS.md - RecordRoute AI 에이전트 참조 문서

## 프로젝트 메타데이터

- **타입**: STT → 요약 파이프라인 (PDF/오디오/비디오 지원)
- **기술스택**: Python, OpenAI Whisper, Ollama, FFmpeg, pypdf
- **플랫폼**: Windows/macOS/Linux 크로스플랫폼
- **실행환경**: 웹서버(HTTP/WebSocket) + CLI + 백그라운드 작업큐

## 1. 프로젝트 개요 (Project Overview)

이 프로젝트는 오디오/비디오/PDF 파일로부터 텍스트를 추출하고, 이를 교정 및 요약하는 자동화된 워크플로우를 제공합니다. STT(Speech-to-Text) 엔진을 중심으로 구성되어 있으며, 다음과 같은 3단계 파이프라인을 통해 작동합니다.

1. **음성 → 텍스트 변환 (Transcribe):** `openai-whisper`를 사용하여 미디어 파일에서 텍스트를 추출합니다. PDF 파일의 경우 텍스트를 직접 추출합니다.
2. **텍스트 교정 (Correct):** `Ollama`를 통해 추출된 텍스트의 오탈자, 문법 등을 교정합니다. (현재는 워크플로우에서 비활성화)
3. **텍스트 요약 (Summarize):** 교정된 텍스트를 `Ollama`를 사용해 구조화된 형식으로 요약합니다.

간단한 웹 업로드 페이지를 통해 이러한 작업을 선택적으로 실행할 수 있으며, 작업 큐와 업로드 기록 관리, 결과 오버레이 뷰어, 요약 전 확인 팝업, 업로드 기록 초기화 기능을 제공합니다.
추가로 문서 임베딩과 벡터 검색, 한 줄 요약, 실시간 작업 진행 상황 업데이트(WebSocket), 작업 취소, 결과물 수정 및 영구 삭제 등 다양한 관리 기능을 지원합니다.

### 핵심 처리 플로우
```
(오디오/PDF)파일 → [transcribe.py | PDF추출] → [summarize.py] → [embedding_pipeline.py]
```

## 2. 기술 스택 (Tech Stack)

- **언어:** Python 3
- **음성 인식:** `openai-whisper`
- **LLM (텍스트 교정/요약):** `Ollama`
- **웹 프레임워크:** `http.server`, `websockets`
- **핵심 의존성:**
  - `openai-whisper`: Python 라이브러리
  - `ollama`: Python 라이브러리
  - `ffmpeg`: 시스템 프로그램 (오디오 처리)
  - `Ollama`: 시스템 서비스 (로컬 LLM 구동을 위해 필요)
  - `pypdf`: PDF 텍스트 추출
  - `sentence-transformers`: 벡터 임베딩
  - `websockets`: 실시간 통신

## 3. 디렉토리 구조 (Directory Structure)

```
RecordRoute/
├── README.md              # 프로젝트 소개 및 설치 가이드
├── TODO.md               # 기능 구현 계획
├── LICENSE                # 라이선스 정보
├── AGENTS.md             # AI 에이전트 통합 가이드 (본 문서)
├── RecordRouteAPI.spec   # PyInstaller 빌드 스펙
├── electron/             # Electron 애플리케이션
│   ├── main.js           # Electron 메인 프로세스
│   └── preload.js        # Electron preload 스크립트 (보안)
├── scripts/              # 빌드 및 실행 스크립트
│   ├── build-backend.sh  # Python 백엔드 빌드 스크립트 (Unix)
│   ├── build-backend.bat # Python 백엔드 빌드 스크립트 (Windows)
│   ├── build-all.sh      # 전체 빌드 스크립트
│   ├── run.command       # macOS/Linux 웹 서버 실행 스크립트
│   ├── start.bat         # Windows 웹 서버 실행 스크립트
│   └── start.vbs         # Windows 숨김 실행 스크립트
├── frontend/
│   ├── upload.html        # 웹 업로드 및 작업 관리 UI
│   ├── upload.js          # 프론트엔드 로직
│   └── upload.css         # 프론트엔드 스타일
└── sttEngine/
    ├── config.py            # 환경변수 기반 설정 관리
    ├── embedding_pipeline.py  # 문서 임베딩 및 벡터 생성
    ├── keyword_frequency.py   # 키워드 빈도 분석 유틸리티
    ├── logger.py              # 로깅 설정 유틸리티
    ├── ollama_utils.py      # Ollama 서버 및 모델 관리 유틸리티
    ├── one_line_summary.py    # 한 줄 요약 유틸리티
    ├── requirements.txt       # Python 의존성 목록
    ├── run_workflow.py        # 메인 워크플로우 오케스트레이션 스크립트
    ├── run.bat                # Windows용 워크플로우 실행 스크립트
    ├── search_cache.py        # 검색 결과 캐시 관리
    ├── server.py              # 업로드 처리 및 워크플로우 실행 서버
    ├── setup.bat              # Windows용 의존성 설치 스크립트
    ├── vector_search.py       # 벡터 검색 기능
    └── workflow/
        ├── transcribe.py     # 1단계: 음성 변환 로직
        ├── correct.py        # 2단계: 텍스트 교정 로직
        └── summarize.py      # 3단계: 텍스트 요약 로직
```

## 4. 코어 시스템 구조

### 주요 타겟 파일들

#### sttEngine/server.py
**기능**: HTTP/WebSocket 서버, 파일 업로드, 작업 큐 관리, 실시간 진행상황 브로드캐스팅, API 엔드포인트 라우팅

**핵심메서드**:
- `do_GET()` / `do_POST()`: API 라우팅 정의
- `run_workflow()`: 백그라운드 워크플로우 실행
- `start_websocket_server()`: 실시간 통신을 위한 WebSocket 서버 구동
- `cancel_task()`: 실행 중인 작업 취소
- `delete_records()`: 기록 및 관련 파일 영구 삭제
- `update_stt_text()`: STT 결과물 수정
- `ThreadingHTTPServer`: 동시요청처리

#### sttEngine/workflow/transcribe.py
**기능**: OpenAI Whisper 음성인식

**주요로직**:
- GPU/MPS 최적화 (Apple Silicon 우선)
- 다양한 오디오 포맷 지원 (FFmpeg 기반)
- 실시간 진행 상황 콜백 지원

#### sttEngine/workflow/summarize.py
**기능**: 구조화된 회의록 요약생성

**핵심패턴**:
- Map-Reduce 방식으로 대용량 텍스트 처리
- 실시간 진행 상황 콜백 지원

#### sttEngine/config.py
**기능**: 환경설정 중앙집중관리

**핵심변수**:
- `PLATFORM_TYPE`: Windows/Unix 자동감지
- `DB_BASE_PATH`: 데이터베이스 및 출력 파일 저장 경로
- `DEFAULT_MODELS`: 플랫폼별 기본 모델
- `get_model_for_task()`: 작업 유형별 모델 조회

## 5. 설치 및 설정 (Setup)

프로젝트를 처음 사용하거나 의존성을 업데이트할 때 사용합니다.

1. `sttEngine` 디렉토리로 이동합니다.
2. `setup.bat` 스크립트를 실행합니다.

이 스크립트는 다음 작업을 자동으로 수행합니다.
- `venv`라는 이름의 Python 가상환경 생성
- `requirements.txt`에 명시된 Python 라이브러리 설치
- 시스템에 `ffmpeg`과 `ollama`가 설치되어 있는지 확인하고, 없을 경우 설치 안내 메시지 출력

**AI 에이전트 명령어:**
```bash
# Windows
cd sttEngine && setup.bat

# Unix/macOS/Linux
cd sttEngine && python3 -m venv venv && source venv/bin/activate && pip install -r requirements.txt
```

## 6. 실행 방법 (How to Run)

루트 디렉토리에서 제공하는 실행 스크립트로 웹 서버를 시작합니다.

1. 의존성 설치 후 실행 스크립트를 호출합니다.
2. 브라우저에서 `http://localhost:8080` 에 접속하여 파일을 업로드하고 작업을 선택합니다.
3. 작업은 백그라운드에서 비동기적으로 처리되며, UI는 WebSocket을 통해 실시간으로 진행 상태를 업데이트 받습니다.

**AI 에이전트 명령어:**

**Windows:**
```bash
scripts\start.bat
```

**macOS/Linux:**
```bash
./scripts/run.command
```

*참고: 기존 `sttEngine/run_workflow.py`는 CLI용으로 남아 있으며 대화형 입력을 요구합니다.*

## 7. 의존성 관리

### requirements.txt 패키지
```
openai-whisper>=20231117
ollama>=0.1.0
websockets
multipart
python-dotenv
sentence-transformers
pypdf>=3.0.0
numpy
```

### 시스템 의존성
- **FFmpeg**: 오디오/비디오 파일 처리
  - **개발 환경**: 시스템 PATH에 FFmpeg 설치 필요
  - **프로덕션 빌드**: `bin/ffmpeg/` 디렉토리에 플랫폼별 바이너리 배치
  - **환경변수**: `FFMPEG_PATH`로 경로 지정 가능 (fallback: 'ffmpeg')
  - **자동 번들링**: Electron 빌드 시 플랫폼에 맞는 FFmpeg 자동 포함
- **Ollama**: 로컬 LLM 서비스, 백그라운드 실행 필수

### 환경변수 (.env)
```
HUGGINGFACE_TOKEN=token_here
SUMMARY_MODEL_WINDOWS=gemma3:4b
SUMMARY_MODEL_UNIX=gpt-oss:20b
CORRECT_MODEL_WINDOWS=gemma3:4b
CORRECT_MODEL_UNIX=gemma3:12b-it-qat
EMBEDDING_MODEL=bge-m3:latest
```

### 새로운 Python 라이브러리 추가

1. `sttEngine/requirements.txt` 파일에 라이브러리(예: `new-library==1.0.0`)를 추가합니다.
2. 설치 스크립트를 다시 실행하여 라이브러리를 설치합니다.

**예시 (AI 에이전트):**
```bash
# 1. requirements.txt에 라이브러리 추가
# 2. Windows 설치 스크립트 실행
cd sttEngine && setup.bat

# Unix/macOS/Linux에서는 직접 pip 설치
./venv/bin/pip install new-library==1.0.0
```

## 8. API 엔드포인트 스펙

### POST /upload
- **기능**: 오디오/비디오/PDF 파일 업로드
- **입력**: multipart/form-data
- **출력**: 업로드된 파일 정보 및 기록 ID (`record_id`)

### POST /process
- **기능**: 워크플로우 실행
- **입력**: `{"file_path": "...", "steps": ["stt", "embedding", "summary"], "record_id": "...", "task_id": "..."}`
- **출력**: `{"stt": "/download/uuid", "summary": "/download/uuid"}`

### GET /history
- **기능**: 활성화된(삭제되지 않은) 업로드 기록 목록 반환
- **출력**: `[{"id": "...", "filename": "...", ...}]`

### GET /download/<file_uuid>
- **기능**: UUID로 식별된 결과 파일을 다운로드

### POST /cancel
- **기능**: 실행 중인 작업을 ID를 통해 취소
- **입력**: `{"task_id": "..."}`

### GET /search?q=<query>
- **기능**: 키워드 및 벡터 검색
- **입력**: 쿼리 문자열 `q`, 날짜 필터 `start`, `end`
- **출력**: `{ "keywordMatches": [...], "similarDocuments": [...] }`

### POST /similar
- **기능**: 특정 문서와 유사한 문서를 벡터 검색
- **입력**: `{"file_identifier": "...", "refresh": false}`
- **출력**: 유사 문서 목록

### POST /update_stt_text
- **기능**: STT 결과 텍스트의 내용을 수정
- **입력**: `{"file_identifier": "...", "content": "새 텍스트"}`

### POST /delete_records
- **기능**: 하나 이상의 기록과 관련 파일들을 영구적으로 삭제
- **입력**: `{"record_ids": ["..."]}`

### POST /reset_summary_embedding
- **기능**: 특정 기록의 요약 및 임베딩 결과물을 초기화
- **입력**: `{"record_id": "..."}`

### GET /models
- **기능**: 사용 가능한 Ollama 모델 목록 반환
- **출력**: `{"models": ["model1", ...], "default": {...}}`

### GET /ws (WebSocket)
- **경로**: `ws://localhost:8765`
- **기능**: 클라이언트에 실시간 작업 진행 상황 (`{"task_id": "...", "message": "..."}`) 전송

## 9. 백그라운드 작업큐 구조

### 작업생명주기
```
업로드 → 작업큐등록(task_id) → 백그라운드실행 → 진행상태 WebSocket전송 → 완료
```

### ThreadingHTTPServer & WebSocket 패턴
- **동시요청처리**: 업로드와 작업실행 병렬처리
- **작업상태추적**: `task_id` 기반 진행상황 모니터링
- **실시간 UI 업데이트**: WebSocket을 통해 프론트엔드로 진행 메시지를 푸시하여 응답성 보장

### 비동기 작업 흐름 이해
- 사용자가 웹 UI에서 "처리 시작"을 누르면, `server.py`는 작업을 즉시 백그라운드로 넘기고 `task_id`를 포함한 응답을 보냅니다.
- `upload.js`는 `task_id`를 사용하여 WebSocket을 통해 해당 작업의 진행 상황을 실시간으로 수신하고 UI를 업데이트합니다.
- 이 과정에서 문제가 발생하면, 서버 로그(`db/log` 디렉토리)와 브라우저 콘솔 로그를 함께 분석하여 원인을 찾아야 합니다.

## 10. 벡터검색 시스템

### embedding_pipeline.py
- **sentence-transformers/Ollama**: 다국어 임베딩 모델 사용
- **numpy 기반 인덱스**: 벡터 저장 및 유사도 계산
- **증분 색인**: 기존 색인에 새로운 문서만 추가

### vector_search.py
- **코사인유사도**: 의미론적 검색
- **날짜 필터링**: 검색 기간 제한 기능
- **검색 캐싱**: `search_cache.py`를 통해 반복적인 검색 쿼리 결과 재사용

## 11. AI 에이전트 활용 가이드

### 워크플로우 스크립트 수정

#### 요약 프롬프트 변경
`sttEngine/workflow/summarize.py` 파일의 `BASE_PROMPT` 변수를 수정합니다.

**예시:**
1. 파일을 읽어 현재 프롬프트를 확인합니다.
2. 원하는 내용으로 `BASE_PROMPT` 변수를 수정합니다.

#### Whisper 모델 변경
`sttEngine/workflow/transcribe.py`의 `model_identifier` 인자 기본값을 변경합니다.

### 웹 UI 및 비동기 작업 디버깅

웹 UI 관련 문제나 작업 처리 중 멈춤 현상 발생 시 다음을 확인합니다.

1. **서버 로직 확인 (`sttEngine/server.py`):**
   - `/process`: 작업을 시작하는 API 엔드포인트. `run_workflow` 함수를 백그라운드에서 실행하는지 확인합니다.
   - `/progress/<task_id>`: (이제 WebSocket으로 대체됨) 작업 진행 상태를 반환하는 엔드포인트.
   - `/history`: 작업 완료 기록을 반환하는 엔드포인트.
   - **신규 기능**: `/cancel`, `/delete_records`, `/update_stt_text` 등의 API가 정상적으로 동작하는지 확인합니다.

2. **프론트엔드 로직 확인 (`frontend/upload.js`):**
   - `processFile()`: `/process` API를 호출하고 서버로부터 `task_id`를 받는지 확인합니다.
   - **WebSocket 연결**: `ws://localhost:8765`로 WebSocket 연결을 수립하고, 서버로부터 오는 진행 메시지를 처리하는 로직을 확인합니다.
   - 브라우저의 개발자 도구(F12) 콘솔에서 자바스크립트 오류 및 WebSocket 통신 내용을 확인하는 것이 매우 중요합니다.

## 12. 디버깅 전략

### 단계별 독립실행

각 워크플로우 단계를 독립적으로 실행하여 문제를 격리합니다.

```bash
# STT 실행
python sttEngine/workflow/transcribe.py [파일경로]

# 요약 실행
python sttEngine/workflow/summarize.py [입력.md]
```

### 로그분석 포인트

- **`db/log/*.log`**: `server.py` 및 하위 모듈의 모든 stdout/stderr 출력이 기록됨.
- **transcribe.py**: GPU/MPS 초기화, FFmpeg 변환 상태, Whisper 모델 로딩
- **summarize.py**: Ollama 연결 상태, Map-Reduce 진행 과정
- **server.py**: HTTP 요청/응답, 작업 큐 상태, WebSocket 메시지

### 환경검증 체크리스트

- Python 가상환경 활성화 상태 (`venv\Scripts\activate`)
- `requirements.txt` 설치완료
- FFmpeg PATH 환경변수 설정
- Ollama 서비스 실행상태 (`ollama serve`)
- Ollama 모델 다운로드 완료 (`ollama list`)

## 13. 확장개발 가이드

### 새로운 워크플로우 단계 추가

1. `workflow/` 디렉토리에 모듈 생성
2. `server.py`의 `run_workflow()` 함수에 로직 추가
3. `frontend/upload.js`에서 해당 `step`을 서버로 전송하도록 UI 옵션 추가

### 새로운 LLM 모델 지원

1. `config.py`에 모델 설정 추가 (필요시)
2. `ollama_utils.py`에서 모델 가용성 확인
3. `frontend/upload.html` 또는 `upload.js`에서 모델 선택 UI 추가

### API 엔드포인트 확장

1. `server.py`의 `UploadHandler` 클래스에 `do_GET` 또는 `do_POST` 메서드 확장
2. URL 라우팅 패턴 추가
3. 요청/응답 JSON 스키마 정의
4. `frontend/upload.js`에서 해당 API를 호출하는 로직 추가

## 14. 프로젝트 참조 포인트

### Claude Code / MCP 에이전트
- 기술적 세부사항과 시스템 구조에 집중
- API 엔드포인트와 데이터 흐름 이해
- 디버깅과 확장 개발에 활용

### Gemini 에이전트
- 프로젝트 개요와 전반적인 구조 파악
- 설치, 실행, 의존성 관리
- UI 디버깅 및 비동기 작업 처리

### 공통 활용
- 코드 수정 시 해당 파일의 역할과 영향 범위 파악
- 새로운 기능 추가 시 관련 모듈 간 연동 방식 확인
- 문제 발생 시 로그 및 환경 검증 체크리스트 활용
