mod discovery;
mod download;
mod output;
mod runtime;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

pub const MODEL_ENV_VAR: &str = "RECORDROUTE_LLAMA_MODEL";
pub const EMBEDDING_MODEL_ENV_VAR: &str = "RECORDROUTE_LLAMA_EMBEDDING_MODEL";
pub(crate) const DEFAULT_MODEL_REPOSITORY: &str = "ggml-org/gemma-3-4b-it-GGUF";
pub(crate) const DEFAULT_EMBEDDING_MODEL_REPOSITORY: &str = "Qwen/Qwen3-Embedding-4B-GGUF";
pub(crate) const DEFAULT_HF_QUANT_TAG: &str = "Q4_K_M";
pub(crate) const DEFAULT_PREDICT_TOKENS: &str = "1024";
pub(crate) const HF_CACHE_RELATIVE_DIR: &str = "models/llama/hf";
pub(crate) const LLAMA_CACHE_ENV_VAR: &str = "LLAMA_CACHE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSource {
    LocalPath(PathBuf),
    HuggingFaceRepo(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toolchain {
    pub llama_cli_path: PathBuf,
    pub llama_embedding_path: PathBuf,
    pub build_script_path: PathBuf,
    pub model_source: ModelSource,
    pub cached_model_path: Option<PathBuf>,
    pub embedding_model_source: ModelSource,
    pub embedding_cached_model_path: Option<PathBuf>,
}

impl Toolchain {
    pub fn discover(repo_root: &Path) -> Result<Self, String> {
        discovery::discover(repo_root)
    }

    pub fn is_model_ready(&self) -> bool {
        discovery::source_is_ready(&self.model_source, self.cached_model_path.as_deref())
    }

    pub fn can_prepare_model(&self) -> Result<(), String> {
        discovery::can_prepare_source(
            "llama",
            &self.model_source,
            self.cached_model_path.as_deref(),
        )
    }

    pub fn ensure_model(&self) -> Result<(), String> {
        download::ensure_source(
            self,
            "llama",
            &self.model_source,
            self.cached_model_path.as_deref(),
        )
    }

    pub fn is_embedding_model_ready(&self) -> bool {
        discovery::source_is_ready(
            &self.embedding_model_source,
            self.embedding_cached_model_path.as_deref(),
        )
    }

    pub fn can_prepare_embedding_model(&self) -> Result<(), String> {
        discovery::can_prepare_source(
            "llama embedding",
            &self.embedding_model_source,
            self.embedding_cached_model_path.as_deref(),
        )
    }

    pub fn ensure_embedding_model(&self) -> Result<(), String> {
        download::ensure_source(
            self,
            "llama embedding",
            &self.embedding_model_source,
            self.embedding_cached_model_path.as_deref(),
        )
    }

    fn runtime_model_source(&self) -> ModelSource {
        discovery::runtime_source(&self.model_source, self.cached_model_path.as_deref())
    }

    fn runtime_embedding_model_source(&self) -> ModelSource {
        discovery::runtime_source(
            &self.embedding_model_source,
            self.embedding_cached_model_path.as_deref(),
        )
    }
}

pub fn embedding_model_id(_repo_root: &Path) -> String {
    match std::env::var(EMBEDDING_MODEL_ENV_VAR) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => DEFAULT_EMBEDDING_MODEL_REPOSITORY.to_string(),
    }
}

pub fn run_summary_embedding(toolchain: &Toolchain, input: &str) -> Result<Vec<f32>, String> {
    runtime::run_summary_embedding(toolchain, input)
}

pub fn run_summary_generation(
    toolchain: &Toolchain,
    prompt_file: &Path,
    output_file: &Path,
) -> Result<(), String> {
    runtime::run_summary_generation(toolchain, prompt_file, output_file)
}

pub(crate) fn summary_model_description(toolchain: &Toolchain) -> String {
    discovery::describe_model_source(
        &toolchain.model_source,
        toolchain.cached_model_path.as_deref(),
    )
}

pub(crate) fn embedding_model_description(toolchain: &Toolchain) -> String {
    discovery::describe_model_source(
        &toolchain.embedding_model_source,
        toolchain.embedding_cached_model_path.as_deref(),
    )
}

pub(crate) fn missing_model_message(toolchain: &Toolchain) -> String {
    discovery::missing_model_message(toolchain)
}

pub(crate) fn missing_embedding_toolchain_message(toolchain: &Toolchain) -> String {
    discovery::missing_embedding_toolchain_message(toolchain)
}
