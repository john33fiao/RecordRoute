# RecordRoute
음성 파일을 회의록으로 변환하는 통합 워크플로우 시스템입니다. STT(Speech-to-Text), 텍스트 교정, 요약 기능을 단계적으로 제공합니다.

## 주요 기능

- **음성→텍스트 변환**: OpenAI Whisper를 사용한 고품질 음성 인식
- **화자 구분**: pyannote.audio를 통한 화자별 구간 구분
- **텍스트 교정**: Ollama LLM을 활용한 문법/어법 개선
- **구조화된 요약**: 회의록 형태의 체계적 요약 생성
- **통합 워크플로우**: 1단계부터 3단계까지 자동화된 처리 파이프라인

## 디렉토리 구조

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

## 설치 및 설정

### 1. 자동 설치 (권장)

#### Windows
```bash
# 1단계: 환경 설정
setup.bat

# 2단계: 워크플로우 실행
run.bat
```

#### Unix/macOS/Linux
```bash
# 워크플로우 실행 (.env 파일에서 환경변수 자동 로드)
./run.sh
```

### 2. 수동 설치

#### Python 패키지
```bash
pip install -r sttEngine/requirements.txt
```

**포함 패키지:**
- `openai-whisper>=20231117`: 음성 인식
- `ollama>=0.1.0`: 로컬 LLM 추론
- `pyannote.audio>=2.1.1`: 화자 구분

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

# macOS/Linux 사용자
ollama pull gemma3:12b-it-qat
ollama pull gpt-oss:20b
```

### 4. 화자 구분 설정

화자 구분 기능 사용을 위해:

1. [Hugging Face](https://huggingface.co/)에서 토큰 발급
2. 환경변수 설정:
   ```bash
   # Windows
   set PYANNOTE_TOKEN=your_token_here
   
   # Unix/macOS/Linux
   export PYANNOTE_TOKEN=your_token_here
   ```

## 사용법

### 통합 워크플로우 실행
```bash
# Windows
run.bat

# Unix/macOS/Linux
./run.sh

# 또는 직접 실행
python sttEngine/run_workflow.py
```

### 단계별 실행

#### 1단계: 음성→텍스트 변환
```bash
python sttEngine/workflow/transcribe.py [audio_folder] --model_size large-v3-turbo --language ko --diarize --filter_fillers
```

**주요 옵션:**
- `--model_size`: Whisper 모델 크기 (tiny, base, small, medium, large, large-v3-turbo)
- `--language ko`: 한국어 힌트
- `--diarize`: 화자 구분 활성화
- `--filter_fillers`: 필러 단어 제거
- `--normalize_punct`: 연속 마침표 정규화

#### 2단계: 텍스트 교정
```bash
python sttEngine/workflow/correct.py input.md --model gemma3:4b --temperature 0.0  # Windows
python sttEngine/workflow/correct.py input.md --model gemma3:12b-it-qat --temperature 0.0  # macOS/Linux
```

#### 3단계: 텍스트 요약
```bash
python sttEngine/workflow/summarize.py input.corrected.md --model gemma3:4b --temperature 0.0  # Windows
python sttEngine/workflow/summarize.py input.corrected.md --model gpt-oss:20b --temperature 0.0  # macOS/Linux
```

## 지원 오디오 포맷

- `.flac`, `.m4a`, `.mp3`, `.mp4`, `.mpeg`, `.mpga`, `.oga`, `.ogg`, `.wav`, `.webm`
- **M4A 자동 변환**: m4a 파일을 wav로 자동 변환하여 처리

## 플랫폼별 최적화

### Windows
- 모델: `gemma3:4b` (교정 및 요약 공용)
- 캐시: `%USERPROFILE%\.cache\whisper\`
- Python 실행파일: 자동 감지

### macOS/Linux
- 모델: 교정 `gemma3:12b-it-qat`, 요약 `gpt-oss:20b`
- 캐시: `~/.cache/whisper/`
- Python 실행파일: `venv/bin/python` (가상환경 사용)
- 환경변수: `.env` 파일에서 자동 로드
- **Apple Silicon MPS**: GPU/MPS 우선 사용, CPU fallback 지원

## 처리 단계

### 1단계: 음성→텍스트
- OpenAI Whisper `large-v3-turbo` 모델 사용
- 화자 구분 기능 (기본 활성화)
- 세그먼트 병합 및 필러 단어 필터링
- 결과: `.md` 파일

### 2단계: 텍스트 교정
- 한국어 문법/어법/오탈자 수정
- 원문 의미와 사실 보존
- 마크다운 구조 및 화자 표기 유지
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

### 화자 구분 관련
- **PYANNOTE_TOKEN**: Hugging Face 토큰 환경변수 설정 확인
- **MPS 오류**: Apple Silicon에서 GPU 실패 시 CPU로 자동 전환
- **M4A 변환 오류**: FFmpeg 설치 및 PATH 설정 확인

## 참고사항

- 이 프로젝트는 개인적인 학습 목적으로 진행되었습니다.
- 상용 서비스에 적용하기 위해서는 추가적인 검토와 개선이 필요합니다.
- 사용 중 발생하는 문제에 대해서는 책임지지 않습니다.

### 추가 참고사항

- 본 레포지토리는 문과 출신 기획자에 의해 운영됩니다. LLM 자체에 대한 학습을 목적으로 합니다.
- 대부분의 코드는 LLM(Claude > Gemini > ChatGPT 순)으로 작성되었습니다.
- 구현 예정사항은 [Todo List](/TodoList.md)로 정리합니다.
