use recordroute_common::{AppConfig, Result};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::history::HistoryManager;
use crate::job_manager::JobManager;

/// Shared application state
pub struct AppState {
    /// Application configuration
    pub config: AppConfig,
    
    /// History manager
    pub history: Arc<RwLock<HistoryManager>>,
    
    /// Job manager
    pub job_manager: Arc<JobManager>,
}

impl AppState {
    /// Create new application state
    pub fn new(config: AppConfig) -> Result<Self> {
        let history_path = config.db_base_path.join("upload_history.json");
        let history = HistoryManager::load(&history_path)?;
        
        Ok(Self {
            config,
            history: Arc::new(RwLock::new(history)),
            job_manager: Arc::new(JobManager::new()),
        })
    }
}
