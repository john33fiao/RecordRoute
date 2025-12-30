use crate::error::RecordRouteError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// RecordRoute application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Database base path
    pub db_base_path: PathBuf,

    /// Upload directory path
    pub upload_dir: PathBuf,

    /// Whisper model name or path
    pub whisper_model: String,

    /// Ollama API base URL
    pub ollama_base_url: String,

    /// Embedding model name
    pub embedding_model: String,

    /// LLM summarization model name
    pub llm_model: String,

    /// Server bind address
    pub server_host: String,

    /// Server port
    pub server_port: u16,

    /// Log directory
    pub log_dir: PathBuf,

    /// Log level
    pub log_level: String,

    /// Vector index file path
    pub vector_index_path: PathBuf,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            db_base_path: PathBuf::from("./db"),
            upload_dir: PathBuf::from("./db/uploads"),
            whisper_model: "base".to_string(),
            ollama_base_url: "http://localhost:11434".to_string(),
            embedding_model: "nomic-embed-text".to_string(),
            llm_model: "llama3.2:latest".to_string(),
            server_host: "0.0.0.0".to_string(),
            server_port: 8080,
            log_dir: PathBuf::from("./db/log"),
            log_level: "info".to_string(),
            vector_index_path: PathBuf::from("./db/vector_index.json"),
        }
    }
}

impl AppConfig {
    /// Load configuration from environment variables and .env file
    pub fn from_env() -> Result<Self, RecordRouteError> {
        // Load .env file (ignore if not exists)
        let _ = dotenv::dotenv();

        let config = Self {
            db_base_path: Self::get_env_path("DB_BASE_PATH")
                .unwrap_or_else(|| PathBuf::from("./db")),
            upload_dir: Self::get_env_path("UPLOAD_DIR")
                .unwrap_or_else(|| PathBuf::from("./db/uploads")),
            whisper_model: std::env::var("WHISPER_MODEL")
                .unwrap_or_else(|_| "base".to_string()),
            ollama_base_url: std::env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            embedding_model: std::env::var("EMBEDDING_MODEL")
                .unwrap_or_else(|_| "nomic-embed-text".to_string()),
            llm_model: std::env::var("LLM_MODEL")
                .unwrap_or_else(|_| "llama3.2:latest".to_string()),
            server_host: std::env::var("SERVER_HOST")
                .unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: std::env::var("SERVER_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8080),
            log_dir: Self::get_env_path("LOG_DIR")
                .unwrap_or_else(|| PathBuf::from("./db/log")),
            log_level: std::env::var("LOG_LEVEL")
                .unwrap_or_else(|_| "info".to_string()),
            vector_index_path: Self::get_env_path("VECTOR_INDEX_PATH")
                .unwrap_or_else(|| PathBuf::from("./db/vector_index.json")),
        };

        // Ensure required directories exist
        config.ensure_directories()?;

        Ok(config)
    }

    /// Get PathBuf from environment variable
    fn get_env_path(key: &str) -> Option<PathBuf> {
        std::env::var(key).ok().map(PathBuf::from)
    }

    /// Ensure required directories exist, create if not
    pub fn ensure_directories(&self) -> Result<(), RecordRouteError> {
        let dirs = vec![
            &self.db_base_path,
            &self.upload_dir,
            &self.log_dir,
        ];

        for dir in dirs {
            if !dir.exists() {
                std::fs::create_dir_all(dir).map_err(|e| {
                    RecordRouteError::config(format!(
                        "Failed to create directory {}: {}",
                        dir.display(),
                        e
                    ))
                })?;
            }
        }

        Ok(())
    }

    /// Get database path for specific alias
    pub fn get_db_path(&self, alias: &str) -> PathBuf {
        self.db_base_path.join(alias)
    }

    /// Get full path for uploaded file
    pub fn get_upload_path(&self, filename: &str) -> PathBuf {
        self.upload_dir.join(filename)
    }

    /// Get log file path
    pub fn get_log_path(&self, filename: &str) -> PathBuf {
        self.log_dir.join(filename)
    }

    /// Get server bind address (host:port)
    pub fn server_bind_address(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), RecordRouteError> {
        // Validate Whisper model name
        if self.whisper_model.is_empty() {
            return Err(RecordRouteError::config("Whisper model name cannot be empty"));
        }

        // Validate Ollama URL
        if !self.ollama_base_url.starts_with("http://")
            && !self.ollama_base_url.starts_with("https://") {
            return Err(RecordRouteError::config(
                "Ollama base URL must start with http:// or https://"
            ));
        }

        // Validate port range
        if self.server_port == 0 {
            return Err(RecordRouteError::config("Server port cannot be 0"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.server_port, 8080);
        assert_eq!(config.whisper_model, "base");
    }

    #[test]
    fn test_server_bind_address() {
        let config = AppConfig::default();
        assert_eq!(config.server_bind_address(), "0.0.0.0:8080");
    }

    #[test]
    fn test_validate() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());

        let mut invalid_config = AppConfig::default();
        invalid_config.whisper_model = String::new();
        assert!(invalid_config.validate().is_err());
    }
}
