use super::{
    DEFAULT_EMBEDDING_MODEL_REPOSITORY, DEFAULT_MODEL_REPOSITORY, EMBEDDING_MODEL_ENV_VAR,
    HF_CACHE_RELATIVE_DIR, MODEL_ENV_VAR, ModelSource, Toolchain,
};
use crate::tool_runtime::{local_toolchain_layout, optional_local_command, require_local_command};
use std::path::{Path, PathBuf};

pub(crate) fn discover(repo_root: &Path) -> Result<Toolchain, String> {
    let layout = local_toolchain_layout(repo_root, "llama", Path::new("bin"));
    let llama_cli_path = require_local_command(
        &layout.bin_dir,
        "llama-cli",
        &layout.build_script_path,
        "llama",
    )?;
    let llama_embedding_path = optional_local_command(&layout.bin_dir, "llama-embedding")
        .unwrap_or_else(|| layout.bin_dir.join("llama-embedding"));

    let model_source = resolve_model_source(repo_root);
    let summary_cached_model_path = match &model_source {
        ModelSource::LocalPath(_) => None,
        ModelSource::HuggingFaceRepo(repo) => Some(cached_model_path(repo_root, repo)),
    };
    let embedding_model_source = resolve_embedding_model_source(repo_root);
    let embedding_cached_model_path = match &embedding_model_source {
        ModelSource::LocalPath(_) => None,
        ModelSource::HuggingFaceRepo(repo) => Some(cached_model_path(repo_root, repo)),
    };

    Ok(Toolchain {
        llama_cli_path,
        llama_embedding_path,
        build_script_path: layout.build_script_path,
        model_source,
        cached_model_path: summary_cached_model_path,
        embedding_model_source,
        embedding_cached_model_path,
    })
}

pub(crate) fn resolve_model_source(repo_root: &Path) -> ModelSource {
    match std::env::var_os(MODEL_ENV_VAR) {
        Some(value) if !value.is_empty() => {
            resolve_model_source_override(repo_root, PathBuf::from(value))
        }
        _ => ModelSource::HuggingFaceRepo(DEFAULT_MODEL_REPOSITORY.to_string()),
    }
}

pub(crate) fn resolve_embedding_model_source(repo_root: &Path) -> ModelSource {
    match std::env::var_os(EMBEDDING_MODEL_ENV_VAR) {
        Some(value) if !value.is_empty() => {
            resolve_model_source_override(repo_root, PathBuf::from(value))
        }
        _ => ModelSource::HuggingFaceRepo(DEFAULT_EMBEDDING_MODEL_REPOSITORY.to_string()),
    }
}

fn resolve_model_source_override(repo_root: &Path, configured: PathBuf) -> ModelSource {
    let local_candidate = if configured.is_absolute() {
        configured.clone()
    } else {
        repo_root.join(&configured)
    };

    if local_candidate.is_file() {
        ModelSource::LocalPath(local_candidate)
    } else {
        ModelSource::HuggingFaceRepo(configured.to_string_lossy().into_owned())
    }
}

pub(crate) fn source_is_ready(source: &ModelSource, cached_path: Option<&Path>) -> bool {
    match (source, cached_path) {
        (ModelSource::LocalPath(path), _) => path.is_file(),
        (ModelSource::HuggingFaceRepo(_), Some(cache_path)) => cache_path.is_file(),
        (ModelSource::HuggingFaceRepo(_), None) => false,
    }
}

pub(crate) fn can_prepare_source(
    label: &str,
    source: &ModelSource,
    cached_path: Option<&Path>,
) -> Result<(), String> {
    match (source, cached_path) {
        (ModelSource::LocalPath(path), _) => {
            if path.is_file() {
                Ok(())
            } else {
                Err(format!("{label} model file not found: {}", path.display()))
            }
        }
        (ModelSource::HuggingFaceRepo(_), Some(_)) => Ok(()),
        (ModelSource::HuggingFaceRepo(repo), None) => Err(format!(
            "{label} model cache path is unavailable for configured Hugging Face repo: {repo}"
        )),
    }
}

pub(crate) fn runtime_source(source: &ModelSource, cached_path: Option<&Path>) -> ModelSource {
    match (source, cached_path) {
        (ModelSource::LocalPath(path), _) => ModelSource::LocalPath(path.clone()),
        (ModelSource::HuggingFaceRepo(_), Some(cache_path)) if cache_path.is_file() => {
            ModelSource::LocalPath(cache_path.to_path_buf())
        }
        (ModelSource::HuggingFaceRepo(repo), _) => ModelSource::HuggingFaceRepo(repo.clone()),
    }
}

pub(crate) fn describe_model_source(source: &ModelSource, cached_path: Option<&Path>) -> String {
    match (source, cached_path) {
        (ModelSource::LocalPath(path), _) => path.display().to_string(),
        (ModelSource::HuggingFaceRepo(repo), Some(cache_path)) => {
            format!("{repo} -> {}", cache_path.display())
        }
        (ModelSource::HuggingFaceRepo(repo), None) => repo.clone(),
    }
}

pub(crate) fn missing_model_message(toolchain: &Toolchain) -> String {
    match (
        &toolchain.model_source,
        toolchain.cached_model_path.as_deref(),
    ) {
        (ModelSource::LocalPath(path), _) => {
            format!("llama model file not found: {}", path.display())
        }
        (ModelSource::HuggingFaceRepo(repo), Some(cache_path)) => format!(
            "llama model cache not found for {repo}: {}",
            cache_path.display()
        ),
        (ModelSource::HuggingFaceRepo(repo), None) => {
            format!(
                "llama model cache path is unavailable for configured Hugging Face repo: {repo}"
            )
        }
    }
}

pub(crate) fn missing_embedding_toolchain_message(toolchain: &Toolchain) -> String {
    format!(
        "local llama embedding toolchain not found. Build it first with {}",
        toolchain.build_script_path.display()
    )
}

pub(crate) fn cached_model_path(repo_root: &Path, repo: &str) -> PathBuf {
    repo_root
        .join(HF_CACHE_RELATIVE_DIR)
        .join(format!("{}.gguf", cache_key(repo)))
}

pub(crate) fn hf_download_cache_dir(build_script_path: &Path, repo: &str) -> PathBuf {
    build_script_path
        .parent()
        .and_then(Path::parent)
        .map(|repo_root| {
            repo_root
                .join(HF_CACHE_RELATIVE_DIR)
                .join(".cache")
                .join(cache_key(repo))
        })
        .unwrap_or_else(|| {
            PathBuf::from(HF_CACHE_RELATIVE_DIR)
                .join(".cache")
                .join(cache_key(repo))
        })
}

fn cache_key(repo: &str) -> String {
    let mut key = String::with_capacity(repo.len());
    for ch in repo.chars() {
        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => key.push(ch),
            '/' => key.push_str("__"),
            ':' => key.push_str("___"),
            _ => key.push('_'),
        }
    }

    if key.is_empty() {
        "model".to_string()
    } else {
        key
    }
}
