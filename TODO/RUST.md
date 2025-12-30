# Rust Conversion Roadmap: Project RecordRoute

이 문서는 기존 Python 기반의 `sttEngine`을 Rust로 완전히 전환하기 위한 단계별 계획을 기술합니다.
주요 목표는 **Type Safety 확보, 단일 실행 파일 배포, AI 추론 성능 최적화**이며, 러스트 학습을 위해 순수 러스트 생태계(Candle 등)를 적극적으로 검토합니다.

## 1. Tech Stack Selection (기술 스택 선정)

기존 Python 라이브러리를 대체할 Rust Crates(라이브러리) 선정표입니다.

| Category | Python (Current) | Rust (Target) | Note |
| :--- | :--- | :--- | :--- |
| **Web Framework** | `http.server`, `flask` style | **`axum`** | `Tokio` 기반의 비동기 웹 프레임워크. 성능과 생태계 호환성 최우수. |
| **Async Runtime** | `threading`, `asyncio` | **`tokio`** | Rust 비동기 런타임의 표준. |
| **Serialization** | `json` | **`serde`, `serde_json`** | 구조체 기반의 강력한 직렬화/역직렬화 지원. |
| **STT (ASR)** | `openai-whisper` | **`whisper-rs`** | C++ 구현체(`whisper.cpp`)의 Rust 바인딩. |
| **LLM Inference** | `llama-cpp-python` | **`candle-core`** or **`llama_cpp_rs`** | 학습 목적이라면 HuggingFace의 순수 Rust 프레임워크인 `Candle` 사용 권장. |
| **Process Control** | `subprocess` (ffmpeg) | **`std::process::Command`** | 외부 프로세스(FFmpeg) 호출 및 제어. |
| **Config/Env** | `python-dotenv`, `os` | **`dotenvy`, `config`** | 환경 변수 및 설정 파일 관리. |
| **Logging** | `logging` | **`tracing`, `tracing-subscriber`** | 비동기 환경에 최적화된 구조적 로깅. |

---

## 2. Phased Migration Plan (단계별 전환 계획)

전환은 **Bottom-up** 방식(기능 단위 구현)과 **Top-down** 방식(서버 골격 구현)을 병행합니다.

### Phase 1: Project Setup & Server Skeleton (서버 골격 구축)
`server.py`의 라우팅 로직과 HTTP 서버 기능을 우선 구현합니다.

- [ ] **프로젝트 초기화**: `cargo new stt-engine-rs`
- [ ] **의존성 추가**: `axum`, `tokio`, `serde`, `tower-http`(CORS), `tracing`
- [ ] **HTTP 서버 구동**: `3000` 포트 바인딩 및 Hello World 응답 확인
- [ ] **CORS 설정**: Electron 프론트엔드와의 통신 허용 설정
- [ ] **API 엔드포인트 포팅** (Mock 응답 반환):
    - `POST /upload`: 파일 업로드 핸들링 (`multipart` 처리 학습 필요)
    - `GET /history`: 로그 조회
    - `POST /transcribe`: (Mock) 작업 요청 수신
- [ ] **데이터 구조체 정의**: Python `dict`를 대체할 Request/Response `struct` 정의 (`derive(Serialize, Deserialize)` 활용)

### Phase 2: System Integration & Audio Processing (시스템 통합)
파일 시스템 접근 및 FFmpeg 연동을 구현합니다.

- [ ] **FFmpeg Wrapper 구현**:
    - `subprocess.run`을 `std::process::Command`로 대체
    - 비동기 환경에서 외부 프로세스 종료 대기 처리 (`await`)
    - `.webm`, `.m4a` 등을 `.wav` (16kHz, mono)로 변환하는 로직 구현
- [ ] **파일 관리 모듈**:
    - 업로드 디렉터리 생성 및 파일 저장 (`tokio::fs` 활용)
    - 파일명 생성 규칙 이식 (타임스탬프 기반)

### Phase 3: AI Core - Whisper STT (음성 인식 구현)
가장 핵심적인 STT 기능을 `transcribe.py`에서 분리하여 이식합니다.

- [ ] **Model Management**:
    - Whisper 모델 파일(`ggml-*.bin`) 다운로드 및 경로 로딩 로직 구현
- [ ] **Whisper Engine 연동**:
    - `whisper-rs` 크레이트 설정 및 컴파일 (C++ 빌드 도구 필요)
    - 오디오 PCM 데이터 로딩 및 리샘플링 로직
    - 추론 실행 및 결과 텍스트 반환
- [ ] **비동기 작업 큐 구현**:
    - `server.py`의 `task_queue`를 Rust의 `mpsc::channel` 또는 `Arc<Mutex<VecDeque>>`로 구현하여 요청 순차 처리

### Phase 4: AI Core - LLM & Vector Search (심화 학습)
`llamacpp_utils.py` 및 `vector_search.py`를 전환합니다. 난이도가 가장 높습니다.

- [ ] **LLM 추론 (Summary/Correct)**:
    - `Candle` 프레임워크를 사용하여 Llama/Mistral 모델 로드 (또는 `llama_cpp_rs` 사용)
    - 프롬프트 템플릿 처리 (`format!` 매크로 활용)
    - 스트리밍 응답 구현 (Optional)
- [ ] **Vector Embedding & RAG**:
    - `sentence-transformers` 대체: `Candle`로 BERT 계열 모델 로드하여 임베딩 벡터 생성
    - 코사인 유사도 계산 로직 구현 (`ndarray` 활용)
    - 단순 벡터 저장소 구현 (메모리 내 검색 또는 `sled` 같은 임베디드 DB 고려)

### Phase 5: Build & Deploy (패키징)
Python 의존성을 완전히 제거하고 단일 바이너리로 Electron과 연결합니다.

- [ ] **Release 빌드 최적화**: `Cargo.toml` 프로필 설정 (`lto`, `codegen-units` 등)
- [ ] **Electron 연동 수정**:
    - `main.js`: Python 프로세스 스폰(`spawn`) 부분을 Rust 바이너리 경로로 변경
    - Python 가상환경(`venv`) 관련 코드 제거
- [ ] **Cross-Platform 빌드 테스트**: Windows/Mac 환경에서 빌드 및 실행 확인

---

## 3. Risks & Considerations (주의사항)

1.  **컴파일 시간**: `whisper-rs`, `candle` 등 AI 관련 크레이트는 빌드 시간이 매우 깁니다. 개발 중에는 기능별로 모듈을 나누어 테스트하는 것이 좋습니다.
2.  **모델 호환성**: Python 생태계의 모델 파일(`.pt`, `.safetensors`, `.gguf`)과 Rust 라이브러리가 지원하는 포맷이 일치하는지 반드시 확인해야 합니다.
3.  **에러 처리**: Python의 예외 처리(Try-Except)를 Rust의 `Result<T, E>` 패턴으로 변환하며, `unwrap()` 남용을 지양하고 명시적인 에러 타입을 정의(`thiserror` 활용 권장)해야 합니다.

## 4. Learning Checklist (학습 점검표)

- [ ] **Ownership**: 요청 데이터가 핸들러에서 백그라운드 워커로 이동할 때 소유권이 어떻게 이전되는가?
- [ ] **Concurrency**: `Arc`, `Mutex`를 사용하여 여러 스레드(요청) 간에 상태(설정, 큐)를 안전하게 공유하는가?
- [ ] **FFI**: C++ 라이브러리(`whisper.cpp`)와 Rust가 어떻게 데이터를 주고받는가? (Unsafe 블록의 이해)
- [ ] **Traits**: 공통된 AI 모델 동작을 Trait으로 추상화하여 구현했는가?