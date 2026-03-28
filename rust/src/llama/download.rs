use super::discovery::hf_download_cache_dir;
use super::runtime::{LlamaRuntimeBackend, configure_runtime_backend, should_retry_llama_on_cpu};
use super::{DEFAULT_HF_QUANT_TAG, LLAMA_CACHE_ENV_VAR, ModelSource, Toolchain};
use crate::tool_runtime::run_with_cpu_fallback;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

pub(crate) fn ensure_source(
    toolchain: &Toolchain,
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
        (ModelSource::HuggingFaceRepo(repo), Some(cache_path)) => {
            ensure_hugging_face_model(toolchain, repo, cache_path)
        }
        (ModelSource::HuggingFaceRepo(repo), None) => Err(format!(
            "{label} model cache path is unavailable for configured Hugging Face repo: {repo}"
        )),
    }
}

fn ensure_hugging_face_model(
    toolchain: &Toolchain,
    repo: &str,
    cache_path: &Path,
) -> Result<(), String> {
    if cache_path.is_file() {
        return Ok(());
    }

    run_with_cpu_fallback(
        || {
            download_hugging_face_model_with_backend(
                toolchain,
                repo,
                cache_path,
                LlamaRuntimeBackend::Preferred,
            )
        },
        || {
            download_hugging_face_model_with_backend(
                toolchain,
                repo,
                cache_path,
                LlamaRuntimeBackend::CpuFallback,
            )
        },
        should_retry_llama_on_cpu,
    )
}

fn download_hugging_face_model_with_backend(
    toolchain: &Toolchain,
    repo: &str,
    cache_path: &Path,
    backend: LlamaRuntimeBackend,
) -> Result<(), String> {
    if cache_path.is_file() {
        return Ok(());
    }

    let cache_dir = cache_path.parent().ok_or_else(|| {
        format!(
            "llama cache path has no parent directory: {}",
            cache_path.display()
        )
    })?;
    fs::create_dir_all(cache_dir).map_err(|error| {
        format!(
            "failed to create llama model cache directory {}: {error}",
            cache_dir.display()
        )
    })?;
    let download_cache_dir = hf_download_cache_dir(&toolchain.build_script_path, repo);
    fs::create_dir_all(&download_cache_dir).map_err(|error| {
        format!(
            "failed to create llama download cache directory {}: {error}",
            download_cache_dir.display()
        )
    })?;
    let existing_downloads = gguf_files_in(&download_cache_dir)?;

    if let Ok(downloaded_model) =
        detect_cached_hugging_face_model_file(&download_cache_dir, &BTreeSet::new(), repo)
    {
        finalize_downloaded_model(&downloaded_model, cache_path)?;
        return Ok(());
    }

    let mut command = Command::new(&toolchain.llama_cli_path);
    command
        .env(LLAMA_CACHE_ENV_VAR, &download_cache_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .arg("--single-turn")
        .arg("--simple-io")
        .arg("--no-display-prompt")
        .arg("--log-disable")
        .arg("--no-warmup")
        .arg("--no-mmproj");
    configure_runtime_backend(&mut command, backend);
    eprintln!(
        "Downloading {repo} via {} into {}. First-time download can take several minutes.",
        toolchain.llama_cli_path.display(),
        cache_path.display()
    );
    let mut child = command
        .arg("-n")
        .arg("0")
        .arg("-hf")
        .arg(repo)
        .arg("-p")
        .arg("/exit")
        .spawn()
        .map_err(|error| {
            format!(
                "failed to execute llama-cli {} for model download: {error}",
                toolchain.llama_cli_path.display()
            )
        })?;

    let exit_status = loop {
        if cache_path.is_file() {
            return Ok(());
        }

        if let Ok(downloaded_model) =
            detect_cached_hugging_face_model_file(&download_cache_dir, &existing_downloads, repo)
        {
            let _ = child.kill();
            let _ = child.wait();
            finalize_downloaded_model(&downloaded_model, cache_path)?;
            return Ok(());
        }

        match child.try_wait().map_err(|error| {
            format!(
                "failed while waiting for llama-cli {} during model download: {error}",
                toolchain.llama_cli_path.display()
            )
        })? {
            Some(status) => break status,
            None => thread::sleep(Duration::from_millis(500)),
        }
    };

    if exit_status.success() {
        if cache_path.is_file() {
            return Ok(());
        }

        let downloaded_model =
            detect_cached_hugging_face_model_file(&download_cache_dir, &existing_downloads, repo)?;
        finalize_downloaded_model(&downloaded_model, cache_path)?;
        return Ok(());
    }
    Err(format!(
        "failed to download llama model {repo} to {}: llama-cli exited with {exit_status}",
        cache_path.display(),
    ))
}

fn gguf_files_in(dir: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let mut files = BTreeSet::new();
    collect_gguf_files(dir, &mut files)?;
    Ok(files)
}

fn collect_gguf_files(dir: &Path, files: &mut BTreeSet<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|error| {
        format!(
            "failed to read llama cache directory {}: {error}",
            dir.display()
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to inspect llama cache directory entry in {}: {error}",
                dir.display()
            )
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "failed to inspect llama cache entry type in {}: {error}",
                path.display()
            )
        })?;

        if file_type.is_dir() {
            collect_gguf_files(&path, files)?;
            continue;
        }

        if path
            .extension()
            .is_some_and(|extension| extension == "gguf")
            && (path.is_file() || file_type.is_symlink())
        {
            files.insert(path);
        }
    }

    Ok(())
}

fn detect_downloaded_model_file(
    download_cache_dir: &Path,
    existing_downloads: &BTreeSet<PathBuf>,
) -> Result<PathBuf, String> {
    let downloads = gguf_files_in(download_cache_dir)?;
    if let Some(path) = downloads.difference(existing_downloads).next().cloned() {
        return Ok(path);
    }

    if downloads.len() == 1 {
        return downloads.into_iter().next().ok_or_else(|| {
            format!(
                "llama download cache unexpectedly became empty: {}",
                download_cache_dir.display()
            )
        });
    }

    Err(format!(
        "could not determine downloaded llama model file in {}",
        download_cache_dir.display()
    ))
}

fn detect_cached_hugging_face_model_file(
    download_cache_dir: &Path,
    existing_downloads: &BTreeSet<PathBuf>,
    repo: &str,
) -> Result<PathBuf, String> {
    detect_downloaded_model_file(download_cache_dir, existing_downloads)
        .or_else(|_| detect_hugging_face_hub_model_file(repo))
}

fn detect_hugging_face_hub_model_file(repo: &str) -> Result<PathBuf, String> {
    let (repo_path, preferred_quant) = split_hugging_face_repo_tag(repo);
    let (owner, model) = repo_path.split_once('/').ok_or_else(|| {
        format!("invalid Hugging Face repo format, expected owner/model[:quant]: {repo}")
    })?;
    let hub_cache_dir = default_hugging_face_hub_cache_dir().ok_or_else(|| {
        format!("could not determine Hugging Face hub cache directory for repo {repo}")
    })?;
    let repo_cache_dir = hub_cache_dir.join(format!("models--{owner}--{model}"));
    let snapshot_dir = repo_cache_dir.join("snapshots");
    let search_dir = if snapshot_dir.is_dir() {
        snapshot_dir
    } else {
        repo_cache_dir
    };
    let downloads = gguf_files_in(&search_dir)?;
    choose_hugging_face_model_file(downloads, preferred_quant).ok_or_else(|| {
        format!(
            "could not determine downloaded llama model file in {}",
            search_dir.display()
        )
    })
}

fn split_hugging_face_repo_tag(repo: &str) -> (&str, Option<&str>) {
    match repo.rsplit_once(':') {
        Some((repo_path, tag)) if repo_path.contains('/') && !tag.is_empty() => {
            (repo_path, Some(tag))
        }
        _ => (repo, None),
    }
}

fn choose_hugging_face_model_file(
    downloads: BTreeSet<PathBuf>,
    preferred_quant: Option<&str>,
) -> Option<PathBuf> {
    if downloads.is_empty() {
        return None;
    }

    let preferred_quant = preferred_quant
        .unwrap_or(DEFAULT_HF_QUANT_TAG)
        .to_ascii_uppercase();
    downloads
        .iter()
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_ascii_uppercase().contains(&preferred_quant))
                .unwrap_or(false)
        })
        .cloned()
        .or_else(|| downloads.into_iter().next())
}

fn default_hugging_face_hub_cache_dir() -> Option<PathBuf> {
    if let Some(hf_home) = env::var_os("HF_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(hf_home).join("hub"));
    }

    if let Some(xdg_cache_home) = env::var_os("XDG_CACHE_HOME").filter(|value| !value.is_empty()) {
        return Some(
            PathBuf::from(xdg_cache_home)
                .join("huggingface")
                .join("hub"),
        );
    }

    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| {
            PathBuf::from(home)
                .join(".cache")
                .join("huggingface")
                .join("hub")
        })
}

fn finalize_downloaded_model(downloaded_model: &Path, cache_path: &Path) -> Result<(), String> {
    fs::copy(downloaded_model, cache_path).map_err(|error| {
        format!(
            "failed to copy downloaded llama model from {} to {}: {error}",
            downloaded_model.display(),
            cache_path.display()
        )
    })?;

    if cache_path.is_file() {
        Ok(())
    } else {
        Err(format!(
            "downloaded llama model was not written to expected cache path {}",
            cache_path.display()
        ))
    }
}
