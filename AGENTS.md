# AGENTS.md - RecordRoute AI 에이전트 참조 문서

## 프로젝트 메타데이터

- **타입**: AI 기반 음성 전사 및 의미 검색 시스템
- **기술스택**: Rust, Whisper.cpp, Ollama, Axum, FFmpeg
- **플랫폼**: Windows/macOS/Linux 크로스플랫폼
- **실행환경**: Axum 웹서버 (HTTP/WebSocket) + Electron 데스크톱 앱

## 1. 프로젝트 개요 (Project Overview)

RecordRoute는 **Rust로 완전히 재구현된** 고성능 음성 전사 및 문서 분석 시스템입니다. Python 레거시 코드에서 Rust로 성공적으로 마이그레이션되어, 더 빠르고 안정적이며 메모리 안전한 구조로 작동합니다.

### 핵심 처리 플로우
```
파일 업로드 → [Whisper.cpp STT] → [Ollama 요약] → [벡터 임베딩] → [의미 검색]
```

### 주요 기능
1. **음성 → 텍스트 변환 (STT):** `whisper.cpp` Rust 바인딩을 사용하여 미디어 파일에서 텍스트를 추출합니다.
2. **AI 요약:** `Ollama` HTTP API를 통해 추출된 텍스트를 Map-Reduce 방식으로 구조화된 요약으로 변환합니다.
3. **벡터 임베딩 및 검색:** Ollama 임베딩 모델을 사용하여 문서를 벡터화하고, 코사인 유사도 기반 의미 검색을 제공합니다.

## 2. 기술 스택 (Tech Stack)

### 핵심 기술
- **언어:** Rust (1.75+)
- **음성 인식:** `whisper-rs` (whisper.cpp 바인딩)
- **LLM (요약):** `Ollama` (HTTP API)
- **웹 프레임워크:** `Axum` (Tokio 기반 비동기)
- **WebSocket:** `axum::extract::ws`
- **벡터 연산:** `ndarray`

### 주요 의존성
- **whisper-rs**: Rust Whisper.cpp 바인딩
- **reqwest**: HTTP 클라이언트 (Ollama API 통신)
- **axum**: 비동기 웹 프레임워크
- **tokio**: 비동기 런타임
- **serde**: 직렬화/역직렬화
- **indicatif**: 진행률 표시 (모델 다운로드)
- **sha2**: 파일 무결성 검증

### 시스템 의존성
- **FFmpeg**: 오디오/비디오 파일 처리 (시스템 PATH 필요)
- **Ollama**: 로컬 LLM 서비스, 백그라운드 실행 필수
  - 요약 모델: `llama3.2` 또는 `gemma3:8b`
  - 임베딩 모델: `nomic-embed-text`

## 3. 디렉토리 구조 (Directory Structure)

```
RecordRoute/
├── README.md                 # 프로젝트 소개 및 설치 가이드
├── AGENTS.md                 # AI 에이전트 통합 가이드 (본 문서)
├── LICENSE                   # 라이선스 정보
├── package.json              # NPM Workspaces 루트 설정
├── .gitignore                # Git 제외 파일 목록
│
├── TODO/                     # 기능 구현 계획 및 로드맵
│   ├── TODO.md               # 전체 TODO 목록
│   ├── Rust.md               # Rust 마이그레이션 로드맵
│   ├── Electron.md           # Electron 통합 가이드
│   ├── GUI.md                # 프론트엔드 개선 계획
│   ├── MVVM.md               # MVVM 아키텍처 전환 계획
│   └── Graph.md              # 문서 그래프 시각화 계획
│
├── configs/                  # 환경 설정 템플릿
├── data/                     # 실행 데이터 저장소 (.gitignore)
├── models/                   # AI 모델 저장소 (.gitignore)
│
├── electron/                 # Electron 데스크톱 애플리케이션
│   ├── main.js               # Electron 메인 프로세스 (Rust 백엔드 실행)
│   ├── preload.js            # Preload 스크립트
│   └── package.json          # Electron 워크스페이스 설정
│
├── frontend/                 # 웹 인터페이스 (HTML/CSS/JS)
│   ├── upload.html           # 메인 UI
│   ├── upload.js             # 프론트엔드 로직
│   ├── upload.css            # 스타일시트
│   └── package.json          # Frontend 워크스페이스 설정
│
├── legacy/                   # 레거시 Python 백엔드 (참고용, 유지보수 중단)
│   └── python-backend/       # 구버전 Python 코드
│       ├── server.py         # 구버전 FastAPI/WebSocket 서버
│       ├── config.py         # Python 설정 관리
│       ├── requirements.txt  # Python 의존성
│       └── workflow/         # Python 워크플로우 모듈
│           ├── transcribe.py # STT 워크플로우
│           ├── summarize.py  # 요약 워크플로우
│           └── correct.py    # 텍스트 교정 워크플로우
│
├── tools/                    # 개발 도구 및 스크립트
│   ├── scripts/              # 빌드 및 실행 스크립트
│   │   ├── build-all.sh      # 전체 빌드 스크립트
│   │   ├── build-backend.sh  # Python 백엔드 빌드 (레거시)
│   │   ├── build-backend.bat # Windows용 빌드 스크립트
│   │   ├── start.bat         # Windows 실행 스크립트
│   │   ├── start.vbs         # Windows 숨김 실행 스크립트
│   │   └── run.command       # macOS/Linux 실행 스크립트
│   └── dev-env/              # 개발 환경 설정
│
└── recordroute-rs/           # 메인 Rust 백엔드 (현재 운영 중)
    ├── Cargo.toml            # Cargo 워크스페이스 설정
    ├── README.md             # Rust 백엔드 문서
    ├── API.md                # API 상세 문서
    ├── ARCHITECTURE.md       # 아키텍처 상세 문서
    ├── CONFIGURATION.md      # 설정 가이드
    └── crates/               # 워크스페이스 크레이트
        ├── common/           # 공통 모듈 (설정, 에러, 로거, 모델 관리)
        ├── stt/              # STT 엔진 (whisper.cpp)
        ├── llm/              # LLM 클라이언트 (Ollama API)
        ├── vector/           # 벡터 검색 엔진
        ├── server/           # Axum 웹 서버 및 API 라우트
        └── recordroute/      # 실행 바이너리
```

## 4. 코어 시스템 구조

### Rust 크레이트 구조

#### recordroute-rs/crates/common
**기능**: 공통 모듈, 설정 관리, 에러 처리, 로깅, 모델 자동 다운로드

**핵심 모듈**:
- `config.rs`: 환경 변수 기반 설정 관리 (`AppConfig` 구조체)
- `error.rs`: 통합 에러 타입 (`RecordRouteError`)
- `logger.rs`: 구조화된 로깅 시스템
- `model_manager.rs`: Whisper 모델 자동 다운로드 및 검증
  - Hugging Face에서 GGML 모델 자동 다운로드
  - SHA256 해시 검증
  - 진행률 표시 (indicatif)

#### recordroute-rs/crates/stt
**기능**: Whisper.cpp 기반 음성 인식

**핵심 모듈**:
- `whisper.rs`: WhisperEngine 구조체, 모델 로딩 및 전사
- `audio.rs`: 오디오 전처리 (16kHz 모노 변환, FFmpeg 연동)
- `postprocess.rs`: 텍스트 후처리 (반복 제거, 정규화)

**주요 기능**:
- GPU/CPU 자동 감지 및 최적화
- 다양한 오디오 포맷 지원 (FFmpeg 기반)
- 실시간 진행 상황 콜백 지원

#### recordroute-rs/crates/llm
**기능**: Ollama HTTP API 클라이언트 및 텍스트 요약

**핵심 모듈**:
- `client.rs`: Ollama HTTP 클라이언트 (`OllamaClient` 구조체)
  - `generate()`: 텍스트 생성
  - `embed()`: 임베딩 생성
  - 재시도 로직 및 에러 핸들링
- `summarize.rs`: Map-Reduce 알고리즘 기반 요약
  - 텍스트 청킹 (2000 토큰 단위)
  - 병렬 청크 요약
  - 최종 요약 병합
- `prompts.rs`: 프롬프트 템플릿 관리

#### recordroute-rs/crates/vector
**기능**: 벡터 임베딩 및 의미 검색

**핵심 모듈**:
- `engine.rs`: 벡터 인덱스 관리 (`VectorIndex` 구조체)
  - 임베딩 생성 및 저장
  - 문서 추가/삭제
  - 메타데이터 관리
- `similarity.rs`: 코사인 유사도 계산 및 Top-K 검색
- 날짜 필터링 지원

#### recordroute-rs/crates/server
**기능**: Axum 기반 HTTP/WebSocket 서버

**핵심 모듈**:
- `lib.rs`: 서버 초기화 및 라우팅
- `state.rs`: 애플리케이션 공유 상태 (`AppState`)
- `workflow.rs`: 워크플로우 오케스트레이션 (STT → 요약 → 임베딩)
- `job_manager.rs`: 작업 큐 관리 및 진행 상황 추적
- `history.rs`: 히스토리 관리 (JSON 파일 기반)
- `websocket.rs`: WebSocket 실시간 통신
- `routes/`: API 엔드포인트
  - `upload.rs`: 파일 업로드
  - `process.rs`: 워크플로우 실행
  - `download.rs`: 결과 파일 다운로드
  - `history.rs`: 히스토리 조회/삭제
  - `tasks.rs`: 작업 상태 조회/취소
  - `search.rs`: 벡터 검색

#### recordroute-rs/crates/recordroute
**기능**: 메인 실행 바이너리

**핵심 기능**:
- CLI 인자 파싱
- 모델 자동 다운로드 확인
- 서버 시작 및 종료 처리

## 5. 설치 및 설정 (Setup)

### Rust 백엔드 설정

```bash
# 1. 저장소 클론
git clone https://github.com/your-repo/RecordRoute.git
cd RecordRoute

# 2. Rust 백엔드 빌드 (첫 실행 시 모델 자동 다운로드)
cd recordroute-rs
cargo build --release

# 3. Ollama 설치 및 모델 다운로드
ollama pull llama3.2
ollama pull nomic-embed-text
ollama serve  # 백그라운드 실행
```

### 환경 변수 (.env)

recordroute-rs 디렉토리에 `.env` 파일 생성:

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

## 6. 실행 방법 (How to Run)

### Rust 백엔드 단독 실행

```bash
cd recordroute-rs
cargo run --release
# 서버가 http://localhost:8080 에서 시작됨
```

브라우저에서 `http://localhost:8080` 접속하여 웹 UI 사용

### Electron 데스크톱 앱 실행

```bash
# 1. Rust 백엔드 빌드
cd recordroute-rs
cargo build --release

# 2. Electron 실행
cd ..
npm install
npm start
```

## 7. API 엔드포인트 스펙

자세한 API 문서는 `recordroute-rs/API.md`를 참조하세요.

### POST /upload
- **기능**: 파일 업로드
- **입력**: multipart/form-data
- **출력**: `{ "file_uuid": "...", "filename": "...", "path": "..." }`

### POST /process
- **기능**: 워크플로우 실행 (STT, 요약, 임베딩)
- **입력**: `{ "file_uuid": "...", "run_stt": true, "run_summarize": true, "run_embed": true }`
- **출력**: `{ "task_id": "...", "message": "..." }`

### GET /tasks
- **기능**: 현재 진행 중인 작업 목록 조회
- **출력**: `{ "tasks": [{ "task_id": "...", "task_type": "...", "status": "...", "progress": 50 }] }`

### POST /cancel
- **기능**: 작업 취소
- **입력**: `{ "task_id": "..." }`

### GET /history
- **기능**: 처리 완료된 기록 목록
- **출력**: `[{ "id": "...", "filename": "...", "created_at": "...", ... }]`

### POST /delete
- **기능**: 기록 삭제
- **입력**: `{ "record_ids": ["..."] }`

### GET /download/{filename}
- **기능**: 결과 파일 다운로드

### GET /search?q=<query>&top_k=5
- **기능**: 의미 기반 검색 (키워드 + 벡터 검색)
- **출력**: `{ "results": [{ "doc_id": "...", "score": 0.92, "filename": "...", ... }] }`

### GET /search/stats
- **기능**: 벡터 인덱스 통계 (문서 수, 인덱스 크기 등)

### WebSocket: ws://localhost:8080/ws
- **기능**: 실시간 작업 진행 상황 브로드캐스트
- **메시지 형식**: `{ "task_id": "...", "message": "Processing...", "progress": 45 }`

## 8. 백그라운드 작업큐 구조

### 작업 생명주기
```
파일 업로드 → 작업큐 등록 (task_id) → 비동기 실행 → 진행 상태 WebSocket 전송 → 완료
```

### JobManager
- **기능**: 작업 상태 추적, 진행률 관리
- **특징**:
  - Tokio 비동기 작업으로 실행
  - `Arc<Mutex<>>` 기반 스레드 안전 상태 관리
  - WebSocket을 통한 실시간 진행 상황 브로드캐스트

### 비동기 작업 흐름
1. 사용자가 웹 UI에서 "처리 시작"을 누르면, `/process` API 호출
2. 서버는 작업을 즉시 백그라운드로 스폰하고 `task_id` 반환
3. `upload.js`는 WebSocket을 통해 해당 작업의 진행 상황을 실시간 수신
4. UI는 진행률 표시, 완료 시 히스토리 목록 갱신

## 9. 벡터 검색 시스템

### 임베딩 생성
- **모델**: Ollama `nomic-embed-text` (768차원)
- **방식**: Ollama HTTP API `/api/embed` 호출
- **저장**: JSON 파일 (`vector_index.json`)

### 유사도 검색
- **알고리즘**: 코사인 유사도 (ndarray 기반)
- **필터링**: 날짜 범위 필터 지원
- **성능**: SIMD 최적화 (ndarray 자동 활용)

### 검색 결과
- **Top-K 알고리즘**: 유사도 점수 순 정렬
- **메타데이터**: 파일명, 요약, 타임스탬프 포함

## 10. AI 에이전트 활용 가이드

### 워크플로우 스크립트 수정

#### 요약 프롬프트 변경
`recordroute-rs/crates/llm/src/prompts.rs` 파일 수정:

```rust
pub const SUMMARY_PROMPT_TEMPLATE: &str = r#"
다음 텍스트를 구조화된 회의록 형태로 요약해주세요:

{text}

요약 형식:
1. 주요 주제
2. 핵심 내용
3. 결론 및 향후 계획
"#;
```

#### Whisper 모델 변경
`.env` 파일에서 `WHISPER_MODEL` 경로 수정:

```bash
# 더 작은 모델 사용 (빠름, 낮은 정확도)
WHISPER_MODEL=./models/ggml-tiny.bin

# 더 큰 모델 사용 (느림, 높은 정확도)
WHISPER_MODEL=./models/ggml-large-v3.bin
```

### 웹 UI 및 비동기 작업 디버깅

#### 서버 로그 확인
```bash
# 로그 파일 위치
recordroute-rs/logs/app.log

# 실시간 로그 확인
tail -f recordroute-rs/logs/app.log
```

#### 프론트엔드 디버깅
1. **브라우저 개발자 도구 (F12)** 열기
2. **Console 탭**: JavaScript 에러 확인
3. **Network 탭**: API 요청/응답 확인
4. **WebSocket**: ws://localhost:8080/ws 연결 상태 확인

#### 작업 상태 확인
```bash
# 현재 진행 중인 작업 조회
curl http://localhost:8080/tasks
```

## 11. 디버깅 전략

### 단계별 독립 실행

각 워크플로우 단계를 독립적으로 테스트:

```bash
# STT 테스트 (Rust 바이너리)
curl -X POST http://localhost:8080/process \
  -H "Content-Type: application/json" \
  -d '{"file_uuid": "...", "run_stt": true, "run_summarize": false, "run_embed": false}'

# 요약 테스트 (전사 파일이 이미 존재하는 경우)
curl -X POST http://localhost:8080/process \
  -H "Content-Type: application/json" \
  -d '{"file_uuid": "...", "run_stt": false, "run_summarize": true, "run_embed": false}'
```

### 로그 분석 포인트

- **common/logger.rs**: 구조화된 로그 출력 (JSON 형식)
- **stt/whisper.rs**: Whisper 모델 로딩, 전사 진행 상황
- **llm/client.rs**: Ollama API 연결, 요청/응답 로그
- **server/workflow.rs**: 워크플로우 단계별 실행 로그
- **server/job_manager.rs**: 작업 상태 전환 로그

### 환경 검증 체크리스트

- ✅ Rust toolchain 설치 (`rustc --version`)
- ✅ Ollama 서비스 실행 (`curl http://localhost:11434/api/tags`)
- ✅ Ollama 모델 다운로드 (`ollama list`)
- ✅ FFmpeg PATH 설정 (`ffmpeg -version`)
- ✅ Whisper 모델 존재 (`ls recordroute-rs/models/`)
- ✅ .env 파일 설정 확인

## 12. 확장 개발 가이드

### 새로운 API 엔드포인트 추가

1. `recordroute-rs/crates/server/src/routes/` 디렉토리에 새 파일 생성
2. Axum 핸들러 함수 구현
3. `recordroute-rs/crates/server/src/lib.rs`에 라우트 등록

예시:
```rust
// routes/custom.rs
pub async fn custom_handler() -> impl IntoResponse {
    Json(json!({ "message": "Custom endpoint" }))
}

// lib.rs
use crate::routes::custom::custom_handler;

let app = Router::new()
    .route("/custom", get(custom_handler));
```

### 새로운 워크플로우 단계 추가

1. `recordroute-rs/crates/server/src/workflow.rs` 수정
2. 새로운 단계 함수 추가
3. `execute_workflow()` 함수에 로직 추가

### 새로운 LLM 모델 지원

`.env` 파일에서 모델 이름만 변경:

```bash
LLM_MODEL=mistral:7b
# 또는
LLM_MODEL=gemma2:9b
```

Ollama에 해당 모델이 다운로드되어 있어야 합니다.

## 13. 성능 최적화

### Rust 백엔드 장점
- **메모리 안전성**: 컴파일 타임 보장, 런타임 오버헤드 없음
- **비동기 처리**: Tokio 기반 고성능 비동기 I/O
- **타입 안전성**: 컴파일 타임 에러 검출
- **제로 코스트 추상화**: Python 대비 10-50배 빠른 처리 속도

### 성능 지표 (Python 대비)
- **STT 처리 속도**: 20-40% 향상
- **LLM 추론**: 10-20% 향상 (Ollama API 동일 사용)
- **벡터 검색**: 5-10배 향상
- **메모리 사용량**: 30-50% 감소
- **서버 응답 시간**: 30-50% 단축

## 14. 프로젝트 참조 포인트

### Claude Code / MCP 에이전트
- Rust 코드 구조 이해 및 수정
- 크레이트 간 의존성 관리
- Axum 라우팅 및 미들웨어
- 비동기 프로그래밍 (Tokio)
- 에러 처리 및 타입 안전성

### Gemini 에이전트
- 프로젝트 개요 및 아키텍처
- 설치 및 실행 가이드
- API 문서 및 엔드포인트
- 디버깅 및 로그 분석

### 공통 활용
- 코드 수정 시 영향 범위 파악
- 새로운 기능 추가 시 관련 크레이트 확인
- 문제 발생 시 로그 및 환경 검증
- 성능 최적화 및 벤치마크

## 15. 마이그레이션 히스토리

### Python → Rust 전환 완료
- **기간**: 2024-09 ~ 2024-12 (약 4개월)
- **상태**: 모든 핵심 기능 전환 완료 ✅

### 주요 성과
- ✅ Whisper.cpp Rust 바인딩 통합
- ✅ Ollama HTTP API 클라이언트 구현
- ✅ 벡터 검색 엔진 Rust 재구현
- ✅ Axum 웹 서버 완전 전환
- ✅ Electron 앱 Rust 백엔드 통합
- ✅ 모델 자동 다운로드 시스템 구현

### 레거시 코드
- Python 코드는 `legacy/python-backend`에 보관
- 참고용으로 유지, 더 이상 유지보수하지 않음

## 16. 추가 참고 자료

- `recordroute-rs/README.md`: Rust 백엔드 상세 문서
- `recordroute-rs/API.md`: API 엔드포인트 전체 목록
- `recordroute-rs/ARCHITECTURE.md`: 아키텍처 상세 설명
- `recordroute-rs/CONFIGURATION.md`: 설정 가이드
- `TODO/Rust.md`: Rust 마이그레이션 로드맵
- `TODO/Electron.md`: Electron 통합 가이드
