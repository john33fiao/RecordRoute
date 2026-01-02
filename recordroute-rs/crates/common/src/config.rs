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

    /// Embedding model name (for Ollama)
    pub embedding_model: String,

    /// LLM summarization model name (for Ollama)
    pub llm_model: String,

    /// Use llama.cpp directly instead of Ollama
    pub use_llamacpp: bool,

    /// Path to GGUF LLM model file (for llama.cpp)
    pub llama_model_path: Option<PathBuf>,

    /// Path to GGUF embedding model file (for llama.cpp)
    pub embedding_model_path: Option<PathBuf>,

    /// Context size for llama.cpp
    pub llama_n_ctx: u32,

    /// Number of threads for llama.cpp
    pub llama_n_threads: u32,

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
            use_llamacpp: false,  // Default to Ollama for backward compatibility
            llama_model_path: None,
            embedding_model_path: None,
            llama_n_ctx: 2048,
            llama_n_threads: 4,
            server_host: "0.0.0.0".to_string(),
            server_port: 8080,
            log_dir: PathBuf::from("./db/log"),
            log_level: "info".to_string(),
            vector_index_path: PathBuf::from("./db/vector_index.json"),
        }
    }
}

impl AppConfig {
    /// Find project root by looking for .git directory
    fn find_project_root() -> Option<PathBuf> {
        let mut current_dir = std::env::current_dir().ok()?;

        loop {
            if current_dir.join(".git").exists() {
                return Some(current_dir);
            }

            if !current_dir.pop() {
                break;
            }
        }

        None
    }

    /// Resolve path relative to project root
    fn resolve_path(path: &str, project_root: Option<&PathBuf>) -> PathBuf {
        let path_buf = PathBuf::from(path);

        // If absolute path, return as-is
        if path_buf.is_absolute() {
            return path_buf;
        }

        // If relative path and we have project root, resolve relative to root
        if let Some(root) = project_root {
            root.join(path)
        } else {
            // Fallback to current directory
            path_buf
        }
    }

    /// Load configuration from environment variables and .env file
    pub fn from_env() -> Result<Self, RecordRouteError> {
        // Try to load .env from project root (where .git directory is)
        let project_root = Self::find_project_root();

        eprintln!("[DEBUG] Current directory: {:?}", std::env::current_dir().ok());
        eprintln!("[DEBUG] Detected project root: {:?}", project_root);

        if let Some(root) = &project_root {
            let env_path = root.join(".env");
            eprintln!("[DEBUG] Looking for .env file at: {:?}", env_path);

            if env_path.exists() {
                eprintln!("[DEBUG] .env file exists, attempting to load...");
                match dotenv::from_path(&env_path) {
                    Ok(_) => eprintln!("[DEBUG] Successfully loaded .env from: {:?}", env_path),
                    Err(e) => eprintln!("[DEBUG] Failed to load .env: {:?}", e),
                }
            } else {
                eprintln!("[DEBUG] .env file does not exist at project root");
            }
        } else {
            // Fallback to default dotenv behavior (current dir and parents)
            eprintln!("[DEBUG] No project root found, using default dotenv behavior");
            match dotenv::dotenv() {
                Ok(path) => eprintln!("[DEBUG] Loaded .env from: {:?}", path),
                Err(e) => eprintln!("[DEBUG] No .env file found or failed to load: {:?}", e),
            }
        }

        // Debug log WHISPER_MODEL environment variable
        eprintln!("[DEBUG] Reading WHISPER_MODEL from environment...");
        let whisper_model_raw = std::env::var("WHISPER_MODEL");
        eprintln!("[DEBUG] WHISPER_MODEL raw value: {:?}", whisper_model_raw);

        let config = Self {
            db_base_path: Self::get_env_path("DB_BASE_PATH")
                .map(|p| Self::resolve_path(&p.to_string_lossy(), project_root.as_ref()))
                .unwrap_or_else(|| PathBuf::from("./db")),
            upload_dir: Self::get_env_path("UPLOAD_DIR")
                .map(|p| Self::resolve_path(&p.to_string_lossy(), project_root.as_ref()))
                .unwrap_or_else(|| PathBuf::from("./db/uploads")),
            whisper_model: whisper_model_raw
                .map(|p| Self::resolve_path(&p, project_root.as_ref()).to_string_lossy().to_string())
                .unwrap_or_else(|_| "base".to_string()),
            ollama_base_url: std::env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            embedding_model: std::env::var("EMBEDDING_MODEL")
                .unwrap_or_else(|_| "nomic-embed-text".to_string()),
            llm_model: std::env::var("LLM_MODEL")
                .unwrap_or_else(|_| "llama3.2:latest".to_string()),
            use_llamacpp: std::env::var("USE_LLAMACPP")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(false),
            llama_model_path: Self::get_env_path("LLAMA_MODEL_PATH")
                .map(|p| Self::resolve_path(&p.to_string_lossy(), project_root.as_ref())),
            embedding_model_path: Self::get_env_path("EMBEDDING_MODEL_PATH")
                .map(|p| Self::resolve_path(&p.to_string_lossy(), project_root.as_ref())),
            llama_n_ctx: std::env::var("LLAMA_N_CTX")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2048),
            llama_n_threads: std::env::var("LLAMA_N_THREADS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(4),
            server_host: std::env::var("SERVER_HOST")
                .unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: std::env::var("SERVER_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8080),
            log_dir: Self::get_env_path("LOG_DIR")
                .map(|p| Self::resolve_path(&p.to_string_lossy(), project_root.as_ref()))
                .unwrap_or_else(|| PathBuf::from("./db/log")),
            log_level: std::env::var("LOG_LEVEL")
                .unwrap_or_else(|_| "info".to_string()),
            vector_index_path: Self::get_env_path("VECTOR_INDEX_PATH")
                .map(|p| Self::resolve_path(&p.to_string_lossy(), project_root.as_ref()))
                .unwrap_or_else(|| PathBuf::from("./db/vector_index.json")),
        };

        // Debug log final resolved value
        eprintln!("[DEBUG] Final whisper_model value: {}", config.whisper_model);

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

        // Validate LLM configuration
        if self.use_llamacpp {
            // When using llama.cpp, model files must exist
            if let Some(model_path) = &self.llama_model_path {
                if !model_path.exists() {
                    return Err(RecordRouteError::config(
                        format!("LLM model file not found: {}", model_path.display())
                    ));
                }
            } else {
                return Err(RecordRouteError::config(
                    "LLAMA_MODEL_PATH must be set when USE_LLAMACPP=true"
                ));
            }

            // Embedding model is optional but should exist if specified
            if let Some(embed_path) = &self.embedding_model_path {
                if !embed_path.exists() {
                    return Err(RecordRouteError::config(
                        format!("Embedding model file not found: {}", embed_path.display())
                    ));
                }
            }
        } else {
            // When using Ollama, validate URL
            if !self.ollama_base_url.starts_with("http://")
                && !self.ollama_base_url.starts_with("https://") {
                return Err(RecordRouteError::config(
                    "Ollama base URL must start with http:// or https://"
                ));
            }
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
