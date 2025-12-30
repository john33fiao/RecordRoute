use std::fmt;

/// RecordRoute error types
#[derive(Debug, thiserror::Error)]
pub enum RecordRouteError {
    /// STT related error
    #[error("STT error: {0}")]
    Stt(String),

    /// LLM related error
    #[error("LLM error: {0}")]
    Llm(String),

    /// Vector search related error
    #[error("Vector search error: {0}")]
    VectorSearch(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// File system error
    #[error("File system error: {0}")]
    FileSystem(String),

    /// Network/HTTP error
    #[error("Network error: {0}")]
    Network(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// General error (anyhow integration)
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl RecordRouteError {
    /// Create STT error
    pub fn stt<S: Into<String>>(msg: S) -> Self {
        Self::Stt(msg.into())
    }

    /// Create LLM error
    pub fn llm<S: Into<String>>(msg: S) -> Self {
        Self::Llm(msg.into())
    }

    /// Create vector search error
    pub fn vector_search<S: Into<String>>(msg: S) -> Self {
        Self::VectorSearch(msg.into())
    }

    /// Create config error
    pub fn config<S: Into<String>>(msg: S) -> Self {
        Self::Config(msg.into())
    }

    /// Create file system error
    pub fn file_system<S: Into<String>>(msg: S) -> Self {
        Self::FileSystem(msg.into())
    }

    /// Create network error
    pub fn network<S: Into<String>>(msg: S) -> Self {
        Self::Network(msg.into())
    }

    /// Create invalid input error
    pub fn invalid_input<S: Into<String>>(msg: S) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create not found error
    pub fn not_found<S: Into<String>>(msg: S) -> Self {
        Self::NotFound(msg.into())
    }

    /// Create internal error
    pub fn internal<S: Into<String>>(msg: S) -> Self {
        Self::Internal(msg.into())
    }
}

// HTTP response conversion (for actix-web later)
impl RecordRouteError {
    /// Get HTTP status code
    pub fn status_code(&self) -> u16 {
        match self {
            Self::InvalidInput(_) => 400,
            Self::NotFound(_) => 404,
            Self::Config(_) => 500,
            Self::Internal(_) => 500,
            Self::Stt(_) => 500,
            Self::Llm(_) => 500,
            Self::VectorSearch(_) => 500,
            Self::FileSystem(_) => 500,
            Self::Network(_) => 503,
            Self::Serialization(_) => 500,
            Self::Io(_) => 500,
            Self::Json(_) => 400,
            Self::Other(_) => 500,
        }
    }
}
