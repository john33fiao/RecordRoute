# RecordRoute (Rust) 🦀

**AI 기반 음성 전사 및 의미 검색 시스템**

RecordRoute는 음성 파일을 자동으로 전사(STT)하고, AI로 요약하며, 의미 기반 검색을 제공하는 완전한 Rust 구현입니다.

## ✨ 주요 기능

- 🎤 **음성 전사** - Whisper.cpp 기반 고성능 STT
- 📝 **AI 요약** - Ollama LLM을 활용한 자동 요약 (Map-Reduce)
- 🧠 **벡터 검색** - 의미 기반 문서 검색 (임베딩 + 코사인 유사도)
- 🌐 **REST API** - HTTP/WebSocket 서버
- ⚡ **고성능** - Rust의 메모리 안전성과 성능
- 🔄 **비동기 처리** - Tokio 기반 async/await

## 🏗️ 시스템 구조

```
📤 파일 업로드
    ↓
🎤 음성 전사 (Whisper)
    ↓
📝 AI 요약 (Ollama)
    ↓
🧠 벡터 임베딩
    ↓
🔍 의미 기반 검색
```

## 📦 프로젝트 구조

```
recordroute-rs/
├── crates/
│   ├── common/          # 공통 모듈 (설정, 에러)
│   ├── stt/             # STT 엔진 (Whisper)
│   ├── llm/             # LLM 통합 (Ollama)
│   ├── vector/          # 벡터 검색 엔진
│   ├── server/          # HTTP/WebSocket 서버
│   └── recordroute/     # 메인 바이너리
├── models/              # AI 모델 파일
└── Cargo.toml           # 워크스페이스 설정
```

## 🚀 빠른 시작

### 필수 요구사항

- **Rust** 1.75 이상
- **Ollama** (LLM 및 임베딩용)
- **Whisper 모델** (ggml 형식)

### 설치

1. **저장소 클론**:
```bash
git clone https://github.com/yourusername/RecordRoute.git
cd RecordRoute/recordroute-rs
```

2. **의존성 빌드**:
```bash
cargo build --release
```

3. **Ollama 설치 및 실행**:
```bash
# https://ollama.ai/ 에서 다운로드
ollama pull llama3.2
ollama pull nomic-embed-text
ollama serve
```

4. **Whisper 모델 다운로드**:

> **🚧 향후 개선 예정**: 자동 모델 다운로드 기능이 추가될 예정입니다. ([TODO/Rust.md Phase 7](../TODO/Rust.md#-phase-7-모델-관리-및-배포) 참조)

현재는 수동으로 모델을 다운로드해야 합니다:
```bash
# models/ 디렉토리에 ggml 모델 다운로드
mkdir -p models
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin -O models/ggml-base.bin
```

**사용 가능한 모델 크기**:
- `ggml-tiny.bin` (75 MB) - 가장 빠름, 낮은 정확도
- `ggml-base.bin` (142 MB) - 권장 (균형)
- `ggml-small.bin` (466 MB) - 높은 정확도
- `ggml-medium.bin` (1.5 GB) - 매우 높은 정확도
- `ggml-large-v3.bin` (3.1 GB) - 최고 정확도

### 환경 설정

`.env` 파일 생성:

```bash
# 데이터베이스 경로
DB_BASE_PATH=./data

# 업로드 디렉토리
UPLOAD_DIR=./uploads

# Whisper 모델
WHISPER_MODEL=./models/ggml-base.bin

# Ollama 설정
OLLAMA_BASE_URL=http://localhost:11434
LLM_MODEL=llama3.2
EMBEDDING_MODEL=nomic-embed-text

# 서버 설정
SERVER_HOST=0.0.0.0
SERVER_PORT=8080

# 로그 설정
LOG_DIR=./logs
LOG_LEVEL=info

# 벡터 인덱스
VECTOR_INDEX_PATH=./data/vector_index.json
```

### 실행

```bash
cargo run --release
```

서버가 `http://localhost:8080` 에서 시작됩니다.

## 📖 사용법

### 1. 파일 업로드

```bash
curl -F "file=@meeting.mp3" http://localhost:8080/upload
```

**응답**:
```json
{
  "file_uuid": "550e8400-e29b-41d4-a716-446655440000",
  "filename": "meeting.mp3",
  "path": "/uploads/550e8400-e29b-41d4-a716-446655440000.mp3"
}
```

### 2. 워크플로우 실행

```bash
curl -X POST http://localhost:8080/process \
  -H "Content-Type: application/json" \
  -d '{
    "file_uuid": "550e8400-e29b-41d4-a716-446655440000",
    "run_stt": true,
    "run_summarize": true,
    "run_embed": true
  }'
```

**응답**:
```json
{
  "task_id": "task-1234",
  "message": "Task started for stt"
}
```

### 3. 작업 상태 확인

```bash
curl http://localhost:8080/tasks
```

**응답**:
```json
{
  "tasks": [
    {
      "task_id": "task-1234",
      "task_type": "stt",
      "status": "Running",
      "progress": 45,
      "message": "Writing transcription results..."
    }
  ]
}
```

### 4. 의미 기반 검색

```bash
curl "http://localhost:8080/search?q=프로젝트 회의&top_k=5"
```

**응답**:
```json
{
  "results": [
    {
      "doc_id": "550e8400-e29b-41d4-a716-446655440000",
      "score": 0.92,
      "filename": "meeting.mp3",
      "one_line_summary": "프로젝트 진행 상황 및 다음 단계 논의",
      "transcript_path": "/data/whisper_output/550e8400-e29b-41d4-a716-446655440000.txt",
      "summary_path": "/data/whisper_output/550e8400-e29b-41d4-a716-446655440000_summary.txt"
    }
  ],
  "query": "프로젝트 회의",
  "count": 1
}
```

### 5. 히스토리 조회

```bash
curl http://localhost:8080/history
```

### 6. 파일 다운로드

```bash
curl http://localhost:8080/download/550e8400-e29b-41d4-a716-446655440000.txt \
  -o transcript.txt
```

## 🛠️ API 엔드포인트

| 메서드 | 경로 | 설명 |
|--------|------|------|
| `POST` | `/upload` | 파일 업로드 |
| `POST` | `/process` | STT/요약/임베딩 실행 |
| `GET` | `/history` | 처리 히스토리 조회 |
| `POST` | `/delete` | 히스토리 삭제 |
| `GET` | `/download/{file}` | 결과 파일 다운로드 |
| `GET` | `/tasks` | 작업 상태 조회 |
| `POST` | `/cancel` | 작업 취소 |
| `GET` | `/search` | 의미 기반 검색 |
| `GET` | `/search/stats` | 검색 인덱스 통계 |

자세한 API 문서는 [API.md](./API.md)를 참고하세요.

## 🧪 개발

### 테스트 실행

```bash
cargo test
```

### 개발 모드 실행

```bash
cargo run
```

### 코드 포맷팅

```bash
cargo fmt
```

### Linting

```bash
cargo clippy
```

## 📊 성능

- **STT 처리 속도**: ~10x 실시간 (Whisper base 모델 기준)
- **메모리 사용량**: ~500MB (기본 설정)
- **동시 처리**: 무제한 (비동기 기반)

## 🔧 트러블슈팅

### Whisper 모델 로딩 실패
```
Error: STT error: Failed to load Whisper model
```
→ `WHISPER_MODEL` 경로가 올바른지 확인하세요.

### Ollama 연결 실패
```
Error: Failed to connect to Ollama
```
→ Ollama가 실행 중인지 확인: `ollama serve`

### 메모리 부족
→ 더 작은 Whisper 모델 사용 (tiny, base)

## 📝 라이선스

MIT License

## 🙏 감사의 글

- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) - 고성능 Whisper 구현
- [Ollama](https://ollama.ai/) - 로컬 LLM 실행
- [Actix-web](https://actix.rs/) - Rust 웹 프레임워크

## 📮 문의

이슈나 질문은 [GitHub Issues](https://github.com/yourusername/RecordRoute/issues)에 등록해주세요.
