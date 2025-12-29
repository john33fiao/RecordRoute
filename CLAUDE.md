# CLAUDE.md - RecordRoute MCP/Claude Code 참조 문서

## 프로젝트 메타데이터

- **타입**: STT → 요약 파이프라인 (PDF/오디오/비디오 지원)
- **기술스택**: Python, OpenAI Whisper, Ollama, FFmpeg, pypdf
- **플랫폼**: Windows/macOS/Linux 크로스플랫폼
- **실행환경**: 웹서버(HTTP/WebSocket) + CLI + 백그라운드 작업큐

## 코어 시스템 구조

### 핵심 처리 플로우
```
(오디오/PDF)파일 → [transcribe.py | PDF추출] → [summarize.py] → [embedding_pipeline.py]
```

### 디렉토리 맵핑
```
RecordRoute/
├── sttEngine/server.py           # HTTP/WebSocket서버, API엔드포인트, 작업큐
├── sttEngine/config.py           # 환경설정, 플랫폼감지
├── sttEngine/logger.py           # 파일 기반 로깅
├── sttEngine/search_cache.py     # 검색 결과 캐시
├── sttEngine/workflow/
│   ├── transcribe.py             # Whisper STT 엔진
│   └── summarize.py              # Ollama 구조화요약
├── sttEngine/embedding_pipeline.py  # 벡터임베딩
├── sttEngine/vector_search.py       # 벡터검색
├── sttEngine/ollama_utils.py        # Ollama 연동유틸
├── frontend/upload.html             # 웹UI
└── sttEngine/requirements.txt       # 의존성정의
```

## MCP 작업 타겟 파일들

### 1. sttEngine/server.py
**기능**: HTTP/WebSocket 서버, 파일 업로드, 작업 큐 관리, 실시간 진행상황 브로드캐스팅, API 엔드포인트 라우팅
**핵심메서드**:
- `do_GET()` / `do_POST()`: API 라우팅 정의
- `run_workflow()`: 백그라운드 워크플로우 실행
- `start_websocket_server()`: 실시간 통신을 위한 WebSocket 서버 구동
- `cancel_task()`: 실행 중인 작업 취소
- `delete_records()`: 기록 및 관련 파일 영구 삭제
- `update_stt_text()`: STT 결과물 수정
- `ThreadingHTTPServer`: 동시요청처리

### 2. sttEngine/workflow/transcribe.py  
**기능**: OpenAI Whisper 음성인식
**주요로직**:
- GPU/MPS 최적화 (Apple Silicon 우선)
- 다양한 오디오 포맷 지원 (FFmpeg 기반)
- 실시간 진행 상황 콜백 지원

### 3. sttEngine/workflow/summarize.py
**기능**: 구조화된 회의록 요약생성
**핵심패턴**:
- Map-Reduce 방식으로 대용량 텍스트 처리
- 실시간 진행 상황 콜백 지원

### 4. sttEngine/config.py
**기능**: 환경설정 중앙집중관리
**핵심변수**:
- `PLATFORM_TYPE`: Windows/Unix 자동감지
- `DB_BASE_PATH`: 데이터베이스 및 출력 파일 저장 경로
- `DEFAULT_MODELS`: 플랫폼별 기본 모델
- `get_model_for_task()`: 작업 유형별 모델 조회

## 의존성 관리

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
- **FFmpeg**: 오디오/비디오 파일 처리, PATH 환경변수 필수
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

## API 엔드포인트 스펙

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

## 백그라운드 작업큐 구조

### 작업생명주기
```
업로드 → 작업큐등록(task_id) → 백그라운드실행 → 진행상태 WebSocket전송 → 완료
```

### ThreadingHTTPServer & WebSocket 패턴
- **동시요청처리**: 업로드와 작업실행 병렬처리
- **작업상태추적**: `task_id` 기반 진행상황 모니터링
- **실시간 UI 업데이트**: WebSocket을 통해 프론트엔드로 진행 메시지를 푸시하여 응답성 보장

## 벡터검색 시스템

### embedding_pipeline.py
- **sentence-transformers/Ollama**: 다국어 임베딩 모델 사용
- **numpy 기반 인덱스**: 벡터 저장 및 유사도 계산
- **증분 색인**: 기존 색인에 새로운 문서만 추가

### vector_search.py  
- **코사인유사도**: 의미론적 검색
- **날짜 필터링**: 검색 기간 제한 기능
- **검색 캐싱**: `search_cache.py`를 통해 반복적인 검색 쿼리 결과 재사용

## MCP 디버깅 전략

### 1. 단계별 독립실행
```bash
# STT 실행
python sttEngine/workflow/transcribe.py [파일경로]
# 요약 실행
python sttEngine/workflow/summarize.py [입력.md]
```

### 2. 로그분석 포인트
- **`db/log/*.log`**: `server.py` 및 하위 모듈의 모든 stdout/stderr 출력이 기록됨.
- **transcribe.py**: GPU/MPS 초기화, FFmpeg 변환 상태, Whisper 모델 로딩
- **summarize.py**: Ollama 연결 상태, Map-Reduce 진행 과정
- **server.py**: HTTP 요청/응답, 작업 큐 상태, WebSocket 메시지

### 3. 환경검증 체크리스트
- Python 가상환경 활성화 상태 (`venv\Scripts\activate`)
- `requirements.txt` 설치완료
- FFmpeg PATH 환경변수 설정
- Ollama 서비스 실행상태 (`ollama serve`)  
- Ollama 모델 다운로드 완료 (`ollama list`)

## 확장개발 가이드

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