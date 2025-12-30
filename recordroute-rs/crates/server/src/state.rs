use recordroute_common::{AppConfig, Result};
use recordroute_stt::WhisperEngine;
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

    /// Workflow executor
    pub workflow: Arc<WorkflowExecutor>,
}

impl AppState {
    /// Create new application state
    pub fn new(config: AppConfig) -> Result<Self> {
        let history_path = config.db_base_path.join("upload_history.json");
        let history = HistoryManager::load(&history_path)?;
        let history = Arc::new(RwLock::new(history));

        let job_manager = Arc::new(JobManager::new());

        // Initialize Whisper engine
        let whisper = Arc::new(WhisperEngine::new(&config.whisper_model)?);

        // Initialize workflow executor
        let workflow = Arc::new(WorkflowExecutor::new(
            whisper.clone(),
            config.clone(),
            history.clone(),
            job_manager.clone(),
        ));

        Ok(Self {
            config,
            history,
            job_manager,
            whisper,
            workflow,
        })
    }
}
