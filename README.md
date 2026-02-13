# RecordRoute
음성 파일을 회의록으로 변환하는 통합 워크플로우 시스템입니다. STT(Speech-to-Text), 교정, 요약 기능을 단계적으로 제공합니다.

## 주요 기능

 - **음성→텍스트 변환**: OpenAI Whisper를 사용한 고품질 음성 인식
 - **텍스트 교정**: LLM을 이용해 오탈자와 문법을 자동으로 수정
 - **구조화된 요약**: 회의록 형태의 체계적 요약 생성
 - **통합 워크플로우**: STT부터 요약까지 자동화된 처리 파이프라인
 - **웹 기반 인터페이스**: 파일 업로드와 단계별 작업 선택, 작업 큐·업로드 기록 관리, 결과 오버레이 뷰어, 기록 초기화 지원
 - **임베딩 기반 검색**: 문서를 벡터화하여 RAG 질의·유사도 검색·유사 문서 추천 지원 (bge-m3)
 - **한 줄 요약**: 텍스트 파일을 한 줄로 요약하는 유틸리티
 - **Vocabulary Manager**: 임베딩된 문서에서 키워드를 추출하여 STT 정확도 향상
 - **WebSocket 실시간 통신**: 작업 진행 상태를 실시간으로 업데이트
 - **검색 결과 캐싱**: 24시간 캐시로 검색 성능 최적화
 - **자동 로깅**: 1MB 제한 로그 파일 자동 롤오버
 - **Cloudflare Tunnel**: 안전한 외부 접근 및 Zero Trust 인증

## 디렉토리 구조

```
RecordRoute/
├── README.md                  # 프로젝트 소개 및 설치 가이드
├── TODO/                      # 기능 구현 계획 디렉토리
├── LICENSE                    # 라이선스 정보
├── CLAUDE.md                  # Claude AI 전용 프로젝트 가이드
├── GEMINI.md                  # Gemini AI 전용 프로젝트 가이드
├── .env.example               # 환경변수 템플릿
├── run.sh                     # Unix 웹 서버 실행 스크립트
├── run.bat                    # Windows 웹 서버 실행 스크립트
├── setup.sh                   # Unix 설정 스크립트
├── setup.bat                  # Windows 설정 스크립트
├── requirements.txt           # Python 의존성 목록
├── frontend/                  # 웹 인터페이스
│   ├── upload.html            # 업로드 및 작업 관리 UI
│   ├── upload.js              # 프론트엔드 로직 (WebSocket 지원)
│   └── upload.css             # 프론트엔드 스타일
└── sttEngine/                 # STT 엔진 및 서버 모듈
    ├── config.py              # 환경변수 기반 설정 관리, DB 경로 관리
    ├── logger.py              # 로깅 시스템 (자동 롤오버)
    ├── vocabulary_manager.py  # STT 정확도 향상용 어휘 관리
    ├── keyword_frequency.py   # 키워드 빈도 분석 유틸리티
    ├── search_cache.py        # 검색 결과 캐싱 (24시간)
    ├── ollama_utils.py        # Ollama 서버 확인 및 자동 실행
    ├── embedding_pipeline.py  # 문서 임베딩 및 벡터 생성 (bge-m3)
    ├── one_line_summary.py    # 한 줄 요약 유틸리티
    ├── run_workflow.py        # CLI 워크플로우 통합 실행기
    ├── server.py              # HTTP/WebSocket 서버, 업로드 처리
    ├── vector_search.py       # 벡터 검색 기능
    └── workflow/              # 핵심 처리 모듈들
        ├── transcribe.py      # 음성→텍스트 변환
        ├── correct.py         # 텍스트 교정
        └── summarize.py       # 텍스트 요약
```

## 설치 및 설정

### 1. 자동 설치 (권장)

#### Windows
```bash
# 1단계: 환경 설정
sttEngine\setup.bat

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

### 2. 수동 설치

#### Python 패키지
```bash
pip install -r sttEngine/requirements.txt
```

**포함 패키지:**
 - `openai-whisper>=20231117`: 음성 인식
 - `ollama>=0.1.0`: 로컬 LLM 추론
 - `torch`, `torchaudio`, `torchvision`: PyTorch GPU/CUDA 지원
 - `websockets>=10.0`: WebSocket 실시간 통신
 - `sentence-transformers`: 벡터 임베딩
 - `pypdf>=3.0.0`: PDF 처리
 - `python-dotenv`: 환경변수 관리
 - `multipart`: 파일 업로드 처리

#### FFmpeg 설치
다양한 오디오 형식 처리를 위해 필수:
```bash
# Windows - Chocolatey 사용 시
choco install ffmpeg

# macOS
brew install ffmpeg

# 또는 https://ffmpeg.org/download.html 에서 직접 설치
```

### 3. Ollama 설정

#### Ollama 설치
```bash
# Windows
winget install Ollama.Ollama

# macOS
brew install ollama

# 또는 https://ollama.com/download 에서 설치
```

#### 모델 다운로드
```bash
# Windows 사용자
ollama pull gemma3:4b
ollama pull bge-m3:latest

# macOS/Linux 사용자
ollama pull gemma3:12b-it-qat
ollama pull gpt-oss:20b
ollama pull bge-m3:latest
```

**모델 용도:**
- `gemma3:4b`: Windows용 요약 모델
- `gemma3:12b-it-qat`: macOS/Linux용 교정 모델
- `gpt-oss:20b`: macOS/Linux용 요약 모델
- `bge-m3:latest`: 벡터 임베딩 모델 (모든 플랫폼)


## 검색 API 계약 (하위호환 확장)

`GET /search`

기존 파라미터(`query`, `limit`, `start_date`, `end_date`, `sort_by`, `sort_order`, `min_score`, `page`, `page_size`, `include_timing`)는 그대로 유지됩니다.

추가 필터(선택):
- `file_type`: `audio,document,other` 중 콤마 구분 목록
- `status`: `completed` 또는 `pending`
- `status_task`: `stt|summary|embedding` (기본 `stt`)

정렬:
- `sort_by=similarity|date`
- `sort_order=asc|desc`

응답 확장(기존 필드 유지):
- `filters.file_type`, `filters.status`, `filters.status_task`
- `contract_version: "search-v2"`
- `keywordMatches[].snippet`, `keywordMatches[].score`
- `similarDocuments[].snippet`, `similarDocuments[].score`

캐시 정책:
- TTL 24시간 유지
- 페이지별 캐시 중복 생성을 줄이기 위해 정렬된 후보 집합을 캐시하고, 페이지네이션은 캐시 조회 후 적용
- 인덱스 파일 시그니처 및 필터 시그니처를 캐시 키에 포함해 stale-hit를 방지

## 사용법

### 웹 인터페이스 실행
```bash
# Windows
run.bat

# macOS/Linux
./run.command
```
웹 브라우저에서 <http://localhost:8080> 에 접속하여 파일을 업로드하고 STT, 요약 작업을 선택합니다. 작업 큐, 업로드 기록(개별 초기화 가능), 결과 오버레이 뷰어, 임베딩 기반 검색·유사 문서 탐색 기능을 제공합니다.

### CLI 워크플로우 실행
```bash
python sttEngine/run_workflow.py
```

### 단계별 실행

#### 1단계: 음성→텍스트 변환
```bash
python sttEngine/workflow/transcribe.py [audio_folder] --model_size large-v3-turbo --language ko --filter_fillers
```

**주요 옵션:**
 - `--model_size`: Whisper 모델 크기 (tiny, base, small, medium, large, large-v3-turbo)
 - `--language ko`: 한국어 힌트
 - `--filter_fillers`: 필러 단어 제거
 - `--normalize_punct`: 연속 마침표 정규화

#### 2단계: 텍스트 교정 (선택)
```bash
python sttEngine/workflow/correct.py input.md --model gemma3:4b --temperature 0.0
```

#### 3단계: 텍스트 요약
```bash
python sttEngine/workflow/summarize.py input.md --model gemma3:4b --temperature 0.0  # Windows
python sttEngine/workflow/summarize.py input.md --model gpt-oss:20b --temperature 0.0  # macOS/Linux
```

## 지원 오디오 포맷

- `.flac`, `.m4a`, `.mp3`, `.mp4`, `.mpeg`, `.mpga`, `.oga`, `.ogg`, `.qta`, `.wav`, `.webm`
- **M4A 자동 변환**: m4a 파일을 wav로 자동 변환하여 처리

## 지원 문서 포맷

- `.md`, `.txt`, `.text`, `.markdown`
- `.pdf` (요약 전용)

## 플랫폼별 최적화

### Windows
- 모델: `gemma3:4b` (요약용), `bge-m3:latest` (임베딩용)
- 캐시: `%USERPROFILE%\.cache\whisper\`
- Python 실행파일: 자동 감지
- GPU: CUDA 124 지원 (PyTorch)

### macOS/Linux
- 모델: 요약 `gpt-oss:20b`, 교정 `gemma3:12b-it-qat`, 임베딩 `bge-m3:latest`
- 캐시: `~/.cache/whisper/`
- Python 실행파일: `venv/bin/python` (가상환경 사용)
- 환경변수: `.env` 파일에서 자동 로드
- **Apple Silicon MPS**: GPU/MPS 우선 사용, CPU fallback 지원

## 처리 단계

### 1단계: 음성→텍스트
- OpenAI Whisper `large-v3-turbo` 모델 사용
- 세그먼트 병합 및 필러 단어 필터링
- 결과: `.md` 파일

### 2단계: 텍스트 교정
- LLM을 이용한 오탈자 및 문법 수정
- 결과: `.corrected.md` 파일

### 3단계: 텍스트 요약
구조화된 회의록 형태의 요약 생성:
1. 주요 주제
2. 핵심 내용
3. 결정 사항
4. 실행 항목
5. 리스크/이슈
6. 차기 일정

결과: `.summary.md` 파일

## 성능 최적화 팁

1. **단일 GPU 환경**: `--workers 1` 사용 권장
2. **대용량 파일**: 청킹 처리로 메모리 효율성 확보
3. **플랫폼별 모델**: 최적화된 모델 사용으로 성능 향상
4. **캐시 활용**: 모델 로딩 시간 단축

## 트러블슈팅

### 일반적인 문제
- **모델 로딩 실패**: 캐시 경로와 모델 파일 존재 여부 확인
- **Ollama 연결 오류**: Ollama 서비스 실행 상태 점검
- **FFmpeg 오류**: 시스템 PATH 환경변수에 FFmpeg 경로 추가
- **인코딩 문제**: UTF-8, CP949, EUC-KR 순으로 자동 시도
- **MPS 오류**: Apple Silicon에서 GPU 실패 시 CPU로 자동 전환
- **M4A 변환 오류**: FFmpeg 설치 및 PATH 설정 확인

## 참고사항

- 이 프로젝트는 개인적인 학습 목적으로 진행되었습니다.
- 상용 서비스에 적용하기 위해서는 추가적인 검토와 개선이 필요합니다.
- 사용 중 발생하는 문제에 대해서는 책임지지 않습니다.

### 추가 참고사항

- 본 레포지토리는 UX 기획자에 의해, LLM 도구 및 Git에 대한 학습을 목적으로 운영됩니다. 
- 대부분의 코드는 LLM(Claude > Gemini > ChatGPT 순)으로 작성되었습니다.
- 구현 예정사항은 [Todo List](/TODO//TODO.md)로 정리합니다.

## Docker 배포 (GPU 자동 활용 포함)

### 검토 결과 요약
- **CUDA(NVIDIA)**: Docker + NVIDIA Container Toolkit 환경이면 컨테이너 내부에서 GPU를 자동 감지하여 사용 가능합니다.
- **Apple Metal(MPS)**: Linux 기반 Docker 컨테이너에서는 직접 사용이 어렵습니다. macOS에서는 보통 앱을 호스트에서 직접 실행할 때 MPS가 활성화됩니다.
- **기타 GPU(예: AMD ROCm)**: 별도 ROCm 런타임/이미지가 필요하며 기본 compose 구성만으로는 보장되지 않습니다.

즉, 현재 도커라이징은 **NVIDIA CUDA 환경에서 정상 동작 가능**하도록 구성했고, 그 외 환경은 CPU fallback 또는 추가 런타임 구성이 필요합니다.

### 1) 빌드 및 실행
```bash
# 기본(내장 Ollama 포함)
docker compose up -d --build

# NVIDIA GPU 활성화
# (사전조건: host에 nvidia-container-toolkit 설치)
docker compose -f docker-compose.yml -f docker-compose.gpu.yml up -d --build
```

### 2) 접속
- 웹 UI: `http://localhost:8080`
- Ollama API: `http://localhost:11434`

### 3) 데이터 영속화
- `./DB` → 컨테이너 `/data/DB`
- `ollama_data` 볼륨 → Ollama 모델 캐시

### 4) 환경변수
- `WORKFLOW_DEVICE=auto` (기본): CUDA/MPS/CPU 자동 선택
- `OLLAMA_BASE_URL=http://ollama:11434` (compose 기본값)
- `DB_FOLDER_PATH=/data/DB`

### 5) 포함된 도커 파일
- `Dockerfile`: Python + FFmpeg + Node 빌드 환경, frontend 빌드까지 포함
- `docker/entrypoint.sh`: 컨테이너 시작 시 실행 환경 세팅 및 서버 기동
- `docker-compose.yml`: RecordRoute + Ollama 통합 실행
- `docker-compose.gpu.yml`: NVIDIA GPU 할당 오버레이
- `.dockerignore`: 이미지 빌드 최적화
