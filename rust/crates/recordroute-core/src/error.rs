use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("record path must start with DB/")]
    InvalidRecordPath,

    #[error("record path escapes DB root: {path}")]
    RecordPathEscapesDbRoot { path: String },

    #[error("file identifier must be a non-empty UUID or DB path")]
    InvalidFileIdentifier,

    #[error("task not found: {task_id}")]
    TaskNotFound { task_id: String },

    #[error("file not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
