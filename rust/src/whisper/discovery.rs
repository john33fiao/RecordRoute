use super::{DEFAULT_MODEL_RELATIVE_PATH, MODEL_ENV_VAR, Toolchain};
use crate::ffmpeg::{build_script_path, locate_command, target_dir_name};
use std::path::{Path, PathBuf};

pub(crate) fn discover(repo_root: &Path) -> Result<Toolchain, String> {
    let build_script_path = build_script_path(repo_root, "whisper");
    let whisper_bin = repo_root
        .join(".build/whisper")
        .join(target_dir_name())
        .join("bin");
    let whisper_cli_path = locate_command(&whisper_bin, "whisper-cli").ok_or_else(|| {
        format!(
            "local whisper toolchain not found. Build it first with {}",
            build_script_path.display()
        )
    })?;

    Ok(Toolchain {
        whisper_cli_path,
        build_script_path,
        model_path: resolve_model_path(repo_root),
    })
}

pub(crate) fn resolve_model_path(repo_root: &Path) -> PathBuf {
    match std::env::var_os(MODEL_ENV_VAR) {
        Some(path) if !path.is_empty() => {
            resolve_model_override_path(repo_root, PathBuf::from(path))
        }
        _ => repo_root.join(DEFAULT_MODEL_RELATIVE_PATH),
    }
}

pub(crate) fn managed_repo_root(toolchain: &Toolchain) -> Option<PathBuf> {
    toolchain
        .build_script_path
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
}

pub(crate) fn infer_model_name(model_path: &Path) -> Option<String> {
    let file_name = model_path.file_name()?.to_str()?;
    let suffix = ".bin";
    let prefix = "ggml-";

    file_name
        .strip_prefix(prefix)?
        .strip_suffix(suffix)
        .map(ToOwned::to_owned)
}

fn resolve_model_override_path(repo_root: &Path, configured: PathBuf) -> PathBuf {
    let explicit_path = if configured.is_absolute() {
        configured.clone()
    } else {
        repo_root.join(&configured)
    };
    if explicit_path.is_file() {
        return explicit_path;
    }

    let file_name = configured
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if file_name.is_empty() {
        return explicit_path;
    }

    let normalized_name = if Path::new(file_name).extension().is_some() {
        file_name.to_string()
    } else {
        normalized_model_file_name(file_name)
    };
    let normalized_parent = if configured.is_absolute() {
        explicit_path.parent().map(Path::to_path_buf)
    } else if configured.components().count() > 1 {
        explicit_path.parent().map(Path::to_path_buf)
    } else {
        Some(repo_root.join("models/whisper"))
    };

    match normalized_parent {
        Some(parent) => parent.join(normalized_name),
        None => explicit_path,
    }
}

fn normalized_model_file_name(file_name: &str) -> String {
    let stem = file_name
        .strip_suffix(".bin")
        .unwrap_or(file_name)
        .strip_prefix("ggml-")
        .unwrap_or(file_name.strip_suffix(".bin").unwrap_or(file_name));
    format!("ggml-{stem}.bin")
}
