use crate::ffmpeg::{build_script_path, locate_command, target_dir_name};
use crate::tool_runtime::{apply_cpu_fallback_env, command_output_details, should_retry_with_cpu};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::Duration;

pub const MODEL_ENV_VAR: &str = "RECORDROUTE_LLAMA_MODEL";
pub const EMBEDDING_MODEL_ENV_VAR: &str = "RECORDROUTE_LLAMA_EMBEDDING_MODEL";
const DEFAULT_MODEL_REPOSITORY: &str = "ggml-org/gemma-3-4b-it-GGUF";
const DEFAULT_EMBEDDING_MODEL_REPOSITORY: &str = "Qwen/Qwen3-Embedding-4B";
const DEFAULT_PREDICT_TOKENS: &str = "1024";
const HF_CACHE_RELATIVE_DIR: &str = "models/llama/hf";
const LLAMA_CACHE_ENV_VAR: &str = "LLAMA_CACHE";

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
}

impl Toolchain {
    pub fn discover(repo_root: &Path) -> Result<Self, String> {
        let build_script_path = build_script_path(repo_root, "llama");
        let llama_bin = repo_root
            .join(".build/llama")
            .join(target_dir_name())
            .join("bin");
        let llama_cli_path = locate_command(&llama_bin, "llama-cli").ok_or_else(|| {
            format!(
                "local llama toolchain not found. Build it first with {}",
                build_script_path.display()
            )
        })?;
        let llama_embedding_path = locate_command(&llama_bin, "llama-embedding")
            .unwrap_or_else(|| llama_bin.join("llama-embedding"));

        let model_source = resolve_model_source(repo_root);
        let cached_model_path = match &model_source {
            ModelSource::LocalPath(_) => None,
            ModelSource::HuggingFaceRepo(repo) => Some(cached_model_path(repo_root, repo)),
        };

        Ok(Self {
            llama_cli_path,
            llama_embedding_path,
            build_script_path,
            model_source,
            cached_model_path,
        })
    }

    pub fn is_model_ready(&self) -> bool {
        match (&self.model_source, &self.cached_model_path) {
            (ModelSource::LocalPath(path), _) => path.is_file(),
            (ModelSource::HuggingFaceRepo(_), Some(cache_path)) => cache_path.is_file(),
            (ModelSource::HuggingFaceRepo(_), None) => false,
        }
    }

    pub fn can_prepare_model(&self) -> Result<(), String> {
        match (&self.model_source, &self.cached_model_path) {
            (ModelSource::LocalPath(path), _) => {
                if path.is_file() {
                    Ok(())
                } else {
                    Err(format!("llama model file not found: {}", path.display()))
                }
            }
            (ModelSource::HuggingFaceRepo(_), Some(_)) => Ok(()),
            (ModelSource::HuggingFaceRepo(repo), None) => Err(format!(
                "llama model cache path is unavailable for configured Hugging Face repo: {repo}"
            )),
        }
    }

    pub fn ensure_model(&self) -> Result<(), String> {
        match (&self.model_source, &self.cached_model_path) {
            (ModelSource::LocalPath(path), _) => {
                if path.is_file() {
                    Ok(())
                } else {
                    Err(format!("llama model file not found: {}", path.display()))
                }
            }
            (ModelSource::HuggingFaceRepo(repo), Some(cache_path)) => {
                ensure_hugging_face_model(self, repo, cache_path)
            }
            (ModelSource::HuggingFaceRepo(repo), None) => Err(format!(
                "llama model cache path is unavailable for configured Hugging Face repo: {repo}"
            )),
        }
    }

    fn runtime_model_source(&self) -> ModelSource {
        match (&self.model_source, &self.cached_model_path) {
            (ModelSource::LocalPath(path), _) => ModelSource::LocalPath(path.clone()),
            (ModelSource::HuggingFaceRepo(_), Some(cache_path)) if cache_path.is_file() => {
                ModelSource::LocalPath(cache_path.clone())
            }
            (ModelSource::HuggingFaceRepo(repo), _) => ModelSource::HuggingFaceRepo(repo.clone()),
        }
    }
}

pub fn embedding_model_id(repo_root: &Path) -> String {
    match std::env::var(EMBEDDING_MODEL_ENV_VAR) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            let default_gguf =
                repo_root.join("models/llama/hf/Qwen_Qwen3-Embedding-4B-GGUF-Q4_K_M.gguf");
            if default_gguf.is_file() {
                "Qwen/Qwen3-Embedding-4B-GGUF:Q4_K_M.gguf".to_string()
            } else {
                DEFAULT_EMBEDDING_MODEL_REPOSITORY.to_string()
            }
        }
    }
}

pub fn run_summary_embedding(toolchain: &Toolchain, input: &str) -> Result<Vec<f32>, String> {
    let mut command = Command::new(&toolchain.llama_embedding_path);
    command
        .arg("--pooling")
        .arg("mean")
        .arg("--embd-normalize")
        .arg("2")
        .arg("--embd-output-format")
        .arg("array")
        .arg("--log-disable")
        .arg("-p")
        .arg(input);

    match toolchain.runtime_model_source() {
        ModelSource::LocalPath(path) => {
            command.arg("-m").arg(path);
        }
        ModelSource::HuggingFaceRepo(repo) => {
            command.env(LLAMA_CACHE_ENV_VAR, hf_download_cache_dir(toolchain, &repo));
            command.arg("-hf").arg(repo);
        }
    }

    let output = command.output().map_err(|error| {
        format!(
            "failed to execute llama-embedding {}: {error}",
            toolchain.llama_embedding_path.display()
        )
    })?;
    if !output.status.success() {
        return Err(format!(
            "llama embedding failed: {}",
            command_output_details(&output)
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str::<Vec<f32>>(stdout.trim())
        .map_err(|error| format!("failed to parse embedding output: {error}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LlamaRuntimeBackend {
    Preferred,
    CpuFallback,
}

fn configure_runtime_backend(command: &mut Command, backend: LlamaRuntimeBackend) {
    match backend {
        LlamaRuntimeBackend::CpuFallback => {
            command
                .arg("-ngl")
                .arg("0")
                .arg("--device")
                .arg("none")
                .arg("--no-op-offload")
                .arg("--no-kv-offload")
                .arg("--no-mmproj-offload");

            apply_cpu_fallback_env(command);
        }
        LlamaRuntimeBackend::Preferred => {}
    }
}

fn should_retry_llama_on_cpu(error: &str) -> bool {
    should_retry_with_cpu(
        error,
        &["metal", "ggml-metal", "ggml_metal", "mtl"],
        &["cuda", "cublas", "ggml-cuda", "ggml_cuda", "nvidia"],
    )
}

pub fn run_summary_generation(
    toolchain: &Toolchain,
    prompt_file: &Path,
    output_file: &Path,
) -> Result<(), String> {
    if !prompt_file.is_file() {
        return Err(format!(
            "summary prompt file not found: {}",
            prompt_file.display()
        ));
    }

    if let Some(parent) = output_file.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create summary output directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let _ = fs::remove_file(output_file);
    let output = execute_summary_generation(toolchain, prompt_file)?;

    let prompt = fs::read_to_string(prompt_file).unwrap_or_default();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let summary_text = extract_summary_text(&stdout, &prompt);

    if summary_text.is_empty() {
        return Err(format!(
            "llama-cli completed without producing summary text for {}",
            prompt_file.display()
        ));
    }

    fs::write(output_file, summary_text).map_err(|error| {
        format!(
            "failed to write summary output {}: {error}",
            output_file.display()
        )
    })
}

fn execute_summary_generation(toolchain: &Toolchain, prompt_file: &Path) -> Result<Output, String> {
    match run_summary_generation_once(toolchain, prompt_file, LlamaRuntimeBackend::Preferred) {
        Ok(output) => Ok(output),
        Err(primary_error) if should_retry_llama_on_cpu(&primary_error) => {
            run_summary_generation_once(toolchain, prompt_file, LlamaRuntimeBackend::CpuFallback)
                .map_err(|cpu_error| {
                    format!(
                        "{cpu_error} (after retrying on CPU because the preferred backend failed: {primary_error})"
                    )
                })
        }
        Err(error) => Err(error),
    }
}

fn run_summary_generation_once(
    toolchain: &Toolchain,
    prompt_file: &Path,
    backend: LlamaRuntimeBackend,
) -> Result<Output, String> {
    let mut command = Command::new(&toolchain.llama_cli_path);
    command
        .arg("--single-turn")
        .arg("--simple-io")
        .arg("--no-display-prompt")
        .arg("--log-disable")
        .arg("-n")
        .arg(DEFAULT_PREDICT_TOKENS);
    configure_runtime_backend(&mut command, backend);

    match toolchain.runtime_model_source() {
        ModelSource::LocalPath(path) => {
            command.arg("-m").arg(path);
        }
        ModelSource::HuggingFaceRepo(repo) => {
            command.env(LLAMA_CACHE_ENV_VAR, hf_download_cache_dir(toolchain, &repo));
            command.arg("-hf").arg(repo);
        }
    }

    let output = command
        .arg("-f")
        .arg(prompt_file)
        .output()
        .map_err(|error| {
            format!(
                "failed to execute llama-cli {}: {error}",
                toolchain.llama_cli_path.display()
            )
        })?;

    if output.status.success() {
        Ok(output)
    } else {
        Err(summary_generation_error(prompt_file, &output))
    }
}

fn summary_generation_error(prompt_file: &Path, output: &Output) -> String {
    format!(
        "llama summary generation failed for {}: {}",
        prompt_file.display(),
        command_output_details(output)
    )
}

fn resolve_model_source(repo_root: &Path) -> ModelSource {
    match std::env::var_os(MODEL_ENV_VAR) {
        Some(value) if !value.is_empty() => {
            resolve_model_source_override(repo_root, PathBuf::from(value))
        }
        _ => ModelSource::HuggingFaceRepo(DEFAULT_MODEL_REPOSITORY.to_string()),
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

fn ensure_hugging_face_model(
    toolchain: &Toolchain,
    repo: &str,
    cache_path: &Path,
) -> Result<(), String> {
    if cache_path.is_file() {
        return Ok(());
    }

    match download_hugging_face_model(
        toolchain,
        repo,
        cache_path,
        LlamaRuntimeBackend::Preferred,
    ) {
        Ok(()) => Ok(()),
        Err(primary_error) if should_retry_llama_on_cpu(&primary_error) => {
            download_hugging_face_model(
                toolchain,
                repo,
                cache_path,
                LlamaRuntimeBackend::CpuFallback,
            )
            .map_err(|cpu_error| {
                format!(
                    "{cpu_error} (after retrying on CPU because the preferred backend failed: {primary_error})"
                )
            })
        }
        Err(error) => Err(error),
    }
}

fn download_hugging_face_model(
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
    let download_cache_dir = hf_download_cache_dir(toolchain, repo);
    fs::create_dir_all(&download_cache_dir).map_err(|error| {
        format!(
            "failed to create llama download cache directory {}: {error}",
            download_cache_dir.display()
        )
    })?;
    let existing_downloads = gguf_files_in(&download_cache_dir)?;

    if let Ok(downloaded_model) =
        detect_downloaded_model_file(&download_cache_dir, &BTreeSet::new())
    {
        finalize_downloaded_model(&downloaded_model, cache_path)?;
        return Ok(());
    }

    let mut command = Command::new(&toolchain.llama_cli_path);
    command
        .env(LLAMA_CACHE_ENV_VAR, &download_cache_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--single-turn")
        .arg("--simple-io")
        .arg("--no-display-prompt")
        .arg("--log-disable")
        .arg("--no-warmup")
        .arg("--no-mmproj");
    configure_runtime_backend(&mut command, backend);
    let mut child = command
        .arg("-n")
        .arg("0")
        .arg("-hf")
        .arg(repo)
        .arg("-p")
        .arg("")
        .spawn()
        .map_err(|error| {
            format!(
                "failed to execute llama-cli {} for model download: {error}",
                toolchain.llama_cli_path.display()
            )
        })?;

    loop {
        if cache_path.is_file() {
            return Ok(());
        }

        if let Ok(downloaded_model) =
            detect_downloaded_model_file(&download_cache_dir, &existing_downloads)
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
            Some(_) => break,
            None => thread::sleep(Duration::from_millis(500)),
        }
    }

    let output = child.wait_with_output().map_err(|error| {
        format!(
            "failed to collect llama-cli {} output after model download: {error}",
            toolchain.llama_cli_path.display()
        )
    })?;

    if output.status.success() {
        if cache_path.is_file() {
            return Ok(());
        }

        let downloaded_model =
            detect_downloaded_model_file(&download_cache_dir, &existing_downloads)?;
        finalize_downloaded_model(&downloaded_model, cache_path)?;
        return Ok(());
    }

    Err(format!(
        "failed to download llama model {repo} to {}: {}",
        cache_path.display(),
        command_output_details(&output)
    ))
}

fn cached_model_path(repo_root: &Path, repo: &str) -> PathBuf {
    repo_root
        .join(HF_CACHE_RELATIVE_DIR)
        .join(format!("{}.gguf", cache_key(repo)))
}

fn hf_download_cache_dir(toolchain: &Toolchain, repo: &str) -> PathBuf {
    toolchain
        .build_script_path
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

fn gguf_files_in(dir: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let entries = fs::read_dir(dir).map_err(|error| {
        format!(
            "failed to read llama cache directory {}: {error}",
            dir.display()
        )
    })?;
    let mut files = BTreeSet::new();

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to inspect llama cache directory entry in {}: {error}",
                dir.display()
            )
        })?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "gguf")
            && path.is_file()
        {
            files.insert(path);
        }
    }

    Ok(files)
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

fn extract_summary_text(stdout: &str, prompt: &str) -> String {
    let mut text = stdout;

    if !prompt.is_empty() {
        if let Some(index) = text.find(prompt) {
            text = &text[index + prompt.len()..];
        } else if let Some(index) = text.find("(truncated)") {
            text = &text[index + "(truncated)".len()..];
        }
    }

    if let Some(index) = find_summary_start(text) {
        text = &text[index..];
    }

    for marker in ["\n\n[ Prompt:", "\n[ Prompt:", "\nExiting..."] {
        if let Some(index) = text.find(marker) {
            text = &text[..index];
            break;
        }
    }

    normalize_summary_text(text)
}

fn find_summary_start(text: &str) -> Option<usize> {
    let mut earliest = None;

    for marker in [
        "## 회의록",
        "# 회의록",
        "회의록",
        "## 개요",
        "# 개요",
        "**개요**",
        "개요",
        "## 핵심 논의",
        "# 핵심 논의",
        "**핵심 논의**",
        "핵심 논의",
    ] {
        if text.starts_with(marker) {
            earliest = Some(earliest.unwrap_or(0).min(0));
        }

        let line_marker = format!("\n{marker}");
        if let Some(index) = text.find(&line_marker) {
            let candidate = index + 1;
            earliest = Some(earliest.map_or(candidate, |current| current.min(candidate)));
        }
    }

    earliest
}

fn normalize_summary_text(text: &str) -> String {
    let mut normalized_lines = Vec::new();

    for raw_line in text.trim().lines() {
        let trimmed = raw_line.trim();
        if is_summary_title(trimmed) {
            continue;
        }
        if let Some(heading) = canonical_summary_heading(trimmed) {
            normalized_lines.push(format!("## {heading}"));
            continue;
        }
        normalized_lines.push(raw_line.trim_end().to_string());
    }

    normalized_lines.join("\n").trim().to_string()
}

fn canonical_summary_heading(line: &str) -> Option<&'static str> {
    match normalized_heading_text(line) {
        "개요" => Some("개요"),
        "핵심 논의" => Some("핵심 논의"),
        "결정/합의" => Some("결정/합의"),
        "후속 조치" => Some("후속 조치"),
        _ => None,
    }
}

fn is_summary_title(line: &str) -> bool {
    normalized_heading_text(line) == "회의록"
}

fn normalized_heading_text(line: &str) -> &str {
    let trimmed = line.trim();
    let without_hashes = trimmed.trim_start_matches('#').trim();
    without_hashes
        .strip_prefix("**")
        .and_then(|inner| inner.strip_suffix("**"))
        .unwrap_or(without_hashes)
        .trim()
}

fn finalize_downloaded_model(downloaded_model: &Path, cache_path: &Path) -> Result<(), String> {
    fs::rename(downloaded_model, cache_path).map_err(|error| {
        format!(
            "failed to move downloaded llama model from {} to {}: {error}",
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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn discovers_toolchain_with_default_hugging_face_model() {
        let _guard = crate::test_support::env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        unsafe { std::env::remove_var(MODEL_ENV_VAR) };

        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/llama")
            .join(target_dir_name())
            .join("bin");
        fs::create_dir_all(&build_bin).expect("build bin");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        write_build_script(&build_script_path(&repo_root, "llama"));
        write_executable(
            &fake_llama_cli_path(&build_bin),
            "#!/bin/sh\nexit 0\n",
            "@echo off\nexit /b 0\n",
        );

        let toolchain = Toolchain::discover(&repo_root).expect("toolchain");

        assert_eq!(
            toolchain.model_source,
            ModelSource::HuggingFaceRepo(DEFAULT_MODEL_REPOSITORY.to_string())
        );
        assert_eq!(
            toolchain.cached_model_path,
            Some(
                repo_root
                    .join("models/llama/hf")
                    .join("ggml-org__gemma-3-4b-it-GGUF.gguf")
            )
        );
    }

    #[test]
    fn resolves_existing_relative_model_path_from_env() {
        let _guard = crate::test_support::env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/llama")
            .join(target_dir_name())
            .join("bin");
        let model_path = repo_root.join("models/llama/custom.gguf");
        fs::create_dir_all(&build_bin).expect("build bin");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        fs::create_dir_all(model_path.parent().expect("parent")).expect("models dir");
        fs::write(&model_path, "model").expect("model");
        write_build_script(&build_script_path(&repo_root, "llama"));
        write_executable(
            &fake_llama_cli_path(&build_bin),
            "#!/bin/sh\nexit 0\n",
            "@echo off\nexit /b 0\n",
        );

        unsafe { std::env::set_var(MODEL_ENV_VAR, "models/llama/custom.gguf") };
        let toolchain = Toolchain::discover(&repo_root).expect("toolchain");
        unsafe { std::env::remove_var(MODEL_ENV_VAR) };

        assert_eq!(toolchain.model_source, ModelSource::LocalPath(model_path));
        assert_eq!(toolchain.cached_model_path, None);
    }

    #[test]
    fn run_summary_generation_uses_preferred_backend_first() {
        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/llama")
            .join(target_dir_name())
            .join("bin");
        let prompt_file = repo_root.join("summary/prompt.txt");
        let output_file = repo_root.join("summary/output.txt");
        let log = repo_root.join("llama-summary.log");
        let model_path = repo_root.join("models/llama/local.gguf");
        fs::create_dir_all(&build_bin).expect("build bin");
        fs::create_dir_all(model_path.parent().expect("model parent")).expect("models dir");
        fs::create_dir_all(prompt_file.parent().expect("prompt parent")).expect("prompt dir");
        fs::write(&model_path, "model").expect("model");
        fs::write(&prompt_file, "prompt").expect("prompt");
        write_executable(
            &fake_llama_cli_path(&build_bin),
            &format!(
                "#!/bin/sh\n: > '{log}'\nfor arg in \"$@\"; do\n  printf 'ARG=%s\\n' \"$arg\" >> '{log}'\ndone\nprintf 'GGML_METAL=%s\\n' \"${{GGML_METAL-}}\" >> '{log}'\nprintf 'GGML_METAL_DEVICES=%s\\n' \"${{GGML_METAL_DEVICES-}}\" >> '{log}'\nprintf 'synthetic summary'\n",
                log = log.display()
            ),
            &format!(
                "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\n> \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto after\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo ARG=!arg!\nshift\ngoto loop\n:after\n>> \"{log}\" echo GGML_METAL=%GGML_METAL%\n>> \"{log}\" echo GGML_METAL_DEVICES=%GGML_METAL_DEVICES%\n<nul set /p =synthetic summary\nexit /b 0\n",
                log = log.display()
            ),
        );

        let toolchain = Toolchain {
            llama_cli_path: fake_llama_cli_path(&build_bin),
            llama_embedding_path: fake_command_path(build_bin.clone(), "llama-embedding"),
            build_script_path: build_script_path(&repo_root, "llama"),
            model_source: ModelSource::LocalPath(model_path.clone()),
            cached_model_path: None,
        };

        run_summary_generation(&toolchain, &prompt_file, &output_file).expect("summary");

        assert_eq!(
            fs::read_to_string(output_file).expect("summary output"),
            "synthetic summary"
        );

        let log = fs::read_to_string(log).expect("llama log");
        assert!(log.contains("ARG=--single-turn"));
        assert!(log.contains("ARG=-m"));
        assert!(log.contains(&format!("ARG={}", model_path.display())));
        assert!(!log.contains("ARG=-ngl"));
        assert!(!log.contains("ARG=--device"));
        assert!(!log.contains("ARG=none"));
        assert!(!log.contains("ARG=--no-op-offload"));
        assert!(!log.contains("ARG=--no-kv-offload"));
        assert!(!log.contains("ARG=--no-mmproj-offload"));
        assert!(!log.contains("GGML_METAL=0"));
        assert!(!log.contains("GGML_METAL_DEVICES=0"));
    }

    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn run_summary_generation_retries_on_cpu_after_backend_failure() {
        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/llama")
            .join(target_dir_name())
            .join("bin");
        let prompt_file = repo_root.join("summary/prompt.txt");
        let output_file = repo_root.join("summary/output.txt");
        let log = repo_root.join("llama-retry.log");
        let count = repo_root.join("llama-retry.count");
        let model_path = repo_root.join("models/llama/local.gguf");
        let failure_marker = if cfg!(target_os = "macos") {
            "metal backend failure"
        } else {
            "cuda backend failure"
        };
        fs::create_dir_all(&build_bin).expect("build bin");
        fs::create_dir_all(model_path.parent().expect("model parent")).expect("models dir");
        fs::create_dir_all(prompt_file.parent().expect("prompt parent")).expect("prompt dir");
        fs::write(&model_path, "model").expect("model");
        fs::write(&prompt_file, "prompt").expect("prompt");
        write_executable(
            &fake_llama_cli_path(&build_bin),
            &format!(
                "#!/bin/sh\ncount=0\nif [ -f '{count}' ]; then\n  count=$(cat '{count}')\nfi\ncount=$((count + 1))\nprintf '%s' \"$count\" > '{count}'\ncpu='0'\nlog='{log}'\nprintf 'CALL=%s\\n' \"$count\" >> \"$log\"\nprintf 'GGML_METAL=%s\\n' \"${{GGML_METAL-}}\" >> \"$log\"\nprintf 'GGML_METAL_DEVICES=%s\\n' \"${{GGML_METAL_DEVICES-}}\" >> \"$log\"\nfor arg in \"$@\"; do\n  printf 'CALL_%s_ARG=%s\\n' \"$count\" \"$arg\" >> \"$log\"\n  case \"$arg\" in\n    -ngl|--device|none|--no-op-offload|--no-kv-offload|--no-mmproj-offload)\n      cpu='1'\n      ;;\n  esac\ndone\nif [ \"$count\" = '1' ]; then\n  if [ \"$cpu\" = '1' ]; then\n    printf 'unexpected cpu fallback on first attempt\\n' >&2\n    exit 2\n  fi\n  printf 'error: {failure}\\n' >&2\n  exit 1\nfi\nif [ \"$cpu\" != '1' ]; then\n  printf 'error: second attempt still used gpu backend\\n' >&2\n  exit 3\nfi\nprintf 'cpu summary'\n",
                count = count.display(),
                log = log.display(),
                failure = failure_marker,
            ),
            &format!(
                "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nset \"count=0\"\nif exist \"{count}\" set /p count=<\"{count}\"\nset /a count+=1\n> \"{count}\" <nul set /p =!count!\nset \"cpu=0\"\n>> \"{log}\" echo CALL=!count!\n>> \"{log}\" echo GGML_METAL=%GGML_METAL%\n>> \"{log}\" echo GGML_METAL_DEVICES=%GGML_METAL_DEVICES%\n:loop\nif \"%~1\"==\"\" goto after\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo CALL_!count!_ARG=!arg!\nif /I \"!arg!\"==\"-ngl\" set \"cpu=1\"\nif /I \"!arg!\"==\"--device\" set \"cpu=1\"\nif /I \"!arg!\"==\"none\" set \"cpu=1\"\nif /I \"!arg!\"==\"--no-op-offload\" set \"cpu=1\"\nif /I \"!arg!\"==\"--no-kv-offload\" set \"cpu=1\"\nif /I \"!arg!\"==\"--no-mmproj-offload\" set \"cpu=1\"\nshift\ngoto loop\n:after\nif \"!count!\"==\"1\" (\n  if \"!cpu!\"==\"1\" (\n    echo unexpected cpu fallback on first attempt 1>&2\n    exit /b 2\n  )\n  echo error: {failure} 1>&2\n  exit /b 1\n)\nif not \"!cpu!\"==\"1\" (\n  echo error: second attempt still used gpu backend 1>&2\n  exit /b 3\n)\n<nul set /p =cpu summary\nexit /b 0\n",
                count = count.display(),
                log = log.display(),
                failure = failure_marker,
            ),
        );

        let toolchain = Toolchain {
            llama_cli_path: fake_llama_cli_path(&build_bin),
            llama_embedding_path: fake_command_path(build_bin.clone(), "llama-embedding"),
            build_script_path: build_script_path(&repo_root, "llama"),
            model_source: ModelSource::LocalPath(model_path),
            cached_model_path: None,
        };

        run_summary_generation(&toolchain, &prompt_file, &output_file).expect("summary");

        assert_eq!(
            fs::read_to_string(output_file).expect("summary output"),
            "cpu summary"
        );

        let log = fs::read_to_string(log).expect("retry log");
        assert!(log.contains("CALL=1"));
        assert!(log.contains("CALL=2"));
        assert!(!log.contains("CALL_1_ARG=-ngl"));
        assert!(log.contains("CALL_2_ARG=-ngl"));
        assert!(log.contains("CALL_2_ARG=0"));
        assert!(log.contains("CALL_2_ARG=--device"));
        assert!(log.contains("CALL_2_ARG=none"));
        assert!(log.contains("CALL_2_ARG=--no-op-offload"));
        assert!(log.contains("CALL_2_ARG=--no-kv-offload"));
        assert!(log.contains("CALL_2_ARG=--no-mmproj-offload"));
        if cfg!(target_os = "macos") {
            assert!(log.contains("GGML_METAL=0"));
            assert!(log.contains("GGML_METAL_DEVICES=0"));
        }
    }

    #[test]
    fn downloads_missing_hugging_face_model_to_repo_cache() {
        let _guard = crate::test_support::env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        unsafe { std::env::remove_var(MODEL_ENV_VAR) };

        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/llama")
            .join(target_dir_name())
            .join("bin");
        let llama_log = repo_root.join("llama-download.log");
        fs::create_dir_all(&build_bin).expect("build bin");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        write_build_script(&build_script_path(&repo_root, "llama"));
        write_executable(
            &fake_llama_cli_path(&build_bin),
            &format!(
                "#!/bin/sh\n: > '{log}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\ndone\nprintf 'LLAMA_CACHE=%s\\n' \"${{LLAMA_CACHE:-}}\" >> '{log}'\nmkdir -p \"$LLAMA_CACHE\"\nprintf 'synthetic model' > \"$LLAMA_CACHE/downloaded-model.gguf\"\n",
                log = llama_log.display()
            ),
            &format!(
                "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\n> \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto after\n>> \"{log}\" echo %~1\nshift\ngoto loop\n:after\n>> \"{log}\" echo LLAMA_CACHE=!LLAMA_CACHE!\nif not exist \"!LLAMA_CACHE!\" mkdir \"!LLAMA_CACHE!\"\n> \"!LLAMA_CACHE!\\downloaded-model.gguf\" <nul set /p =synthetic model\nexit /b 0\n",
                log = llama_log.display()
            ),
        );

        let toolchain = Toolchain::discover(&repo_root).expect("toolchain");
        toolchain.ensure_model().expect("model download");

        let cache_path = toolchain
            .cached_model_path
            .clone()
            .expect("cached model path");
        assert!(cache_path.is_file());
        assert_eq!(
            toolchain.runtime_model_source(),
            ModelSource::LocalPath(cache_path.clone())
        );

        let log = fs::read_to_string(llama_log).expect("llama log");
        assert!(log.contains("-hf"));
        assert!(log.contains(DEFAULT_MODEL_REPOSITORY));
        assert!(log.contains("LLAMA_CACHE="));
    }

    #[test]
    fn reports_missing_llama_toolchain() {
        let repo_root = temp_workspace();
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");

        let error = Toolchain::discover(&repo_root).expect_err("toolchain should fail");

        assert!(
            error.contains(
                build_script_path(&repo_root, "llama")
                    .to_string_lossy()
                    .as_ref()
            )
        );
    }

    #[test]
    fn extracts_summary_text_from_cli_stdout_with_truncated_prompt_echo() {
        let stdout = "Loading model...\n\n> 당신은 회의 녹취를 정리하는 한국어 회의록 작성 도우미다.\n- 출력은 반드시 한국어 Markdown으로 작� ... (truncated)\n\n## 회의록\n\n**개요**\n내용\n\n**핵심 논의**\n항목\n\n[ Prompt: 10.0 t/s | Generation: 20.0 t/s ]\n\nExiting...\n";

        let extracted = extract_summary_text(stdout, "길어서 일치하지 않는 원본 프롬프트");

        assert_eq!(extracted, "## 개요\n내용\n\n## 핵심 논의\n항목");
    }

    #[test]
    fn extracts_markdown_summary_without_meeting_title() {
        let stdout = "Assistant preface\n\n## 개요\n내용\n\n## 핵심 논의\n항목";

        let extracted = extract_summary_text(stdout, "");

        assert_eq!(extracted, "## 개요\n내용\n\n## 핵심 논의\n항목");
    }

    fn temp_workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!("recordroute-llama-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).expect("temp workspace");
        path
    }

    fn fake_llama_cli_path(build_bin: &Path) -> PathBuf {
        crate::ffmpeg::fake_command_path(build_bin, "llama-cli")
    }

    fn write_build_script(path: &Path) {
        write_executable(path, "#!/bin/sh\nexit 0\n", "@echo off\nexit /b 0\n");
    }

    fn write_executable(path: &Path, unix_content: &str, windows_content: &str) {
        let content = if cfg!(windows) {
            windows_content.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            unix_content.to_string()
        };
        fs::write(path, content).expect("write executable");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("permissions");
        }
    }
}
