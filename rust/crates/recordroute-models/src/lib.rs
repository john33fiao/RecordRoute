use recordroute_core::config::AppConfig;

pub fn model_summary(config: &AppConfig) -> String {
    format!(
        "whisper={}, llama={}",
        config.whisper_model_dir.display(),
        config
            .llama_model_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<missing>".to_string())
    )
}
