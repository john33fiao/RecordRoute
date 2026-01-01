use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Upload history record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRecord {
    /// Unique identifier
    pub id: String,

    /// Original filename
    pub filename: String,

    /// File path (for download)
    #[serde(default)]
    pub file_path: String,

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
        let file_path = format!("/download/{}", id);
        Self {
            id,
            filename,
            file_path,
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

/// Update STT text request
#[derive(Debug, Deserialize)]
pub struct UpdateSttTextRequest {
    /// File identifier (UUID)
    pub file_identifier: String,

    /// New STT content
    pub content: String,
}

/// Update STT text response
#[derive(Debug, Serialize)]
pub struct UpdateSttTextResponse {
    /// Success flag
    pub success: bool,

    /// Record ID (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
}

/// Similar documents request
#[derive(Debug, Deserialize)]
pub struct SimilarDocsRequest {
    /// File path or identifier
    pub file_path: String,

    /// Refresh flag
    #[serde(default)]
    pub refresh: bool,
}

/// Similar document item
#[derive(Debug, Serialize)]
pub struct SimilarDocItem {
    /// Display name
    pub display_name: String,

    /// File path
    pub file: String,

    /// Download link
    pub link: String,

    /// Similarity score
    pub score: f32,

    /// Title summary
    pub title_summary: Option<String>,
}

/// Check existing STT request
#[derive(Debug, Deserialize)]
pub struct CheckExistingSttRequest {
    /// File path or identifier
    pub file_path: String,
}

/// Check existing STT response
#[derive(Debug, Serialize)]
pub struct CheckExistingSttResponse {
    /// Has STT flag
    pub has_stt: bool,
}

/// Reset record request
#[derive(Debug, Deserialize)]
pub struct ResetRecordRequest {
    /// Record ID
    pub record_id: String,
}

/// Reset summary/embedding request
#[derive(Debug, Deserialize)]
pub struct ResetSummaryEmbeddingRequest {
    /// Record ID
    pub record_id: String,
}

/// Update filename request
#[derive(Debug, Deserialize)]
pub struct UpdateFilenameRequest {
    /// Record ID
    pub record_id: String,

    /// New filename
    pub filename: String,
}

/// Reset all tasks request
#[derive(Debug, Deserialize)]
pub struct ResetAllTasksRequest {
    /// Task types to reset
    pub tasks: Vec<String>,
}

/// Generic success response
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    /// Success flag
    pub success: bool,

    /// Optional message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Incremental embedding response
#[derive(Debug, Serialize)]
pub struct IncrementalEmbeddingResponse {
    /// Success flag
    pub success: bool,

    /// Number of files processed
    pub processed_count: usize,

    /// Message
    pub message: String,
}

/// Cache stats response
#[derive(Debug, Serialize)]
pub struct CacheStatsResponse {
    /// Total cache entries
    pub total_entries: usize,

    /// Cache size in bytes
    pub cache_size_bytes: u64,

    /// Expired entries count
    pub expired_entries: usize,
}

/// Cache cleanup response
#[derive(Debug, Serialize)]
pub struct CacheCleanupResponse {
    /// Success flag
    pub success: bool,

    /// Number of cleaned entries
    pub cleaned_entries: usize,

    /// Message
    pub message: String,
}
