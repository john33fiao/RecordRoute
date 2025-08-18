# CLAUDE.md - RecordRoute MCP/Claude Code 참조 문서

## 프로젝트 메타데이터

- **타입**: STT → 교정 → 요약 파이프라인
- **기술스택**: Python, OpenAI Whisper, Ollama, FFmpeg
- **플랫폼**: Windows/macOS/Linux 크로스플랫폼
- **실행환경**: 웹서버(HTTP) + CLI + 백그라운드 작업큐

## 코어 시스템 구조

### 핵심 처리 플로우
```
오디오파일 → transcribe.py → correct.py → summarize.py → 벡터임베딩
```

### 디렉토리 맵핑
```
RecordRoute/
├── sttEngine/server.py           # HTTP서버, API엔드포인트, 작업큐
├── sttEngine/config.py           # 환경설정, 플랫폼감지
├── sttEngine/workflow/
│   ├── transcribe.py             # Whisper STT 엔진
│   ├── correct.py                # Ollama 텍스트교정
│   └── summarize.py              # Ollama 구조화요약
├── sttEngine/embedding_pipeline.py  # 벡터임베딩
├── sttEngine/vector_search.py       # 벡터검색
├── sttEngine/ollama_utils.py        # Ollama 연동유틸
├── frontend/upload.html             # 웹UI
└── sttEngine/requirements.txt       # 의존성정의
```

## MCP 작업 타겟 파일들

### 1. sttEngine/server.py
**기능**: HTTP서버, 파일업로드, 작업큐관리, API엔드포인트
**핵심메서드**:
- `do_POST()`: API라우팅 (/upload, /process, /search 등)
- `run_workflow()`: 백그라운드 워크플로우 실행
- `ThreadingHTTPServer`: 동시요청처리

### 2. sttEngine/workflow/transcribe.py  
**기능**: OpenAI Whisper 음성인식
**주요로직**:
- GPU/MPS 최적화 (Apple Silicon 우선)
- M4A→WAV 자동변환 (FFmpeg)
- 세그먼트 병합, 필러단어 필터링
- 플랫폼별 캐시경로 자동감지

### 3. sttEngine/workflow/correct.py
**기능**: Ollama LLM 텍스트교정
**핵심패턴**:
- 청킹처리 (대용량텍스트)
- 재시도메커니즘 (Ollama연결실패)
- 플랫폼별 기본모델 (Windows: gemma3:4b, Unix: gemma3:12b-it-qat)

### 4. sttEngine/workflow/summarize.py
**기능**: 구조화된 회의록 요약생성
**고정구조** (6섹션):
1. 주요 주제
2. 핵심 내용  
3. 결정 사항
4. 실행 항목
5. 리스크/이슈
6. 차기 일정

### 5. sttEngine/config.py
**기능**: 환경설정 중앙집중관리
**핵심변수**:
- `PLATFORM_TYPE`: Windows/Unix 자동감지
- `PYTHON_PATH`: 플랫폼별 Python실행경로
- `DEFAULT_MODELS`: 플랫폼별 최적모델
- `CACHE_PATHS`: Whisper 캐시경로

## 의존성 관리

### requirements.txt 패키지
```
openai-whisper>=20231117
ollama>=0.1.0
multipart
python-dotenv
sentence-transformers
pypdf>=3.0.0
```

### 시스템 의존성
- **FFmpeg**: M4A→WAV 변환, PATH환경변수 필수
- **Ollama**: 로컬LLM서비스, 백그라운드실행 필수

### 환경변수 (.env)
```
HUGGINGFACE_TOKEN=token_here
SUMMARY_MODEL_WINDOWS=gemma3:4b
SUMMARY_MODEL_UNIX=gpt-oss:20b
CORRECT_MODEL_WINDOWS=gemma3:4b
CORRECT_MODEL_UNIX=gemma3:12b-it-qat
```

## API 엔드포인트 스펙

### POST /upload
- **기능**: 오디오파일 업로드
- **입력**: multipart/form-data
- **출력**: 업로드상태 JSON

### POST /process  
- **기능**: 워크플로우 실행
- **입력**: `{"filename": "file.m4a", "steps": ["transcribe", "correct", "summarize"]}`
- **출력**: `{"task_id": "uuid", "status": "started"}`

### GET /tasks
- **기능**: 작업큐 상태조회
- **출력**: 실행중인 작업리스트

### POST /search
- **기능**: 벡터검색
- **입력**: `{"query": "검색어", "limit": 5, "threshold": 0.7}`
- **출력**: 유사문서 리스트

## 플랫폼별 최적화 패턴

### Windows 환경
```python
# config.py에서 자동감지
if platform.system() == "Windows":
    DEFAULT_SUMMARY_MODEL = "gemma3:4b"
    DEFAULT_CORRECT_MODEL = "gemma3:4b"
    PYTHON_PATH = "python.exe"
    WHISPER_CACHE = os.path.expanduser("~/.cache/whisper/")
```

### macOS/Linux 환경  
```python
else:  # Unix계열
    DEFAULT_SUMMARY_MODEL = "gpt-oss:20b"
    DEFAULT_CORRECT_MODEL = "gemma3:12b-it-qat"
    PYTHON_PATH = "venv/bin/python"
    # Apple Silicon MPS 우선사용
```

## 에러처리 패턴

### 1. 모듈import 실패
- **원인**: 의존성 미설치, PYTHONPATH 문제
- **해결**: requirements.txt 재설치, sys.path 확인

### 2. Ollama 연결실패
- **원인**: 서비스 미실행, 모델 미다운로드
- **해결**: `ollama serve` 실행상태, `ollama list` 모델확인

### 3. Whisper 모델로딩 실패
- **원인**: 캐시파일 손상, GPU메모리 부족
- **해결**: 캐시디렉토리 재생성, CPU fallback

### 4. FFmpeg 처리실패
- **원인**: FFmpeg 미설치, PATH 미설정
- **해결**: 시스템 PATH 환경변수 확인

### 5. 인코딩 문제
- **자동처리**: UTF-8 → CP949 → EUC-KR 순차시도
- **파일별 개별로깅**: 전체작업 중단방지

## 백그라운드 작업큐 구조

### 작업생명주기
```
업로드 → 작업큐등록 → 백그라운드실행 → 진행상태폴링 → 완료알림
```

### ThreadingHTTPServer 패턴
- **동시요청처리**: 업로드와 작업실행 병렬처리
- **작업상태추적**: task_id 기반 진행상황 모니터링
- **논블로킹UI**: 웹인터페이스 응답성 보장

## 벡터검색 시스템

### embedding_pipeline.py
- **sentence-transformers**: 다국어 임베딩모델
- **numpy 기반 인덱스**: 벡터저장 및 유사도계산
- **청킹전략**: 대용량문서 분할처리

### vector_search.py  
- **코사인유사도**: 의미론적 검색
- **임계값필터링**: 정확도 제어
- **결과랭킹**: 유사도점수 기반정렬

## MCP 디버깅 전략

### 1. 단계별 독립실행
```bash
python sttEngine/workflow/transcribe.py [파일경로]
python sttEngine/workflow/correct.py [입력.md]  
python sttEngine/workflow/summarize.py [입력.md]
```

### 2. 로그분석 포인트
- **transcribe.py**: GPU/MPS 초기화, FFmpeg 변환상태
- **correct.py**: Ollama 연결상태, 청킹처리
- **summarize.py**: 모델응답시간, 메모리사용량
- **server.py**: HTTP요청처리, 작업큐상태

### 3. 환경검증 체크리스트
- Python 가상환경 활성화 상태
- requirements.txt 설치완료
- FFmpeg PATH 환경변수 설정
- Ollama 서비스 실행상태  
- 모델 다운로드 완료 (`ollama list`)

## 확장개발 가이드

### 새로운 워크플로우 단계 추가
1. `workflow/` 디렉토리에 모듈생성
2. `server.py`의 `run_workflow()` 함수에 라우팅 추가
3. `frontend/upload.html`에 UI 옵션추가

### 새로운 LLM 모델 지원
1. `config.py`에 모델설정 추가
2. `ollama_utils.py`에 모델검증로직 추가
3. 플랫폼별 기본모델 업데이트

### API 엔드포인트 확장
1. `server.py`의 `do_POST()` 메서드 확장
2. URL 라우팅패턴 추가
3. 요청/응답 JSON 스키마 정의

### 벡터검색 고도화
1. `embedding_pipeline.py` 모델설정 수정
2. `vector_search.py` 검색알고리즘 개선
3. 메타데이터 기반 필터링 추가