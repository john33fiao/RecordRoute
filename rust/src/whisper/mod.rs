mod discovery;
mod download;
mod output;
mod runtime;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

pub const MODEL_ENV_VAR: &str = "RECORDROUTE_WHISPER_MODEL";
pub const LANGUAGE_ENV_VAR: &str = "RECORDROUTE_WHISPER_LANGUAGE";
pub(crate) const DEFAULT_MODEL_RELATIVE_PATH: &str = "models/whisper/ggml-base.bin";
pub(crate) const DEFAULT_LANGUAGE: &str = "ko";
pub(crate) const MODEL_URL_TEMPLATE_ENV_VAR: &str = "RECORDROUTE_WHISPER_MODEL_URL_TEMPLATE";
pub(crate) const MODEL_SOURCE_DIR_ENV_VAR: &str = "RECORDROUTE_WHISPER_MODEL_SOURCE_DIR";
pub(crate) const DEFAULT_MODEL_URL_TEMPLATE: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{model}.bin";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toolchain {
    pub whisper_cli_path: PathBuf,
    pub build_script_path: PathBuf,
    pub model_path: PathBuf,
}

impl Toolchain {
    pub fn discover(repo_root: &std::path::Path) -> Result<Self, String> {
        discovery::discover(repo_root)
    }

    pub fn is_model_ready(&self) -> bool {
        self.model_path.is_file()
    }

    pub fn can_prepare_model(&self) -> Result<(), String> {
        if self.is_model_ready() {
            return Ok(());
        }

        let _ = download::managed_model_name(&self.model_path)?;
        let _ = download::model_directory(&self.model_path)?;
        Ok(())
    }

    pub fn ensure_model(&self) -> Result<(), String> {
        if self.is_model_ready() {
            return Ok(());
        }

        let model_name = download::managed_model_name(&self.model_path)?;
        let model_dir = download::model_directory(&self.model_path)?;
        download::download_model(self, &model_name, model_dir)
    }
}

pub fn run_transcription(
    toolchain: &Toolchain,
    input: &std::path::Path,
    output_text: &std::path::Path,
    language: &str,
    keywords: &[String],
) -> Result<(), String> {
    runtime::run_transcription(toolchain, input, output_text, language, keywords)
}

pub(crate) fn model_description(toolchain: &Toolchain) -> String {
    toolchain.model_path.display().to_string()
}

pub fn transcription_language_from_env() -> String {
    std::env::var(LANGUAGE_ENV_VAR)
        .ok()
        .and_then(|raw| normalize_language(&raw))
        .unwrap_or_else(|| DEFAULT_LANGUAGE.to_string())
}

pub fn normalize_language(raw: &str) -> Option<String> {
    let language = raw.trim().to_ascii_lowercase();
    if language.is_empty() {
        None
    } else {
        Some(language)
    }
}

pub fn normalize_keywords(keywords: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();

    for keyword in keywords {
        let trimmed = keyword.trim();
        if trimmed.is_empty() {
            continue;
        }

        let owned = trimmed.to_string();
        if !normalized.contains(&owned) {
            normalized.push(owned);
        }
    }

    normalized
}

pub(crate) fn build_keyword_prompt(keywords: &[String]) -> Option<String> {
    if keywords.is_empty() {
        None
    } else {
        Some(keywords.join(", "))
    }
}
