# RecordRoute Rust 완전 전환 로드맵 🦀

## 전환 목표
**Python 코드베이스 전체를 Rust로 완전 전환**하여 최고 수준의 성능, 메모리 안전성 확보, Rust 심화 학습 달성

## 전환 전략: 완전 네이티브 Rust (Full Rewrite)

모든 Python 의존성을 제거하고 순수 Rust 스택으로 재구축. ML/AI 라이브러리도 Rust 네이티브 대안 사용.

### 아키텍처 개요
```
┌─────────────────────────────────────────────────────────┐
│              Pure Rust Stack (Zero Python)              │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │  HTTP/WebSocket Server (actix-web + tokio)         │ │
│  │  - REST API 엔드포인트                              │ │
│  │  - 파일 업로드/다운로드                             │ │
│  │  - 실시간 WebSocket 스트리밍                        │ │
│  └────────────────────────────────────────────────────┘ │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │  STT Engine (whisper.cpp Rust bindings)            │ │
│  │  - 오디오 전처리 (symphonia)                        │ │
│  │  - Whisper 모델 추론                                │ │
│  │  - 후처리 & 정제                                    │ │
│  └────────────────────────────────────────────────────┘ │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │  LLM Integration (Ollama HTTP API + llama.cpp)     │ │
│  │  - Ollama API 클라이언트 (reqwest)                  │ │
│  │  - 텍스트 청킹 & Map-Reduce 요약                    │ │
│  │  - 스트리밍 응답 처리                               │ │
│  └────────────────────────────────────────────────────┘ │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │  Embedding & Vector Search (candle + ndarray)      │ │
│  │  - 텍스트 임베딩 생성 (candle-transformers)         │ │
│  │  - 벡터 인덱싱 & 코사인 유사도 (ndarray + SIMD)     │ │
│  │  - 검색 캐싱 (TTL 기반)                             │ │
│  └────────────────────────────────────────────────────┘ │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │  Document Processing                                │ │
│  │  - PDF 텍스트 추출 (lopdf, pdf-extract)             │ │
│  │  - 오디오/비디오 변환 (FFmpeg 바인딩)               │ │
│  └────────────────────────────────────────────────────┘ │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │  Storage & Indexing                                 │ │
│  │  - 파일 시스템 관리                                 │ │
│  │  - JSON 인덱싱 (serde_json)                         │ │
│  │  - 구조화 로깅 (tracing)                            │ │
│  └────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

---

## 핵심 기술 스택 변경

| 기능 | Python | Rust 대안 | 난이도 |
|------|--------|-----------|--------|
| **STT** | openai-whisper (PyTorch) | whisper-rs (whisper.cpp 바인딩) | ⭐⭐⭐⭐ |
| **LLM 요약** | Ollama Python SDK | reqwest (HTTP 클라이언트) | ⭐⭐ |
| **임베딩** | sentence-transformers | candle-transformers + ONNX | ⭐⭐⭐⭐⭐ |
| **벡터 연산** | NumPy | ndarray + ndarray-linalg | ⭐⭐⭐ |
| **PDF 처리** | pypdf | lopdf, pdf-extract | ⭐⭐ |
| **오디오 처리** | FFmpeg (subprocess) | symphonia, ffmpeg-next | ⭐⭐⭐ |
| **웹 서버** | http.server | actix-web + actix-ws | ⭐⭐⭐ |
| **비동기** | asyncio + websockets | tokio + tokio-tungstenite | ⭐⭐⭐⭐ |

---

## Phase 1: 기반 인프라 구축 (2-3주)

### 1.1 프로젝트 초기 설정
- [ ] Cargo 워크스페이스 생성
  ```
  recordroute-rs/
  ├── Cargo.toml           # 워크스페이스 루트
  ├── crates/
  │   ├── recordroute/     # 메인 바이너리
  │   ├── stt/             # STT 엔진
  │   ├── llm/             # LLM 통합
  │   ├── vector/          # 벡터 검색
  │   ├── server/          # HTTP/WS 서버
  │   └── common/          # 공통 유틸리티
  └── models/              # 모델 파일 저장소
  ```

- [ ] 핵심 의존성 추가 (Cargo.toml)
  ```toml
  [workspace]
  members = ["crates/*"]

  [workspace.dependencies]
  # 웹 서버
  actix-web = "4"
  actix-files = "0.6"
  actix-ws = "0.2"

  # 비동기 런타임
  tokio = { version = "1", features = ["full"] }
  tokio-util = "0.7"
  futures = "0.3"

  # 직렬화
  serde = { version = "1", features = ["derive"] }
  serde_json = "1"

  # HTTP 클라이언트
  reqwest = { version = "0.11", features = ["json", "stream"] }

  # ML/AI
  candle-core = "0.3"
  candle-nn = "0.3"
  candle-transformers = "0.3"
  ndarray = "0.15"
  ndarray-linalg = "0.16"

  # STT (whisper.cpp 바인딩)
  whisper-rs = "0.10"

  # 오디오 처리
  symphonia = "0.5"

  # PDF 처리
  lopdf = "0.31"
  pdf-extract = "0.7"

  # 유틸리티
  uuid = { version = "1", features = ["v4", "serde"] }
  dotenv = "0.15"
  tracing = "0.1"
  tracing-subscriber = { version = "0.3", features = ["env-filter"] }
  anyhow = "1"
  thiserror = "1"
  chrono = { version = "0.4", features = ["serde"] }
  ```

### 1.2 공통 모듈 (crates/common/)
- [ ] 설정 시스템 (`config.rs`)
  ```rust
  pub struct AppConfig {
      pub db_base_path: PathBuf,
      pub upload_dir: PathBuf,
      pub whisper_model: String,
      pub ollama_base_url: String,
      pub embedding_model: String,
  }

  impl AppConfig {
      pub fn from_env() -> Result<Self>;
      pub fn get_db_path(&self, alias: &str) -> PathBuf;
  }
  ```

- [ ] 로깅 시스템 (`logger.rs`)
  ```rust
  pub fn setup_logging(log_dir: &Path) -> Result<()>;
  pub fn get_logger(module: &str) -> tracing::Subscriber;
  ```

- [ ] 에러 타입 정의 (`error.rs`)
  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum RecordRouteError {
      #[error("STT error: {0}")]
      Stt(String),
      #[error("LLM error: {0}")]
      Llm(String),
      #[error("Vector search error: {0}")]
      VectorSearch(String),
      #[error("IO error: {0}")]
      Io(#[from] std::io::Error),
  }
  ```

---

## Phase 2: STT 엔진 (Whisper.cpp) (4-5주) ⭐ 최고 난이도

### 2.1 Whisper.cpp 통합 (crates/stt/)
- [ ] whisper-rs 크레이트 설정
  - Whisper.cpp 빌드 설정
  - CUDA/Metal 가속 옵션 (선택사항)
  - 모델 파일 다운로드 스크립트

- [ ] Whisper 래퍼 구현 (`whisper.rs`)
  ```rust
  pub struct WhisperEngine {
      ctx: whisper_rs::WhisperContext,
      model_path: PathBuf,
  }

  impl WhisperEngine {
      pub fn new(model_path: PathBuf) -> Result<Self>;

      pub async fn transcribe(
          &self,
          audio_path: &Path,
          language: Option<&str>,
      ) -> Result<Transcription>;

      pub fn transcribe_with_progress<F>(
          &self,
          audio_path: &Path,
          progress_callback: F,
      ) -> Result<Transcription>
      where
          F: Fn(f32) + Send + 'static;
  }

  pub struct Transcription {
      pub text: String,
      pub segments: Vec<Segment>,
      pub language: String,
  }
  ```

### 2.2 오디오 전처리 (`audio.rs`)
- [ ] 오디오 파일 로딩 (symphonia)
  ```rust
  pub fn load_audio(path: &Path) -> Result<AudioBuffer>;
  pub fn resample_to_16khz(audio: &AudioBuffer) -> Result<AudioBuffer>;
  pub fn convert_to_mono(audio: &AudioBuffer) -> Result<AudioBuffer>;
  ```

- [ ] FFmpeg 통합 (비디오 → 오디오 추출)
  ```rust
  pub async fn extract_audio_from_video(
      video_path: &Path,
      output_path: &Path,
  ) -> Result<()>;
  ```

### 2.3 후처리 (`postprocess.rs`)
- [ ] 텍스트 정제 (transcribe.py의 로직 포팅)
  ```rust
  pub fn remove_word_repetitions(text: &str) -> String;
  pub fn remove_discard_phrases(text: &str) -> String;
  pub fn normalize_whitespace(text: &str) -> String;
  ```

- [ ] 원자적 파일 쓰기
  ```rust
  pub fn write_transcript_atomic(
      path: &Path,
      transcript: &Transcription,
  ) -> Result<()>;
  ```

### 2.4 병렬 처리
- [ ] 다중 파일 동시 처리
  ```rust
  pub async fn transcribe_batch(
      files: &[PathBuf],
      model: &WhisperEngine,
      max_parallel: usize,
  ) -> Vec<Result<Transcription>>;
  ```

**학습 포인트**:
- FFI (Foreign Function Interface) 사용
- 오디오 신호 처리 기초
- SIMD 최적화 (선택사항)
- GPU 가속 (CUDA/Metal)

---

## Phase 3: LLM 통합 (Ollama API) (2-3주)

### 3.1 Ollama HTTP 클라이언트 (crates/llm/)
- [ ] API 클라이언트 구현 (`ollama.rs`)
  ```rust
  pub struct OllamaClient {
      base_url: String,
      client: reqwest::Client,
  }

  impl OllamaClient {
      pub fn new(base_url: String) -> Self;

      pub async fn generate(
          &self,
          model: &str,
          prompt: &str,
          temperature: f32,
      ) -> Result<String>;

      pub async fn generate_stream(
          &self,
          model: &str,
          prompt: &str,
      ) -> Result<impl Stream<Item = Result<String>>>;

      pub async fn embed(
          &self,
          model: &str,
          text: &str,
      ) -> Result<Vec<f32>>;

      pub async fn list_models(&self) -> Result<Vec<ModelInfo>>;
  }
  ```

- [ ] 에러 핸들링 및 재시도 로직
  ```rust
  pub async fn generate_with_retry(
      client: &OllamaClient,
      model: &str,
      prompt: &str,
      max_retries: u32,
  ) -> Result<String>;
  ```

### 3.2 텍스트 요약 (summarize.py 포팅)
- [ ] Map-Reduce 요약 구현 (`summarize.rs`)
  ```rust
  pub struct Summarizer {
      client: OllamaClient,
      model: String,
      chunk_size: usize,
      temperature: f32,
  }

  impl Summarizer {
      pub async fn summarize_mapreduce(
          &self,
          text: &str,
      ) -> Result<String>;

      async fn chunk_text(&self, text: &str) -> Vec<String>;

      async fn summarize_chunk(&self, chunk: &str) -> Result<String>;

      async fn merge_summaries(&self, summaries: Vec<String>) -> Result<String>;
  }
  ```

- [ ] 프롬프트 템플릿 관리
  ```rust
  pub const SUMMARY_PROMPT_TEMPLATE: &str = r#"
  다음 텍스트를 회의록 형식으로 요약해주세요...
  "#;

  pub fn format_prompt(template: &str, text: &str) -> String;
  ```

### 3.3 한 줄 요약 (`one_line_summary.rs`)
- [ ] 간단한 요약 생성
  ```rust
  pub async fn generate_one_line_summary(
      client: &OllamaClient,
      text: &str,
  ) -> Result<String>;
  ```

**학습 포인트**:
- HTTP 클라이언트 구현 (reqwest)
- 스트리밍 응답 처리
- 비동기 에러 핸들링

---

## Phase 4: 임베딩 & 벡터 검색 (4-6주) ⭐⭐⭐ 고난이도

### 4.1 텍스트 임베딩 (crates/vector/)

#### 옵션 A: Ollama API 사용 (간단, 추천)
- [ ] Ollama 임베딩 엔드포인트 호출
  ```rust
  pub async fn embed_text_ollama(
      client: &OllamaClient,
      text: &str,
      model: &str,
  ) -> Result<Vec<f32>>;
  ```

#### 옵션 B: Candle로 로컬 임베딩 (도전적)
- [ ] Candle-transformers 설정
  ```rust
  use candle_core::{Device, Tensor};
  use candle_transformers::models::bert::{BertModel, Config};

  pub struct EmbeddingModel {
      model: BertModel,
      tokenizer: tokenizers::Tokenizer,
      device: Device,
  }

  impl EmbeddingModel {
      pub fn load(model_path: &Path) -> Result<Self>;

      pub fn embed(&self, text: &str) -> Result<Vec<f32>>;

      pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
  }
  ```

- [ ] ONNX 런타임 통합 (대안)
  ```rust
  use ort::{Environment, SessionBuilder, Value};

  pub struct OnnxEmbedder {
      session: ort::Session,
  }
  ```

### 4.2 벡터 검색 (`search.rs`)
- [ ] 인덱스 구조 정의
  ```rust
  #[derive(Serialize, Deserialize)]
  pub struct VectorIndex {
      entries: HashMap<String, VectorEntry>,
  }

  #[derive(Serialize, Deserialize)]
  pub struct VectorEntry {
      vector_path: PathBuf,
      timestamp: DateTime<Utc>,
      metadata: HashMap<String, String>,
      deleted: bool,
  }
  ```

- [ ] 코사인 유사도 계산 (ndarray + SIMD)
  ```rust
  use ndarray::{Array1, ArrayView1};

  pub fn cosine_similarity(a: ArrayView1<f32>, b: ArrayView1<f32>) -> f32 {
      let dot = a.dot(&b);
      let norm_a = a.dot(&a).sqrt();
      let norm_b = b.dot(&b).sqrt();
      dot / (norm_a * norm_b)
  }

  // SIMD 최적화 버전
  #[cfg(target_feature = "avx2")]
  pub fn cosine_similarity_simd(a: &[f32], b: &[f32]) -> f32;
  ```

- [ ] 검색 함수 구현
  ```rust
  pub struct VectorSearchEngine {
      index: Arc<RwLock<VectorIndex>>,
      index_path: PathBuf,
  }

  impl VectorSearchEngine {
      pub async fn search(
          &self,
          query: &str,
          top_k: usize,
          date_filter: Option<DateRange>,
      ) -> Result<Vec<SearchResult>>;

      pub async fn find_similar(
          &self,
          document_id: &str,
          top_k: usize,
      ) -> Result<Vec<SearchResult>>;

      pub async fn add_document(
          &self,
          path: &Path,
          embedding: Vec<f32>,
      ) -> Result<()>;
  }

  pub struct SearchResult {
      pub file_path: String,
      pub score: f32,
      pub metadata: HashMap<String, String>,
  }
  ```

### 4.3 검색 캐싱 (`cache.rs`)
- [ ] TTL 기반 캐시
  ```rust
  pub struct SearchCache {
      cache: Arc<Mutex<HashMap<CacheKey, CacheEntry>>>,
      ttl: Duration,
  }

  #[derive(Hash, Eq, PartialEq)]
  struct CacheKey {
      query_hash: u64,
      top_k: usize,
      filters: String,
  }

  struct CacheEntry {
      results: Vec<SearchResult>,
      created_at: Instant,
  }

  impl SearchCache {
      pub fn get(&self, key: &CacheKey) -> Option<Vec<SearchResult>>;
      pub fn insert(&self, key: CacheKey, results: Vec<SearchResult>);
      pub fn cleanup_expired(&self);
  }
  ```

- [ ] 백그라운드 정리 태스크
  ```rust
  pub async fn start_cache_cleanup_task(
      cache: Arc<SearchCache>,
      interval: Duration,
  );
  ```

**학습 포인트**:
- ML 모델 추론 (Candle/ONNX)
- 고성능 벡터 연산 (SIMD)
- 동시성 안전 인덱싱

---

## Phase 5: HTTP/WebSocket 서버 (3-4주)

### 5.1 서버 구조 설계 (crates/server/)
- [ ] Actix-web 애플리케이션 설정 (`main.rs`)
  ```rust
  use actix_web::{web, App, HttpServer};
  use actix_files as fs;

  #[actix_web::main]
  async fn main() -> std::io::Result<()> {
      let config = AppConfig::from_env()?;

      // 공유 상태
      let app_state = web::Data::new(AppState {
          config,
          whisper: WhisperEngine::new(...)?,
          ollama: OllamaClient::new(...),
          vector_search: VectorSearchEngine::new(...)?,
          job_manager: JobManager::new(),
      });

      HttpServer::new(move || {
          App::new()
              .app_data(app_state.clone())
              .wrap(tracing_actix_web::TracingLogger::default())
              .wrap(actix_cors::Cors::permissive())
              .service(routes::upload)
              .service(routes::process)
              .service(routes::history)
              .service(routes::download)
              .service(routes::search)
              .service(routes::websocket)
              .service(fs::Files::new("/", "frontend").index_file("upload.html"))
      })
      .bind(("0.0.0.0", 8080))?
      .run()
      .await
  }
  ```

### 5.2 REST API 라우트 (`routes.rs`)
- [ ] `POST /upload` - 파일 업로드
  ```rust
  #[post("/upload")]
  async fn upload(
      mut payload: Multipart,
      state: web::Data<AppState>,
  ) -> Result<HttpResponse, Error>;
  ```

- [ ] `POST /process` - 워크플로우 실행
  ```rust
  #[derive(Deserialize)]
  struct ProcessRequest {
      file_uuid: String,
      run_stt: bool,
      run_summarize: bool,
      run_embed: bool,
  }

  #[post("/process")]
  async fn process(
      req: web::Json<ProcessRequest>,
      state: web::Data<AppState>,
  ) -> Result<HttpResponse, Error>;
  ```

- [ ] `GET /history` - 작업 기록
  ```rust
  #[get("/history")]
  async fn history(state: web::Data<AppState>) -> Result<HttpResponse, Error>;
  ```

- [ ] `GET /search` - 검색
  ```rust
  #[derive(Deserialize)]
  struct SearchQuery {
      q: String,
      top_k: Option<usize>,
      start_date: Option<String>,
      end_date: Option<String>,
  }

  #[get("/search")]
  async fn search(
      query: web::Query<SearchQuery>,
      state: web::Data<AppState>,
  ) -> Result<HttpResponse, Error>;
  ```

### 5.3 WebSocket 핸들러 (`websocket.rs`)
- [ ] WebSocket 연결 관리
  ```rust
  #[get("/ws")]
  async fn websocket(
      req: HttpRequest,
      stream: web::Payload,
      state: web::Data<AppState>,
  ) -> Result<HttpResponse, Error> {
      ws::start(WsSession::new(state), &req, stream)
  }

  struct WsSession {
      id: Uuid,
      state: web::Data<AppState>,
  }

  impl Actor for WsSession {
      type Context = ws::WebsocketContext<Self>;
  }

  impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
      fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context);
  }
  ```

- [ ] 진행률 브로드캐스트
  ```rust
  pub struct ProgressBroadcaster {
      sessions: Arc<Mutex<HashMap<Uuid, Addr<WsSession>>>>,
  }

  impl ProgressBroadcaster {
      pub fn send_progress(&self, job_id: &str, progress: f32, message: &str);
      pub fn send_complete(&self, job_id: &str, result: JobResult);
      pub fn send_error(&self, job_id: &str, error: &str);
  }
  ```

### 5.4 작업 관리 (`job_manager.rs`)
- [ ] 비동기 작업 스케줄러
  ```rust
  pub struct JobManager {
      jobs: Arc<Mutex<HashMap<String, JobHandle>>>,
      broadcaster: Arc<ProgressBroadcaster>,
  }

  pub struct JobHandle {
      id: String,
      status: JobStatus,
      cancel_token: CancellationToken,
      handle: JoinHandle<Result<JobResult>>,
  }

  impl JobManager {
      pub async fn start_job(
          &self,
          job_id: String,
          task: impl Future<Output = Result<JobResult>> + Send + 'static,
      ) -> Result<()>;

      pub async fn cancel_job(&self, job_id: &str) -> Result<()>;

      pub fn get_status(&self, job_id: &str) -> Option<JobStatus>;
  }
  ```

**학습 포인트**:
- Actix-web 프레임워크
- WebSocket 양방향 통신
- 비동기 작업 관리 (tokio)
- Actor 모델

---

## Phase 6: 워크플로우 통합 (2-3주)

### 6.1 파이프라인 오케스트레이션 (`workflow.rs`)
- [ ] 전체 워크플로우 구현
  ```rust
  pub struct WorkflowExecutor {
      whisper: Arc<WhisperEngine>,
      summarizer: Arc<Summarizer>,
      vector_search: Arc<VectorSearchEngine>,
      ollama: Arc<OllamaClient>,
  }

  impl WorkflowExecutor {
      pub async fn execute(
          &self,
          file_path: &Path,
          options: WorkflowOptions,
          progress: impl Fn(WorkflowStep, f32) + Send + 'static,
      ) -> Result<WorkflowResult>;
  }

  pub struct WorkflowOptions {
      pub run_stt: bool,
      pub run_summarize: bool,
      pub run_embed: bool,
      pub stt_model: String,
      pub summary_model: String,
  }

  pub enum WorkflowStep {
      AudioExtraction,
      Transcription,
      Summarization,
      Embedding,
  }

  pub struct WorkflowResult {
      pub transcript_path: Option<PathBuf>,
      pub summary_path: Option<PathBuf>,
      pub embedding_id: Option<String>,
  }
  ```

- [ ] 단계별 진행률 추적
  ```rust
  async fn execute_with_progress<F>(
      &self,
      file_path: &Path,
      options: WorkflowOptions,
      mut progress_fn: F,
  ) -> Result<WorkflowResult>
  where
      F: FnMut(WorkflowStep, f32) + Send,
  {
      if options.run_stt {
          progress_fn(WorkflowStep::Transcription, 0.0);
          let transcript = self.whisper.transcribe_with_progress(
              file_path,
              |p| progress_fn(WorkflowStep::Transcription, p),
          ).await?;
          // ...
      }
      // ...
  }
  ```

### 6.2 히스토리 관리 (`history.rs`)
- [ ] 히스토리 레코드 CRUD
  ```rust
  #[derive(Serialize, Deserialize)]
  pub struct HistoryRecord {
      pub uuid: String,
      pub filename: String,
      pub uploaded_at: DateTime<Utc>,
      pub stt_done: bool,
      pub summarize_done: bool,
      pub embed_done: bool,
      pub stt_path: Option<String>,
      pub summary_path: Option<String>,
  }

  pub struct HistoryManager {
      file_path: PathBuf,
      records: Arc<RwLock<Vec<HistoryRecord>>>,
  }

  impl HistoryManager {
      pub fn load(path: &Path) -> Result<Self>;
      pub fn add_record(&self, record: HistoryRecord) -> Result<()>;
      pub fn update_record(&self, uuid: &str, updates: RecordUpdate) -> Result<()>;
      pub fn delete_records(&self, uuids: &[String]) -> Result<()>;
      pub fn get_all(&self) -> Vec<HistoryRecord>;
      pub fn save(&self) -> Result<()>;
  }
  ```

### 6.3 PDF 처리 (`pdf.rs`)
- [ ] PDF 텍스트 추출
  ```rust
  pub fn extract_text_from_pdf(path: &Path) -> Result<String> {
      use lopdf::Document;
      let doc = Document::load(path)?;
      // 텍스트 추출 로직
  }
  ```

---

## Phase 7: 테스트 & 최적화 (지속적)

### 7.1 단위 테스트
- [ ] 각 크레이트별 테스트 작성
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_cosine_similarity() {
          let a = array![1.0, 0.0, 0.0];
          let b = array![1.0, 0.0, 0.0];
          assert_eq!(cosine_similarity(a.view(), b.view()), 1.0);
      }

      #[tokio::test]
      async fn test_ollama_client() {
          // 통합 테스트
      }
  }
  ```

- [ ] 테스트 커버리지 > 80% 달성
  ```bash
  cargo tarpaulin --out Html --output-dir coverage
  ```

### 7.2 통합 테스트
- [ ] E2E 테스트 (tests/ 디렉토리)
  ```rust
  #[tokio::test]
  async fn test_full_workflow() {
      // 서버 시작
      // 파일 업로드
      // 워크플로우 실행
      // 결과 검증
  }
  ```

### 7.3 벤치마크
- [ ] Criterion.rs 벤치마크 작성
  ```rust
  use criterion::{black_box, criterion_group, criterion_main, Criterion};

  fn bench_vector_search(c: &mut Criterion) {
      c.bench_function("search top 10", |b| {
          b.iter(|| {
              // 검색 벤치마크
          });
      });
  }

  criterion_group!(benches, bench_vector_search);
  criterion_main!(benches);
  ```

- [ ] Python vs Rust 성능 비교 리포트

### 7.4 프로파일링
- [ ] Flamegraph 생성
  ```bash
  cargo flamegraph --bin recordroute
  ```

- [ ] 메모리 프로파일링
  ```bash
  cargo valgrind --bin recordroute
  ```

---

## Phase 8: 배포 & 패키징 (2-3주)

### 8.1 크로스 플랫폼 빌드
- [ ] GitHub Actions CI/CD 설정
  ```yaml
  name: Build
  on: [push, pull_request]
  jobs:
    build:
      strategy:
        matrix:
          os: [ubuntu-latest, macos-latest, windows-latest]
      runs-on: ${{ matrix.os }}
      steps:
        - uses: actions/checkout@v3
        - uses: actions-rs/toolchain@v1
        - run: cargo build --release
  ```

- [ ] 릴리스 최적화
  ```toml
  [profile.release]
  opt-level = 3
  lto = "fat"
  codegen-units = 1
  strip = true
  panic = "abort"
  ```

### 8.2 모델 번들링
- [ ] Whisper 모델 자동 다운로드 스크립트
  ```rust
  pub async fn download_whisper_model(
      model_name: &str,
      target_dir: &Path,
  ) -> Result<PathBuf>;
  ```

- [ ] 모델 검증 (체크섬)

### 8.3 Electron 통합
- [ ] Rust 바이너리를 Electron에 번들
  ```javascript
  // electron/main.js
  const { spawn } = require('child_process');
  const backend = spawn('./bin/recordroute', {
      env: { ...process.env, RUST_LOG: 'info' }
  });
  ```

### 8.4 배포 패키지 생성
- [ ] Linux: AppImage, .deb
- [ ] macOS: .dmg, .app
- [ ] Windows: .msi, .exe

---

## 임시 마이그레이션 도구 (선택사항)

Python 코드를 점진적으로 전환하는 동안 임시로 PyO3 사용 가능:

```rust
use pyo3::prelude::*;

#[pyfunction]
fn transcribe_fallback(path: &str) -> PyResult<String> {
    Python::with_gil(|py| {
        let whisper = py.import("whisper")?;
        let result = whisper.call_method1("transcribe", (path,))?;
        result.extract()
    })
}
```

**전환 우선순위**:
1. 서버 & 인프라 → Rust (쉬움)
2. 벡터 검색 → Rust (중간)
3. LLM 통합 → Rust (쉬움)
4. STT → Rust (어려움) ← 마지막에 전환
5. 임베딩 → Rust (매우 어려움) ← Ollama API 사용 권장

---

## 학습 체크포인트 🎓

### Rust 핵심 개념
- [ ] 소유권 & 차용 (Ownership & Borrowing)
- [ ] 라이프타임 (Lifetimes)
- [ ] 트레잇 & 제네릭 (Traits & Generics)
- [ ] 스마트 포인터 (`Arc`, `Mutex`, `RwLock`)
- [ ] 에러 핸들링 (`Result`, `Option`, `anyhow`, `thiserror`)
- [ ] 비동기 프로그래밍 (`async`/`await`, Futures)
- [ ] 패턴 매칭 & 열거형
- [ ] 매크로 (선언적, 절차적)

### 고급 Rust
- [ ] FFI (Foreign Function Interface)
- [ ] Unsafe Rust (필요 시)
- [ ] SIMD & 병렬화
- [ ] 메모리 레이아웃 최적화
- [ ] 제로 코스트 추상화

### 생태계 라이브러리
- [ ] **Tokio**: 비동기 런타임
- [ ] **Actix-web**: 웹 프레임워크
- [ ] **Serde**: 직렬화/역직렬화
- [ ] **Reqwest**: HTTP 클라이언트
- [ ] **ndarray**: 과학 계산
- [ ] **Candle**: ML 프레임워크
- [ ] **whisper-rs**: STT 바인딩
- [ ] **Tracing**: 구조화 로깅

---

## 예상 타임라인 (풀타임 기준)

| Phase | 기간 | 누적 기간 | 난이도 |
|-------|------|-----------|--------|
| Phase 1: 기반 인프라 | 2-3주 | 3주 | ⭐⭐ |
| Phase 2: STT 엔진 | 4-5주 | 8주 | ⭐⭐⭐⭐⭐ |
| Phase 3: LLM 통합 | 2-3주 | 11주 | ⭐⭐ |
| Phase 4: 벡터 검색 | 4-6주 | 17주 | ⭐⭐⭐⭐ |
| Phase 5: 웹 서버 | 3-4주 | 21주 | ⭐⭐⭐ |
| Phase 6: 워크플로우 | 2-3주 | 24주 | ⭐⭐ |
| Phase 7: 테스트 & 최적화 | 지속적 | - | ⭐⭐⭐ |
| Phase 8: 배포 | 2-3주 | 27주 | ⭐⭐ |

**총 예상 기간**:
- 풀타임: 6-7개월
- 파트타임: 10-12개월

---

## 성공 지표

### 성능 목표 (Python 대비)
- [x] HTTP 요청 처리: **5-10배 빠름**
- [x] 벡터 검색: **10-20배 빠름**
- [x] 메모리 사용량: **50-70% 감소**
- [x] STT 처리: **1.5-3배 빠름** (GPU 가속 시)
- [x] 바이너리 크기: **< 100MB** (단일 실행파일)

### 품질 목표
- [x] 테스트 커버리지 > 80%
- [x] Clippy 경고 0개
- [x] 메모리 누수 없음
- [x] 동시성 안전성 검증 (Miri, ThreadSanitizer)
- [x] 모든 기능 정상 작동 (Python 버전과 동일)

---

## 리스크 및 대응 방안

### 리스크 1: Whisper.cpp 통합 복잡도 ⚠️
- **영향**: 매우 높음 (핵심 기능)
- **대응**:
  1. 초기에 Python fallback 유지 (PyO3)
  2. 간단한 모델부터 테스트 (tiny, base)
  3. whisper-rs 대신 whisper.cpp HTTP 서버 사용 고려
- **플랜 B**: whisper.cpp를 별도 서비스로 실행하고 HTTP API로 통신

### 리스크 2: 임베딩 모델 로컬 실행 어려움 ⚠️
- **영향**: 중간 (Ollama API로 대체 가능)
- **대응**:
  1. Ollama API 우선 사용
  2. Candle은 선택사항으로 진행
  3. ONNX 런타임 고려
- **플랜 B**: 임베딩은 Ollama API만 사용 (성능 목표 조정)

### 리스크 3: 개발 시간 초과 ⏰
- **대응**:
  1. MVP 정의: Phase 1-3-5만 완료 (STT는 PyO3)
  2. 우선순위 조정: 임베딩은 Ollama API 사용
  3. Phase 2를 후순위로 미루고 다른 Phase 먼저 완료
- **조정**: 6개월 → 9개월로 연장

### 리스크 4: Rust 학습 곡선 📚
- **대응**:
  1. Rust Book 먼저 완독 (2-3주)
  2. 간단한 프로젝트로 연습 (CLI 도구 등)
  3. 커뮤니티 활용 (Discord, Reddit)
- **리소스**:
  - [Rust Book](https://doc.rust-lang.org/book/)
  - [Rustlings](https://github.com/rust-lang/rustlings)
  - [Exercism Rust Track](https://exercism.org/tracks/rust)

---

## 대안 전략: 하이브리드 MVP

완전 전환이 너무 부담스러우면 다음 하이브리드 전략 고려:

```
┌─────────────────────┐
│   Rust Core (MVP)   │
│  - HTTP/WS 서버     │
│  - 벡터 검색        │
│  - 파일 I/O         │
└──────────┬──────────┘
           │ PyO3
           ▼
┌─────────────────────┐
│  Python (임시)      │
│  - Whisper STT      │
│  - Ollama (HTTP)    │
└─────────────────────┘
```

**MVP 목표**: 3-4개월 내 작동하는 버전
**완전 전환**: 이후 6-9개월 추가

---

## 다음 스텝

### 즉시 시작 가능한 작업
1. **Rust 학습** (아직 익숙하지 않다면)
   - Rust Book 1-10장 읽기
   - Rustlings 완료
   - 간단한 HTTP 서버 만들어보기

2. **환경 설정**
   ```bash
   # Rust 설치
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # 필수 도구 설치
   cargo install cargo-watch cargo-edit cargo-tarpaulin

   # Whisper.cpp 빌드 테스트
   git clone https://github.com/ggerganov/whisper.cpp
   cd whisper.cpp && make
   ```

3. **프로젝트 초기화**
   ```bash
   cargo new --bin recordroute-rs
   cd recordroute-rs
   cargo add actix-web tokio serde

   # 간단한 "Hello World" 서버 작성
   ```

4. **Phase 1.1 시작**: 프로젝트 구조 설계 및 Cargo 워크스페이스 생성

---

## 참고 자료 📚

### 공식 문서
- [Rust Book](https://doc.rust-lang.org/book/)
- [Async Book](https://rust-lang.github.io/async-book/)
- [Actix-web](https://actix.rs/docs/)
- [Tokio](https://tokio.rs/tokio/tutorial)
- [Candle](https://github.com/huggingface/candle)
- [whisper-rs](https://github.com/tazz4843/whisper-rs)

### 유사 프로젝트 (영감)
- [Qdrant](https://github.com/qdrant/qdrant) - 벡터 검색 엔진 (Rust)
- [Tantivy](https://github.com/quickwit-oss/tantivy) - 풀텍스트 검색 (Rust)
- [Ruff](https://github.com/astral-sh/ruff) - Python 린터 (Rust 10-100배 빠름)
- [uv](https://github.com/astral-sh/uv) - Python 패키지 관리자 (Rust)

### 블로그 & 튜토리얼
- ["Rewriting Python in Rust"](https://www.lpalmieri.com/posts/2019-12-01-taking-ml-to-production-with-rust-a-25x-speedup/)
- ["Building ML Systems in Rust"](https://www.arewelearningyet.com/)
- ["Actix-web Full Tutorial"](https://actix.rs/docs/getting-started/)

### 커뮤니티
- [r/rust](https://reddit.com/r/rust)
- [Rust Discord](https://discord.gg/rust-lang)
- [This Week in Rust](https://this-week-in-rust.org/)

---

## 마치며

**완전 전환은 도전적이지만 매우 보람찬 여정**입니다.

핵심 포인트:
- ✅ **MVP 먼저**: Phase 1, 3, 5만 완료해도 큰 성과
- ✅ **점진적 접근**: PyO3로 시작해서 하나씩 전환
- ✅ **커뮤니티 활용**: 막히면 질문하세요
- ✅ **완벽보다 진행**: 작동하는 코드가 완벽한 코드보다 낫습니다

**시작할 준비 되셨나요?** 🦀🚀

어떤 Phase부터 시작하고 싶으신가요?
