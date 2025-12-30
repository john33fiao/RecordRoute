use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Upload history record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRecord {
    /// Unique identifier
    pub id: String,
    
    /// Original filename
    pub filename: String,
    
    /// Upload timestamp
    pub timestamp: DateTime<Utc>,
    
    /// STT completed
    pub stt_done: bool,
    
    /// Summary completed
    pub summarize_done: bool,
    
    /// Embedding completed
    pub embed_done: bool,
    
    /// Path to STT result
    pub stt_path: Option<String>,
    
    /// Path to summary result
    pub summary_path: Option<String>,
    
    /// One-line summary
    pub one_line_summary: Option<String>,
    
    /// Tags
    #[serde(default)]
    pub tags: Vec<String>,
    
    /// Deleted flag
    #[serde(default)]
    pub deleted: bool,
}

impl HistoryRecord {
    /// Create new history record
    pub fn new(id: String, filename: String) -> Self {
        Self {
            id,
            filename,
            timestamp: Utc::now(),
            stt_done: false,
            summarize_done: false,
            embed_done: false,
            stt_path: None,
            summary_path: None,
            one_line_summary: None,
            tags: Vec::new(),
            deleted: false,
        }
    }
}

/// Process workflow request
#[derive(Debug, Deserialize)]
pub struct ProcessRequest {
    /// File UUID
    pub file_uuid: String,
    
    /// Run STT
    #[serde(default)]
    pub run_stt: bool,
    
    /// Run summarization
    #[serde(default)]
    pub run_summarize: bool,
    
    /// Run embedding
    #[serde(default)]
    pub run_embed: bool,
    
    /// Whisper model
    pub stt_model: Option<String>,
    
    /// Summary model
    pub summary_model: Option<String>,
}

/// Task status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Running task information
#[derive(Debug, Clone, Serialize)]
pub struct TaskInfo {
    /// Task ID
    pub task_id: String,
    
    /// Task type (stt, summary, embedding)
    pub task_type: String,
    
    /// File UUID
    pub file_uuid: String,
    
    /// Status
    pub status: TaskStatus,
    
    /// Progress percentage (0-100)
    pub progress: u8,
    
    /// Current message
    pub message: String,
    
    /// Started at
    pub started_at: DateTime<Utc>,
}

/// Delete request
#[derive(Debug, Deserialize)]
pub struct DeleteRequest {
    /// Record IDs to delete
    pub ids: Vec<String>,
}

/// Cancel task request
#[derive(Debug, Deserialize)]
pub struct CancelTaskRequest {
    /// Task ID to cancel
    pub task_id: String,
}

/// Search query
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Search query text
    pub q: String,
    
    /// Start date filter
    pub start: Option<String>,
    
    /// End date filter
    pub end: Option<String>,
    
    /// Top K results
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

fn default_top_k() -> usize {
    10
}

/// Upload response
#[derive(Debug, Serialize)]
pub struct UploadResponse {
    /// File UUID
    pub file_uuid: String,
    
    /// Original filename
    pub filename: String,
    
    /// File path
    pub path: String,
}

/// Process response
#[derive(Debug, Serialize)]
pub struct ProcessResponse {
    /// Task ID
    pub task_id: String,
    
    /// Message
    pub message: String,
}
