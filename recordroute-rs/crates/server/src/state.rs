use recordroute_common::{AppConfig, ModelManager, Result};
use recordroute_llm::{OllamaClient, Summarizer};
use recordroute_stt::WhisperEngine;
use recordroute_vector::VectorSearchEngine;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::history::HistoryManager;
use crate::job_manager::JobManager;
use crate::workflow::WorkflowExecutor;

/// Shared application state
pub struct AppState {
    /// Application configuration
    pub config: AppConfig,

    /// History manager
    pub history: Arc<RwLock<HistoryManager>>,

    /// Job manager
    pub job_manager: Arc<JobManager>,

    /// Whisper STT engine
    pub whisper: Arc<WhisperEngine>,

    /// Ollama client
    pub ollama: Arc<OllamaClient>,

    /// Summarizer
    pub summarizer: Arc<Summarizer>,

    /// Vector search engine
    pub vector_search: Arc<VectorSearchEngine>,

    /// Workflow executor
    pub workflow: Arc<WorkflowExecutor>,
}

impl AppState {
    /// Create new application state
    pub async fn new(config: AppConfig) -> Result<Self> {
        let history_path = config.db_base_path.join("upload_history.json");
        let history = HistoryManager::load(&history_path)?;
        let history = Arc::new(RwLock::new(history));

        let job_manager = Arc::new(JobManager::new());

        // Initialize Model Manager and ensure Whisper model is available
        let models_dir = ModelManager::default_models_dir();
        let model_manager = ModelManager::new(models_dir)?;
        let model_path = model_manager.ensure_whisper_model(&config.whisper_model).await?;

        // Initialize Whisper engine with downloaded model
        let whisper = Arc::new(WhisperEngine::new(&model_path)?);

        // Initialize Ollama client and summarizer
        let ollama = Arc::new(OllamaClient::new(config.ollama_base_url.clone())?);
        let summarizer = Arc::new(Summarizer::new(
            (*ollama).clone(),
            config.llm_model.clone(),
        ));

        // Initialize vector search engine
        let vector_search = Arc::new(VectorSearchEngine::new(&config, ollama.clone())?);

        // Initialize workflow executor
        let workflow = Arc::new(WorkflowExecutor::new(
            whisper.clone(),
            summarizer.clone(),
            vector_search.clone(),
            config.clone(),
            history.clone(),
            job_manager.clone(),
        ));

        Ok(Self {
            config,
            history,
            job_manager,
            whisper,
            ollama,
            summarizer,
            vector_search,
            workflow,
        })
    }
}
