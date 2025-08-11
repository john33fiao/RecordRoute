# CLAUDE.md - RecordRoute 프로젝트 가이드

## 프로젝트 개요
RecordRoute는 음성 파일을 회의록으로 변환하는 통합 워크플로우 시스템입니다. STT(Speech-to-Text), 텍스트 교정, 요약 기능을 단계적으로 제공합니다.

## 아키텍처 구조

### 디렉토리 구조
```
RecordRoute/
├── README.md              # 프로젝트 소개 및 설치 가이드
├── TodoList.md           # 기능 구현 계획
├── LICENSE              # 라이선스 정보
├── CLAUDE.md             # Claude AI 전용 프로젝트 가이드
├── GEMINI.md             # Gemini AI 전용 프로젝트 가이드
├── run.sh               # Unix/macOS/Linux 실행 스크립트
├── venv/                # Python 가상환경
├── test/                # 테스트 오디오 파일 디렉토리
├── whisper_output/      # STT 변환 결과 저장소
└── sttEngine/           # STT 엔진 메인 모듈
    ├── requirements.txt    # Python 의존성
    ├── setup.bat          # Windows 설치 스크립트
    ├── run.bat           # Windows 실행 스크립트
    ├── run_workflow.py   # 워크플로우 통합 실행기
    └── workflow/         # 핵심 처리 모듈들
        ├── transcribe.py   # 음성→텍스트 변환
        ├── correct.py     # 텍스트 교정
        └── summarize.py   # 텍스트 요약
```

## 핵심 기능 모듈

### 1. transcribe.py - 음성 인식 엔진
**기능**: OpenAI Whisper를 사용한 음성→텍스트 변환

**주요 특징**:
- **화자 구분 기능**: pyannote.audio를 사용한 화자 구분 (기본 활성화)
- **GPU/MPS 최적화**: Apple Silicon MPS 우선 사용, CPU fallback 지원
- **M4A 자동 변환**: m4a 파일을 wav로 자동 변환하여 처리
- 플랫폼별 Whisper 캐시 경로 자동 감지 (Windows: `%USERPROFILE%\.cache\whisper\`, macOS/Linux: `~/.cache/whisper/`)
- `large-v3-turbo` 모델 우선 사용, 다양한 파일명 패턴 지원 (`whisper-turbo.pt`, `large-v3-turbo.pt`, `turbo.pt`)
- 지원 포맷: `.flac`, `.m4a`, `.mp3`, `.mp4`, `.mpeg`, `.mpga`, `.oga`, `.ogg`, `.wav`, `.webm`
- 병렬 처리 지원 (단일 GPU 환경에서는 권장하지 않음)
- 세그먼트 병합 및 필러 단어 필터링 기능
- 원자적 파일 저장으로 안정성 보장

**실행 예시**:
```bash
python transcribe.py /path/to/audio/folder --model_size large-v3-turbo --language ko --filter_fillers --normalize_punct --diarize
```

**화자 구분 기능**:
- pyannote.audio 2.1.1+ 사용
- GPU/MPS 우선 사용, CPU fallback 자동 전환
- PYANNOTE_TOKEN 환경변수 필요 (Hugging Face 토큰)

### 2. correct.py - 텍스트 교정
**기능**: Ollama를 사용한 한국어 텍스트 교정

**플랫폼별 기본 모델**:
- Windows: `gemma3:4b`
- macOS/Linux: `gpt-oss:20b`

**교정 규칙**:
- 원문 의미와 사실 보존
- 오탈자/문법/어법 수정
- 마크다운 구조 보존
- 발화자 표기와 타임스탬프 유지
- 중복어, 군말 제거
- 수치/단위/고유명사 보존

**실행 예시**:
```bash
python correct.py input.md --model gemma3:4b --temperature 0.0
```

### 3. summarize.py - 텍스트 요약
**기능**: 구조화된 회의록 요약 생성

**플랫폼별 기본 모델**:
- Windows: `gemma3:4b`
- macOS/Linux: `gpt-oss:20b`

**요약 구조** (고정 섹션):
1. 주요 주제
2. 핵심 내용
3. 결정 사항
4. 실행 항목
5. 리스크/이슈
6. 차기 일정

**청킹 처리**: 큰 문서는 청크 단위로 분할 후 통합 요약 수행

### 4. run_workflow.py - 통합 워크플로우 실행기
**기능**: 1→2→3단계 또는 선택적 단계 실행 지원

**플랫폼 호환성**:
- Windows: 현재 Python 인터프리터 자동 사용
- Unix 계열: 지정된 Python 경로 사용

**실행 모드**:
- 단계별 선택 실행 (1, 2, 3 또는 조합)
- 연속 처리 파이프라인
- 실시간 진행 상황 출력

## 의존성 및 환경 설정

### Python 패키지
```txt
openai-whisper>=20231117
ollama>=0.1.0
pyannote.audio>=2.1.1
```

### 시스템 의존성
- **FFmpeg**: 다양한 오디오 포맷 처리용
- **Ollama**: 로컬 LLM 추론 엔진

### 설치 자동화

**Windows:**
```bash
# 1단계: 환경 설정
setup.bat

# 2단계: 워크플로우 실행  
run.bat
```

**Unix/macOS/Linux:**
```bash
# 워크플로우 실행 (.env 파일에서 환경변수 자동 로드)
./run.sh
```

## 플랫폼별 설정

### Windows
- 모델: `gemma3:4b` (교정 및 요약 공용)
- 캐시: `%USERPROFILE%\.cache\whisper\`
- Python 실행파일: 자동 감지

### macOS/Linux
- 모델: `gpt-oss:20b` (교정 및 요약 공용)
- 캐시: `~/.cache/whisper/`
- Python 실행파일: `venv/bin/python` (가상환경 사용)
- 환경변수: `.env` 파일에서 자동 로드
- 화자 구분: MPS (Apple Silicon) > CPU 순으로 자동 선택

## 파일 처리 플로우

### 1단계: 음성→텍스트
```
오디오 파일 → Whisper 모델 → 세그먼트 병합 → 필터링 → .md 파일
```

### 2단계: 텍스트 교정
```
.md 파일 → Ollama 모델 → 문법/어법 수정 → .corrected.md 파일
```

### 3단계: 텍스트 요약
```
.corrected.md 파일 → Ollama 모델 → 구조화 요약 → .summary.md 파일
```

## 주요 설정 옵션

### transcribe.py
- `--model_size`: Whisper 모델 선택
- `--language`: 언어 힌트 (기본: ko)  
- `--filter_fillers`: 필러 단어 제거
- `--normalize_punct`: 연속 마침표 정규화
- `--workers`: 병렬 처리 수 (기본: 1)
- `--diarize`: 화자 구분 활성화 (기본: True)
- `--initial_prompt`: 도메인 특화 용어 힌트
- `--min_seg_length`: 세그먼트 최소 길이 (기본: 2)

### correct.py & summarize.py  
- `--model`: Ollama 모델 지정
- `--temperature`: 생성 온도 (기본: 0.0~0.2)
- `--chunk_size`: 청크 크기 (요약 시)

## 에러 처리 및 로깅
- 파일별 개별 에러 처리로 전체 작업 중단 방지
- 상세한 로깅으로 디버깅 지원
- 원자적 파일 쓰기로 데이터 손실 방지
- 재시도 메커니즘 (summarize.py)

## 확장 계획 (TodoList.md 기준)
- [ ] 임베딩 및 RAG 질의 시스템
- [ ] 웹 UI 개발
- [ ] LLM API 연동 확장
- [ ] 다국어 지원 강화

## 성능 최적화 팁
1. 단일 GPU 환경에서는 `--workers 1` 사용 권장
2. 대용량 파일은 청킹 처리로 메모리 효율성 확보
3. 플랫폼별 최적화 모델 사용으로 성능 향상
4. 캐시 활용으로 모델 로딩 시간 단축

## 트러블슈팅
- **모델 로딩 실패**: 캐시 경로와 모델 파일 존재 여부 확인
- **Ollama 연결 오류**: Ollama 서비스 실행 상태 점검
- **FFmpeg 오류**: 시스템 PATH 환경변수에 FFmpeg 경로 추가
- **인코딩 문제**: UTF-8, CP949, EUC-KR 순으로 자동 시도
- **화자 구분 실패**: PYANNOTE_TOKEN 환경변수 설정 확인
- **MPS 오류**: Apple Silicon에서 GPU 실패 시 CPU로 자동 전환
- **M4A 변환 오류**: FFmpeg 설치 및 PATH 설정 확인