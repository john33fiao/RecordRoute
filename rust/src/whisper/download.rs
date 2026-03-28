use super::discovery::{infer_model_name, managed_repo_root};
use super::{
    DEFAULT_MODEL_RELATIVE_PATH, DEFAULT_MODEL_URL_TEMPLATE, MODEL_ENV_VAR,
    MODEL_SOURCE_DIR_ENV_VAR, MODEL_URL_TEMPLATE_ENV_VAR, Toolchain,
};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

pub(crate) fn should_refresh_managed_model(toolchain: &Toolchain, error: &str) -> bool {
    if !error.contains("failed to initialize whisper context") {
        return false;
    }

    let Some(repo_root) = managed_repo_root(toolchain) else {
        return false;
    };
    let managed_dir = repo_root.join("models/whisper");

    toolchain.model_path.is_file()
        && toolchain.model_path.starts_with(&managed_dir)
        && infer_model_name(&toolchain.model_path).is_some()
}

pub(crate) fn refresh_managed_model(toolchain: &Toolchain) -> Result<(), String> {
    let model_name = managed_model_name(&toolchain.model_path)?;
    let model_dir = model_directory(&toolchain.model_path)?;

    match fs::remove_file(&toolchain.model_path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "failed to remove invalid whisper model cache {}: {error}",
                toolchain.model_path.display()
            ));
        }
    }

    download_model(toolchain, &model_name, model_dir)
}

pub(crate) fn download_model(
    toolchain: &Toolchain,
    model_name: &str,
    model_dir: &Path,
) -> Result<(), String> {
    fs::create_dir_all(model_dir).map_err(|error| {
        format!(
            "failed to create whisper model directory {}: {error}",
            model_dir.display()
        )
    })?;

    if let Some(source_dir) = std::env::var_os(MODEL_SOURCE_DIR_ENV_VAR) {
        let source = std::path::PathBuf::from(source_dir).join(format!("ggml-{model_name}.bin"));
        if source.is_file() {
            fs::copy(&source, &toolchain.model_path).map_err(|error| {
                format!(
                    "failed to copy whisper model from {} to {}: {error}",
                    source.display(),
                    toolchain.model_path.display()
                )
            })?;
            return Ok(());
        }
    }

    let template = std::env::var(MODEL_URL_TEMPLATE_ENV_VAR)
        .unwrap_or_else(|_| DEFAULT_MODEL_URL_TEMPLATE.to_string());
    let url = template.replace("{model}", model_name);

    let response = reqwest::blocking::get(&url)
        .and_then(|response| response.error_for_status())
        .map_err(|error| {
            format!(
                "failed to download whisper model {} from {}: {error}",
                model_name, url
            )
        })?;
    let bytes = response.bytes().map_err(|error| {
        format!(
            "failed to read whisper model response body from {}: {error}",
            url
        )
    })?;

    fs::write(&toolchain.model_path, &bytes).map_err(|error| {
        format!(
            "failed to write whisper model {} to {}: {error}",
            model_name,
            toolchain.model_path.display()
        )
    })
}

pub(crate) fn managed_model_name(model_path: &Path) -> Result<String, String> {
    infer_model_name(model_path).ok_or_else(|| {
        format!(
            "whisper model not found at {} and the configured path does not match ggml-<model>.bin. Update {} or use {}",
            model_path.display(),
            MODEL_ENV_VAR,
            DEFAULT_MODEL_RELATIVE_PATH
        )
    })
}

pub(crate) fn model_directory(model_path: &Path) -> Result<&Path, String> {
    model_path.parent().ok_or_else(|| {
        format!(
            "whisper model path has no parent directory: {}",
            model_path.display()
        )
    })
}
