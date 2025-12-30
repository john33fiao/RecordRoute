pub mod config;
pub mod error;
pub mod logger;

// Re-export commonly used types
pub use config::AppConfig;
pub use error::RecordRouteError;
pub type Result<T> = std::result::Result<T, RecordRouteError>;
