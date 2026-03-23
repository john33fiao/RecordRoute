use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub db_root: PathBuf,
    pub model_root: PathBuf,
    pub whisper_model_dir: PathBuf,
    pub llama_model_path: Option<PathBuf>,
    pub embedding_model_dir: PathBuf,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self::from_env_iter(env::vars())
    }

    pub fn from_env_iter<I, K, V>(vars: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let vars = vars
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect::<std::collections::HashMap<String, String>>();

        let db_root = vars
            .get("DB_FOLDER_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("DB"));

        let model_root = vars
            .get("MODEL_ROOT_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("models"));

        let whisper_model_dir = vars
            .get("WHISPER_MODEL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| model_root.join("whisper"));

        let llama_model_path = vars
            .get("LLAMA_CPP_MODEL_PATH")
            .map(PathBuf::from)
            .or_else(|| default_first_gguf_path(&model_root.join("llama")));

        let embedding_model_dir = model_root.join("embedding");

        Self {
            db_root,
            model_root,
            whisper_model_dir,
            llama_model_path,
            embedding_model_dir,
        }
    }
}

fn default_first_gguf_path(dir: &std::path::Path) -> Option<PathBuf> {
    Some(dir.join("default.gguf"))
}

#[cfg(test)]
mod tests {
    use super::AppConfig;
    use std::path::PathBuf;

    #[test]
    fn uses_defaults_when_env_missing() {
        let config = AppConfig::from_env_iter(std::iter::empty::<(String, String)>());

        assert_eq!(config.db_root, PathBuf::from("DB"));
        assert_eq!(config.model_root, PathBuf::from("models"));
        assert_eq!(config.whisper_model_dir, PathBuf::from("models/whisper"));
        assert_eq!(
            config.embedding_model_dir,
            PathBuf::from("models/embedding")
        );
        assert_eq!(
            config.llama_model_path,
            Some(PathBuf::from("models/llama/default.gguf"))
        );
    }

    #[test]
    fn respects_compatibility_env_vars() {
        let config = AppConfig::from_env_iter([
            ("DB_FOLDER_PATH", "custom-db"),
            ("MODEL_ROOT_PATH", "custom-models"),
            ("WHISPER_MODEL_DIR", "legacy-whisper"),
            ("LLAMA_CPP_MODEL_PATH", "legacy-llama/model.gguf"),
        ]);

        assert_eq!(config.db_root, PathBuf::from("custom-db"));
        assert_eq!(config.model_root, PathBuf::from("custom-models"));
        assert_eq!(config.whisper_model_dir, PathBuf::from("legacy-whisper"));
        assert_eq!(
            config.llama_model_path,
            Some(PathBuf::from("legacy-llama/model.gguf"))
        );
        assert_eq!(
            config.embedding_model_dir,
            PathBuf::from("custom-models/embedding")
        );
    }
}
