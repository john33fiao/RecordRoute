# RecordRoute 아키텍처 문서

RecordRoute Rust 버전의 시스템 아키텍처 및 설계 문서입니다.

## 시스템 개요

RecordRoute는 음성 파일을 AI로 처리하는 완전한 파이프라인을 제공합니다:

```
┌─────────────────────────────────────────────────────────┐
│              RecordRoute Architecture                    │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │  HTTP/WebSocket Server (Actix-web)                 │ │
│  │  - REST API                                         │ │
│  │  - File Upload/Download                             │ │
│  │  - Real-time WebSocket                              │ │
│  └────────────────────────────────────────────────────┘ │
│                        │                                 │
│                        ↓                                 │
│  ┌────────────────────────────────────────────────────┐ │
│  │  Workflow Executor (Orchestration)                 │ │
│  │  - Task Management                                  │ │
│  │  - Progress Tracking                                │ │
│  │  - Error Handling                                   │ │
│  └────────────────────────────────────────────────────┘ │
│           │              │              │                │
│           ↓              ↓              ↓                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │
│  │ STT Engine  │  │ Summarizer  │  │   Vector    │    │
│  │  (Whisper)  │  │  (Ollama)   │  │   Search    │    │
│  └─────────────┘  └─────────────┘  └─────────────┘    │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

## Cargo 워크스페이스 구조

RecordRoute는 모듈화된 Cargo 워크스페이스로 구성되어 있습니다:

```
recordroute-rs/
├── Cargo.toml                    # 워크스페이스 루트
└── crates/
    ├── common/                   # 공통 유틸리티
    │   ├── config.rs            # 설정 관리
    │   ├── error.rs             # 통합 에러 타입
    │   └── logger.rs            # 로깅 설정
    │
    ├── stt/                      # STT 엔진
    │   ├── types.rs             # STT 타입 정의
    │   ├── whisper.rs           # Whisper 엔진
    │   ├── audio.rs             # 오디오 전처리
    │   └── postprocess.rs       # 텍스트 후처리
    │
    ├── llm/                      # LLM 통합
    │   ├── types.rs             # LLM 타입
    │   ├── client.rs            # Ollama 클라이언트
    │   ├── chunking.rs          # 텍스트 청킹
    │   └── summarize.rs         # Map-Reduce 요약
    │
    ├── vector/                   # 벡터 검색
    │   ├── types.rs             # 벡터 타입
    │   ├── similarity.rs        # 유사도 계산
    │   └── engine.rs            # 검색 엔진
    │
    ├── server/                   # HTTP 서버
    │   ├── state.rs             # 애플리케이션 상태
    │   ├── workflow.rs          # 워크플로우 실행기
    │   ├── job_manager.rs       # 작업 관리
    │   ├── history.rs           # 히스토리 관리
    │   ├── websocket.rs         # WebSocket 핸들러
    │   └── routes/              # REST API 라우트
    │       ├── upload.rs
    │       ├── process.rs
    │       ├── history.rs
    │       ├── download.rs
    │       ├── tasks.rs
    │       └── search.rs
    │
    └── recordroute/              # 메인 바이너리
        └── main.rs
```

## 핵심 컴포넌트

### 1. STT Engine (crates/stt/)

**책임**:
- Whisper.cpp 모델 로딩
- 오디오 파일 전처리
- 음성 전사
- 세그먼트 후처리 및 병합

**주요 타입**:
```rust
pub struct WhisperEngine {
    ctx: Arc<WhisperContext>,
    model_path: String,
}

pub struct Transcription {
    pub text: String,
    pub segments: Vec<Segment>,
    pub language: String,
}

pub struct TranscriptionOptions {
    pub language: Option<String>,
    pub temperature: f32,
    pub filter_fillers: bool,
    // ...
}
```

**데이터 흐름**:
```
Audio File → Load/Convert → Whisper Inference → Post-process → Transcription
```

### 2. LLM Integration (crates/llm/)

**책임**:
- Ollama API 통신
- 텍스트 청킹 (긴 텍스트 처리)
- Map-Reduce 요약
- 임베딩 생성

**주요 타입**:
```rust
pub struct OllamaClient {
    base_url: String,
    client: Client,
}

pub struct Summarizer {
    client: OllamaClient,
    model: String,
}

pub struct Summary {
    pub text: String,
    pub one_line: String,
    pub model: String,
}
```

**Map-Reduce 알고리즘**:
```
Long Text
    ↓
Chunk into pieces (2000 tokens each)
    ↓
Map: Summarize each chunk in parallel
    ↓
Combine chunk summaries
    ↓
Reduce: Final summarization
    ↓
Generate one-line summary
```

### 3. Vector Search (crates/vector/)

**책임**:
- 텍스트 임베딩 생성
- 벡터 인덱스 관리
- 코사인 유사도 계산
- 의미 기반 검색

**주요 타입**:
```rust
pub struct VectorSearchEngine {
    index: Arc<RwLock<VectorIndex>>,
    index_path: PathBuf,
    embedding_dir: PathBuf,
    ollama: Arc<OllamaClient>,
    embedding_model: String,
}

pub struct VectorIndex {
    entries: HashMap<String, VectorEntry>,
    embedding_model: String,
    embedding_dim: usize,
}

pub struct SearchResult {
    pub doc_id: String,
    pub score: f32,
    pub metadata: VectorMetadata,
}
```

**검색 알고리즘**:
```
Query Text
    ↓
Generate embedding (Ollama API)
    ↓
Load all indexed embeddings
    ↓
Compute cosine similarity for each
    ↓
Sort by score (descending)
    ↓
Return top-k results
```

### 4. HTTP Server (crates/server/)

**책임**:
- REST API 제공
- WebSocket 실시간 통신
- 파일 업로드/다운로드
- 작업 관리
- 히스토리 관리

**주요 타입**:
```rust
pub struct AppState {
    pub config: AppConfig,
    pub history: Arc<RwLock<HistoryManager>>,
    pub job_manager: Arc<JobManager>,
    pub whisper: Arc<WhisperEngine>,
    pub ollama: Arc<OllamaClient>,
    pub summarizer: Arc<Summarizer>,
    pub vector_search: Arc<VectorSearchEngine>,
    pub workflow: Arc<WorkflowExecutor>,
}
```

**Actix-web 구조**:
```rust
HttpServer::new(move || {
    App::new()
        .app_data(web::Data::new(app_state.clone()))
        .wrap(middleware::Logger::default())
        .wrap(Cors::permissive())
        .service(routes::upload::upload)
        .service(routes::process::process)
        .service(routes::search::search)
        // ...
})
```

### 5. Workflow Executor (server/workflow.rs)

**책임**:
- 워크플로우 오케스트레이션
- 진행률 추적
- 에러 핸들링
- 히스토리 업데이트

**실행 흐름**:
```rust
pub async fn execute(
    file_uuid: &str,
    file_path: &Path,
    options: WorkflowOptions,
    task_id: &str,
) -> Result<WorkflowResult> {
    // Phase 1: STT
    if options.run_stt {
        let transcript = run_stt_workflow(...).await?;
        update_history(...);
        update_progress(50, "Transcription completed");
    }
    
    // Phase 2: Summarization
    if options.run_summarize {
        let summary = run_summarize_workflow(...).await?;
        update_history(...);
        update_progress(80, "Summarization completed");
    }
    
    // Phase 3: Embedding
    if options.run_embed {
        let embedding_id = run_embed_workflow(...).await?;
        update_history(...);
        update_progress(95, "Embedding completed");
    }
    
    update_progress(100, "Workflow completed");
    Ok(result)
}
```

## 데이터 저장

### 파일 시스템 구조

```
data/
├── upload_history.json           # 히스토리 DB (JSON)
├── vector_index.json             # 벡터 인덱스 (JSON)
├── embeddings/                   # 임베딩 벡터
│   ├── {uuid}.json              # [0.1, 0.2, ...]
│   └── ...
└── whisper_output/               # STT 결과
    ├── {uuid}.txt               # 전체 전사 텍스트
    ├── {uuid}_segments.json     # 타임스탬프 세그먼트
    ├── {uuid}_summary.txt       # 전체 요약
    └── {uuid}_oneline.txt       # 한 줄 요약
```

### HistoryRecord 스키마

```rust
pub struct HistoryRecord {
    pub id: String,                      // UUID
    pub filename: String,                 // 원본 파일명
    pub timestamp: DateTime<Utc>,        // 업로드 시간
    pub stt_done: bool,                  // STT 완료 여부
    pub summarize_done: bool,            // 요약 완료 여부
    pub embed_done: bool,                // 임베딩 완료 여부
    pub stt_path: Option<String>,        // 전사 파일 경로
    pub summary_path: Option<String>,    // 요약 파일 경로
    pub one_line_summary: Option<String>,// 한 줄 요약
    pub tags: Vec<String>,               // 태그
    pub deleted: bool,                   // 삭제 플래그
}
```

## 동시성 및 비동기

### Tokio 런타임

RecordRoute는 Tokio async 런타임을 사용합니다:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::from_env()?;
    logger::setup_logging(&config.log_dir, &config.log_level)?;
    recordroute_server::start_server(config).await?;
    Ok(())
}
```

### 공유 상태 관리

스레드 안전성을 위해 `Arc<RwLock<T>>` 패턴 사용:

```rust
// 읽기 잠금
let history = state.history.read().await;
let records = history.get_active_records();

// 쓰기 잠금
let mut history = state.history.write().await;
history.add_record(record)?;
```

### 백그라운드 작업

워크플로우는 백그라운드에서 실행:

```rust
tokio::spawn(async move {
    match state.workflow
        .execute(file_uuid, file_path, options, task_id)
        .await
    {
        Ok(_) => job_manager.complete_task(task_id).await,
        Err(e) => job_manager.fail_task(task_id, e).await,
    }
});
```

## 에러 처리

### 통합 에러 타입

```rust
#[derive(Debug, thiserror::Error)]
pub enum RecordRouteError {
    #[error("STT error: {0}")]
    Stt(String),
    
    #[error("LLM error: {0}")]
    Llm(String),
    
    #[error("Vector search error: {0}")]
    VectorSearch(String),
    
    #[error("File system error: {0}")]
    FileSystem(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
```

### 에러 전파

```rust
// Result<T> = Result<T, RecordRouteError>
pub async fn process_file(path: &Path) -> Result<Transcription> {
    let audio = load_audio(path)?;           // IO 에러 자동 변환
    let transcript = whisper.transcribe(audio)?; // STT 에러
    Ok(transcript)
}
```

### HTTP 에러 응답

```rust
// Actix-web 에러 변환
.map_err(|e| actix_web::error::ErrorInternalServerError(e))?
```

## 성능 고려사항

### 1. 메모리 관리

- **Arc 사용**: 불필요한 복사 방지
- **스트리밍**: 대용량 파일 청크 단위 처리
- **제로 카피**: 가능한 경우 참조 사용

### 2. 병렬 처리

- **Tokio**: 비동기 I/O
- **Rayon**: CPU 집약적 작업 병렬화 (미래 개선)
- **SIMD**: 벡터 연산 최적화 가능

### 3. 캐싱

- **모델 로딩**: WhisperEngine 한 번만 초기화
- **임베딩**: 파일로 저장하여 재사용
- **인덱스**: 메모리에 유지

## 확장성

### 수평 확장

현재 단일 서버 구조이지만, 다음과 같이 확장 가능:

```
┌──────────────┐
│ Load Balancer│
└──────┬───────┘
       │
   ┌───┴───┬────────┬────────┐
   │       │        │        │
┌──▼───┐ ┌─▼────┐ ┌─▼────┐ ┌─▼────┐
│Server│ │Server│ │Server│ │Server│
│  1   │ │  2   │ │  3   │ │  4   │
└──┬───┘ └──┬───┘ └──┬───┘ └──┬───┘
   │        │        │        │
   └────────┴────┬───┴────────┘
                 │
        ┌────────▼────────┐
        │ Shared Storage  │
        │ (NFS/S3)        │
        └─────────────────┘
```

필요한 수정:
- 공유 파일 저장소 (S3 등)
- 분산 작업 큐 (Redis)
- 중앙집중식 벡터 인덱스 (Qdrant, Milvus)

### 마이크로서비스 분리

```
┌───────────┐   ┌────────────┐   ┌──────────────┐
│   API     │──→│ STT Service│──→│LLM Service   │
│  Gateway  │   └────────────┘   └──────────────┘
└─────┬─────┘
      │
      ↓
┌──────────────┐
│Vector Service│
└──────────────┘
```

## 보안

### 입력 검증

- 파일 크기 제한
- 파일 타입 검증
- 경로 트래버설 방지

### 데이터 격리

- UUID 기반 파일명
- 사용자별 디렉토리 분리 (미래 기능)

### 로깅

- 민감 정보 마스킹
- 구조화된 로그 (tracing)

## 테스트 전략

### 단위 테스트

각 크레이트별 독립 테스트:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }
}
```

### 통합 테스트

워크플로우 전체 테스트:

```rust
#[tokio::test]
async fn test_full_workflow() {
    let config = test_config();
    let state = AppState::new(config)?;
    
    // 파일 업로드
    // 워크플로우 실행
    // 결과 검증
}
```

## 관련 문서

- [README.md](./README.md) - 프로젝트 개요
- [API.md](./API.md) - API 문서
- [CONFIGURATION.md](./CONFIGURATION.md) - 설정 가이드
