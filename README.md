# RecordRoute
음성을 회의록으로, STT·교정·요약·임베딩·RAG 질의 지원까지 일원화.

## 설치 및 설정

### 1. 필수 라이브러리 설치

#### Python 패키지
```bash
pip install openai-whisper
pip install ollama
```

#### Windows에서 추가 설치 (FFmpeg)
Whisper가 다양한 오디오 형식을 처리하려면 FFmpeg가 필요합니다:
```bash
# Chocolatey 사용 시
choco install ffmpeg

# 또는 직접 다운로드: https://ffmpeg.org/download.html
```

### 2. Whisper 모델 설정

#### 자동 다운로드 (권장)
첫 실행 시 모델이 자동으로 다운로드됩니다:
```bash
python sttEngine/workflow/transcribe.py [audio_folder]
```

#### 수동 모델 설치
특정 모델을 미리 다운로드하려면:
```python
import whisper
model = whisper.load_model("large-v3-turbo")  # 약 3GB
```

**지원 모델 크기:**
- `tiny` (39MB) - 빠르지만 정확도 낮음
- `base` (74MB) - 기본 수준
- `small` (244MB) - 실용적 선택
- `medium` (769MB) - 높은 정확도
- `large` (1550MB) - 최고 정확도
- `large-v3-turbo` (3GB) - 최신 고성능 (권장)

### 3. Ollama 설정 (텍스트 교정/요약용)

#### Ollama 설치
```bash
# Windows
winget install Ollama.Ollama

# 또는 https://ollama.com/download 에서 설치
```

#### 필요 모델 다운로드
```bash
# 텍스트 교정용
ollama pull gpt-oss:20b

# 요약용  
ollama pull llama3.2
```

### 4. 모델 캐시 경로

**Windows:** `%USERPROFILE%\.cache\whisper\`  
**macOS/Linux:** `~/.cache/whisper/`

처음 실행 시 모델이 해당 경로에 자동 저장됩니다.

## 참고사항

- 이 프로젝트는 개인적인 학습 목적으로 진행되었습니다.
- 상용 서비스에 적용하기 위해서는 추가적인 검토와 개선이 필요합니다.
- 사용 중 발생하는 문제에 대해서는 책임지지 않습니다.

### 추가 참고사항

- 본 레포지토리는 문과 출신 기획자에 의해 운영됩니다. 
- 대부분의 코드는 LLM(Claude > Gemini > ChatGPT 순)으로 작성되었습니다.
- 구현 예정사항은 [Todo List](/TodoList.md)로 정리합니다. 