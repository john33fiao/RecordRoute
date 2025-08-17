# CLAUDE.md - RecordRoute 프로젝트 가이드 (Claude Code 최적화)

## 프로젝트 개요
RecordRoute는 음성 파일을 회의록으로 변환하는 통합 워크플로우 시스템입니다. STT(Speech-to-Text) → 텍스트 교정 → 요약의 3단계 파이프라인을 제공하며, 웹 기반 인터페이스와 CLI 실행을 모두 지원합니다.

## 아키텍처 구조

### 디렉토리 구조
```
RecordRoute/
├── README.md              # 프로젝트 소개 및 설치 가이드
├── TODO.md               # 기능 구현 계획 및 우선순위
├── LICENSE               # MIT 라이선스
├── CLAUDE.md             # Claude AI 전용 프로젝트 가이드 (현재 파일)
├── GEMINI.md             # Gemini AI 전용 프로젝트 가이드
├── run.bat               # Windows 웹 서버 실행 스크립트
├── run.command           # macOS/Linux 웹 서버 실행 스크립트
├── .env                  # 환경변수 설정 (HuggingFace 토큰 등)
├── file_registry.json    # 파일 메타데이터 레지스트리
├── upload_history.json   # 업로드 이력 관리
├── frontend/             # 웹 인터페이스
│   └── upload.html       # 파일 업로드 및 작업 관리 UI
├── uploads/              # 업로드된 오디오 파일 저장소
├── whisper_output/       # STT 결과 저장소
├── vector_store/         # 벡터 임베딩 저장소
└── sttEngine/            # 핵심 STT 엔진 및 서버 모듈
    ├── requirements.txt     # Python 의존성 정의
    ├── setup.bat           # Windows 환경 설정 스크립트
    ├── config.py           # 환경변수 기반 설정 관리
    ├── server.py           # 웹 서버 및 API 엔드포인트
    ├── run_workflow.py     # CLI 워크플로우 통합 실행기
    ├── embedding_pipeline.py  # 문서 임베딩 및 벡터 생성
    ├── vector_search.py    # 벡터 검색 엔진
    ├── one_line_summary.py # 한 줄 요약 유틸리티
    ├── ollama_utils.py     # Ollama LLM 연동 유틸리티
    └── workflow/           # 핵심 처리 모듈들
        ├── transcribe.py   # 음성→텍스트 변환 (OpenAI Whisper)
        ├── correct.py      # 텍스트 교정 (Ollama LLM)
        └── summarize.py    # 텍스트 요약 (Ollama LLM)
```

## 핵심 기능 모듈

### 1. sttEngine/workflow/transcribe.py - 음성 인식 엔진
**기능**: OpenAI Whisper를 사용한 음성→텍스트 변환

**주요 특징**:
- **GPU/MPS 최적화**: Apple Silicon MPS 우선 사용, CUDA/CPU fallback 지원
- **자동 포맷 변환**: M4A → WAV 자동 변환 (FFmpeg 기반)
- **Whisper 모델 캐시**: 플랫폼별 캐시 경로 자동 감지
  - Windows: `%USERPROFILE%\.cache\whisper\`
  - macOS/Linux: `~/.cache/whisper/`
- **모델 파일명 패턴**: `whisper-turbo.pt`, `large-v3-turbo.pt`, `turbo.pt` 지원
- **지원 포맷**: `.flac`, `.m4a`, `.mp3`, `.mp4`, `.mpeg`, `.mpga`, `.oga`, `.ogg`, `.wav`, `.webm`
- **병렬 처리**: 멀티코어 지원 (단일 GPU 환경에서는 비권장)
- **세그먼트 처리**: 병합, 필러 단어 필터링, 구두점 정규화
- **원자적 저장**: 파일 무결성 보장

**CLI 사용 예시**:
```bash
python sttEngine/workflow/transcribe.py /path/to/audio/folder \
  --model_size large-v3-turbo \
  --language ko \
  --filter_fillers \
  --normalize_punct \
  --workers 1
```

### 2. sttEngine/workflow/correct.py - 텍스트 교정
**기능**: Ollama LLM을 사용한 텍스트 교정 및 정제

**주요 특징**:
- **플랫폼별 기본 모델**:
  - Windows: `gemma3:4b`
  - macOS/Linux: `gemma3:12b-it-qat`
- **청킹 처리**: 대용량 텍스트 분할 처리
- **맞춤법 교정**: 띄어쓰기, 문법, 구두점 정리
- **재시도 메커니즘**: Ollama 연결 실패 시 자동 재시도

### 3. sttEngine/workflow/summarize.py - 텍스트 요약
**기능**: 구조화된 회의록 요약 생성

**플랫폼별 기본 모델**:
- Windows: `gemma3:4b`
- macOS/Linux: `gpt-oss:20b`

**요약 구조** (고정 6개 섹션):
1. **주요 주제**: 회의의 핵심 주제 식별
2. **핵심 내용**: 중요한 논의 사항 요약
3. **결정 사항**: 확정된 결론 및 합의사항
4. **실행 항목**: 후속 조치 및 할 일 목록
5. **리스크/이슈**: 우려사항 및 해결 필요 문제
6. **차기 일정**: 예정된 미팅 및 마일스톤

**고급 기능**:
- **Map-Reduce 요약**: 대용량 문서 청크 단위 처리 후 통합
- **온도 조절**: 생성 일관성 제어 (기본: 0.2)
- **PDF 지원**: PyPDF를 통한 PDF 문서 처리

### 4. sttEngine/server.py - 웹 서버 및 API
**기능**: 파일 업로드와 선택된 단계(STT, 교정, 요약)의 백그라운드 실행을 처리하는 HTTP 서버

**주요 API 엔드포인트**:
- `GET /`: 업로드 웹 인터페이스 제공
- `POST /upload`: 오디오 파일 업로드 처리
- `POST /process`: 선택된 워크플로우 단계 실행
- `GET /download/<file>`: 처리 결과 파일 다운로드
- `GET /tasks`: 실행 중인 작업 상태 조회
- `POST /cancel`: 작업 취소
- `POST /reset`: 업로드 기록 초기화
- `POST /search`: 벡터 검색 API

**주요 특징**:
- **ThreadingHTTPServer**: 동시 요청 처리 지원
- **작업 큐 관리**: 백그라운드 작업 스케줄링
- **업로드 기록 관리**: 개별 파일별 처리 이력 추적
- **결과 오버레이 뷰어**: 웹 기반 텍스트 뷰어 제공
- **요약 확인 팝업**: 리소스 집약적 작업에 대한 사용자 확인

### 5. sttEngine/embedding_pipeline.py - 벡터 임베딩
**기능**: 문서 임베딩 및 벡터 검색 기반 RAG 시스템

**주요 특징**:
- **sentence-transformers**: 다국어 임베딩 모델 지원
- **Ollama 임베딩**: 로컬 임베딩 모델 활용
- **벡터 저장소**: NumPy 기반 인덱스 관리
- **유사도 검색**: 코사인 유사도 기반 문서 검색

### 6. sttEngine/vector_search.py - 벡터 검색 엔진
**기능**: 임베딩된 문서에 대한 의미론적 검색

**검색 기능**:
- **의미론적 검색**: 키워드 매칭을 넘어선 의미 기반 검색
- **임계값 필터링**: 유사도 임계값 설정
- **결과 랭킹**: 유사도 점수 기반 정렬

## 의존성 및 환경 설정

### Python 패키지 (requirements.txt)
```txt
openai-whisper>=20231117    # 음성 인식 엔진
ollama>=0.1.0              # 로컬 LLM 추론 엔진
multipart                  # HTTP 멀티파트 파싱
python-dotenv              # 환경변수 로드
sentence-transformers      # 텍스트 임베딩 모델
pypdf>=3.0.0              # PDF 문서 처리
```

### 시스템 의존성
- **FFmpeg**: 다양한 오디오 포맷 처리 (M4A → WAV 변환)
- **Ollama**: 로컬 LLM 추론 엔진 (요약 및 교정)

### 환경변수 설정 (.env)
```env
# HuggingFace 액세스 토큰 (임베딩 모델 다운로드용)
HUGGINGFACE_TOKEN=your_token_here

# 플랫폼별 기본 모델 설정 (선택적)
SUMMARY_MODEL_WINDOWS=gemma3:4b
SUMMARY_MODEL_UNIX=gpt-oss:20b
CORRECT_MODEL_WINDOWS=gemma3:4b
CORRECT_MODEL_UNIX=gemma3:12b-it-qat
```

## 설치 및 실행

### 자동 설치 (권장)

#### Windows
```batch
# 1단계: 환경 설정 및 의존성 설치
sttEngine\setup.bat

# 2단계: 웹 서버 실행
run.bat
```

#### macOS/Linux
```bash
# 1단계: 의존성 설치
pip install -r sttEngine/requirements.txt

# 2단계: 웹 서버 실행 (.env 파일에서 환경변수 자동 로드)
chmod +x run.command
./run.command
```

### CLI 워크플로우 실행
```bash
# 통합 워크플로우 실행기
python sttEngine/run_workflow.py

# 개별 단계 실행
python sttEngine/workflow/transcribe.py [audio_folder]
python sttEngine/workflow/correct.py [input.md]
python sttEngine/workflow/summarize.py [input.md]
```

## 플랫폼별 최적화

### Windows 설정
- **기본 요약 모델**: `gemma3:4b` (경량화)
- **기본 교정 모델**: `gemma3:4b`
- **Whisper 캐시**: `%USERPROFILE%\.cache\whisper\`
- **Python 실행파일**: 자동 감지 (python.exe)

### macOS/Linux 설정
- **기본 요약 모델**: `gpt-oss:20b` (고성능)
- **기본 교정 모델**: `gemma3:12b-it-qat`
- **Whisper 캐시**: `~/.cache/whisper/`
- **Python 실행파일**: `venv/bin/python` (가상환경 권장)
- **Apple Silicon MPS**: GPU 가속 우선 사용

## 파일 처리 플로우

### 전체 워크플로우
```
오디오 파일 (.m4a, .mp3, .wav 등)
    ↓ (FFmpeg 자동변환)
Whisper STT 처리
    ↓ (.md 파일 생성)
텍스트 교정 (Ollama)
    ↓ (.corrected.md 파일 생성)
텍스트 요약 (Ollama)
    ↓ (.summary.md 파일 생성)
벡터 임베딩 (선택적)
    ↓ (vector_store/ 저장)
검색 가능한 지식베이스 구축
```

### 웹 인터페이스 플로우
```
파일 업로드 → 단계 선택 → 작업 큐 추가 → 백그라운드 처리 → 결과 다운로드/뷰어
```

## 주요 설정 옵션

### transcribe.py 옵션
```bash
--model_size large-v3-turbo  # Whisper 모델 크기
--language ko               # 언어 힌트 (한국어)
--filter_fillers           # 필러 단어 제거 ("음", "어" 등)
--normalize_punct          # 연속 마침표 정규화 ("..." → ".")
--workers 1               # 병렬 처리 수 (GPU 메모리 고려)
--initial_prompt "..."    # 도메인 특화 용어 힌트
--min_seg_length 2       # 세그먼트 최소 길이 (초)
```

### summarize.py 옵션
```bash
--model gpt-oss:20b       # Ollama 모델 지정
--temperature 0.2         # 생성 온도 (0.0-1.0)
--chunk_size 8000        # 청크 크기 (토큰 단위)
```

### correct.py 옵션
```bash
--model gemma3:4b        # 교정용 모델
--temperature 0.1        # 낮은 온도로 일관성 확보
```

## API 사용 예시

### 파일 업로드
```javascript
const formData = new FormData();
formData.append('file', audioFile);

fetch('/upload', {
    method: 'POST',
    body: formData
})
.then(response => response.json())
.then(data => console.log(data));
```

### 워크플로우 실행
```javascript
fetch('/process', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
        filename: 'audio.m4a',
        steps: ['transcribe', 'correct', 'summarize']
    })
})
.then(response => response.json())
.then(data => console.log(data));
```

### 벡터 검색
```javascript
fetch('/search', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
        query: '프로젝트 일정',
        limit: 5,
        threshold: 0.7
    })
})
.then(response => response.json())
.then(results => console.log(results));
```

## 에러 처리 및 로깅

### 주요 에러 패턴
- **Whisper 모델 로드 실패**: 캐시 경로 및 모델 파일 확인
- **Ollama 연결 오류**: 서비스 실행 상태 및 모델 다운로드 확인
- **FFmpeg 오류**: PATH 환경변수 설정 확인
- **인코딩 문제**: UTF-8, CP949, EUC-KR 순차 시도
- **MPS 오류**: Apple Silicon GPU 실패 시 CPU 자동 전환

### 로깅 전략
- **파일별 개별 로깅**: 전체 작업 중단 방지
- **상세 스택 트레이스**: 디버깅 정보 제공
- **진행 상황 추적**: 단계별 처리 상태 모니터링

## 확장 계획 및 개발 우선순위

### 단기 목표 (TODO.md 기반)
1. **임베딩 및 RAG 시스템 완성**: 질의응답 기능 구현
2. **검색 및 탐색 강화**: 날짜/시간 필터링, 다중 키워드 검색
3. **UI/UX 개선**: 드래그 앤 드롭, 실시간 진행률 표시

### 중기 목표
1. **LLM API 확장**: OpenAI, Anthropic API 연동
2. **다국어 지원**: 영어, 일본어 등 추가 언어 처리
3. **화자 분리**: Diarization 기능 추가

### 장기 목표
1. **클라우드 배포**: Docker 컨테이너화, 스케일링
2. **협업 기능**: 사용자 관리, 권한 제어, 공유 기능
3. **고급 분석**: 감정 분석, 키워드 빈도 분석

## 성능 최적화 가이드

### 하드웨어 최적화
1. **GPU 메모리 관리**: 단일 GPU 환경에서 `--workers 1` 사용
2. **Apple Silicon**: MPS 가속 활용으로 처리 속도 향상
3. **SSD 스토리지**: 대용량 오디오 파일 I/O 성능 향상

### 소프트웨어 최적화
1. **모델 캐싱**: Whisper 모델 로컬 캐시 활용
2. **청킹 전략**: 메모리 효율적인 대용량 파일 처리
3. **백그라운드 처리**: 웹 UI 응답성 확보

## 트러블슈팅 체크리스트

### 설치 관련
- [ ] Python 3.8+ 설치 확인
- [ ] pip 최신 버전 업데이트
- [ ] requirements.txt 의존성 설치 완료
- [ ] FFmpeg PATH 환경변수 설정
- [ ] Ollama 서비스 실행 상태

### 모델 관련
- [ ] Whisper 모델 캐시 디렉토리 존재
- [ ] Ollama 모델 다운로드 완료 (`ollama list`로 확인)
- [ ] HuggingFace 토큰 .env 파일 설정 (임베딩용)

### 실행 관련
- [ ] 포트 8080 사용 가능 여부
- [ ] uploads/ 디렉토리 쓰기 권한
- [ ] 오디오 파일 포맷 지원 확인
- [ ] 시스템 메모리 충분성 (4GB 이상 권장)

## Claude Code 활용 팁

### 코드 분석 시 집중 영역
1. **sttEngine/server.py**: 웹 서버 로직 및 API 엔드포인트
2. **sttEngine/workflow/**: 핵심 처리 알고리즘
3. **sttEngine/config.py**: 환경 설정 및 플랫폼 감지
4. **frontend/upload.html**: 웹 UI 및 JavaScript 로직

### 디버깅 우선순위
1. **모듈 import 오류**: PYTHONPATH 및 sys.path 확인
2. **Ollama 연결 실패**: 서비스 상태 및 모델 가용성
3. **파일 인코딩 문제**: 다중 인코딩 fallback 로직
4. **메모리 부족**: 청킹 크기 및 병렬 처리 수 조정

### 확장 개발 가이드
1. **새로운 워크플로우 단계**: workflow/ 디렉토리에 모듈 추가
2. **새로운 LLM 모델**: ollama_utils.py에 모델 검증 로직 추가
3. **새로운 API 엔드포인트**: server.py의 do_POST 메소드 확장
4. **새로운 임베딩 모델**: embedding_pipeline.py 모델 설정 수정

이 가이드는 Claude Code를 통한 효율적인 개발과 디버깅을 지원하며, 프로젝트의 모든 핵심 구성 요소에 대한 포괄적인 이해를 제공합니다.