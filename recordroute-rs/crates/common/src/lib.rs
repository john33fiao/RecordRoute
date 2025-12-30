pub mod config;
pub mod error;
pub mod logger;
pub mod model_manager;

// Re-export commonly used types
pub use config::AppConfig;
pub use error::RecordRouteError;
pub use model_manager::{ModelManager, WhisperModel, available_whisper_models};
pub type Result<T> = std::result::Result<T, RecordRouteError>;
