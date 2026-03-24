use crate::ffmpeg::target_dir_name;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

pub const MODEL_ENV_VAR: &str = "RECORDROUTE_LLAMA_MODEL";
const DEFAULT_MODEL_REPOSITORY: &str = "ggml-org/gemma-3-4b-it-GGUF";
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
    pub build_script_path: PathBuf,
    pub model_source: ModelSource,
    pub cached_model_path: Option<PathBuf>,
}

impl Toolchain {
    pub fn discover(repo_root: &Path) -> Result<Self, String> {
        let build_script_path = repo_root.join("scripts/build_llama.sh");
        let llama_cli_path = repo_root
            .join(".build/llama")
            .join(target_dir_name())
            .join("bin")
            .join(executable_name("llama-cli"));

        if !llama_cli_path.is_file() {
            return Err(format!(
                "local llama toolchain not found. Build it first with {}",
                build_script_path.display()
            ));
        }

        let model_source = resolve_model_source(repo_root);
        let cached_model_path = match &model_source {
            ModelSource::LocalPath(_) => None,
            ModelSource::HuggingFaceRepo(repo) => Some(cached_model_path(repo_root, repo)),
        };

        Ok(Self {
            llama_cli_path,
            build_script_path,
            model_source,
            cached_model_path,
        })
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

    let mut command = Command::new(&toolchain.llama_cli_path);
    command
        .arg("--single-turn")
        .arg("--simple-io")
        .arg("--no-display-prompt")
        .arg("--log-disable")
        .arg("-n")
        .arg(DEFAULT_PREDICT_TOKENS);

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

    if !output.status.success() {
        let _ = fs::remove_file(output_file);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(format!(
            "llama summary generation failed for {}: {}",
            prompt_file.display(),
            if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                "unknown error".to_string()
            }
        ));
    }

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

    let mut child = Command::new(&toolchain.llama_cli_path)
        .env(LLAMA_CACHE_ENV_VAR, &download_cache_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--single-turn")
        .arg("--simple-io")
        .arg("--no-display-prompt")
        .arg("--log-disable")
        .arg("--no-warmup")
        .arg("--no-mmproj")
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

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let details = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "unknown error".to_string()
    };

    Err(format!(
        "failed to download llama model {repo} to {}: {details}",
        cache_path.display()
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
    [
        "\n## 회의록",
        "\n**개요**",
        "\n개요",
        "\n**핵심 논의**",
        "\n핵심 논의",
    ]
    .iter()
    .filter_map(|marker| text.find(marker).map(|index| index + 1))
    .min()
}

fn normalize_summary_text(text: &str) -> String {
    let mut normalized = text.trim().to_string();

    for heading in ["개요", "핵심 논의", "결정/합의", "후속 조치"] {
        normalized = normalized.replace(&format!("**{heading}**"), heading);
    }

    for prefix in ["## 회의록\n\n", "# 회의록\n\n", "회의록\n\n"] {
        if let Some(stripped) = normalized.strip_prefix(prefix) {
            normalized = stripped.trim_start().to_string();
            break;
        }
    }

    normalized
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

fn executable_name(name: &str) -> OsString {
    if cfg!(windows) {
        OsString::from(format!("{name}.exe"))
    } else {
        OsString::from(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn discovers_toolchain_with_default_hugging_face_model() {
        let _guard = crate::test_support::env_lock().lock().expect("env lock");
        unsafe { std::env::remove_var(MODEL_ENV_VAR) };

        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/llama")
            .join(target_dir_name())
            .join("bin");
        fs::create_dir_all(&build_bin).expect("build bin");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        write_executable(
            &build_bin.join(executable_name("llama-cli")),
            "#!/bin/sh\nexit 0\n",
        );
        fs::write(
            repo_root.join("scripts/build_llama.sh"),
            "#!/bin/sh\nexit 0\n",
        )
        .expect("build script");

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
        let _guard = crate::test_support::env_lock().lock().expect("env lock");

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
        write_executable(
            &build_bin.join(executable_name("llama-cli")),
            "#!/bin/sh\nexit 0\n",
        );
        fs::write(
            repo_root.join("scripts/build_llama.sh"),
            "#!/bin/sh\nexit 0\n",
        )
        .expect("build script");

        unsafe { std::env::set_var(MODEL_ENV_VAR, "models/llama/custom.gguf") };
        let toolchain = Toolchain::discover(&repo_root).expect("toolchain");
        unsafe { std::env::remove_var(MODEL_ENV_VAR) };

        assert_eq!(toolchain.model_source, ModelSource::LocalPath(model_path));
        assert_eq!(toolchain.cached_model_path, None);
    }

    #[test]
    fn downloads_missing_hugging_face_model_to_repo_cache() {
        let _guard = crate::test_support::env_lock().lock().expect("env lock");
        unsafe { std::env::remove_var(MODEL_ENV_VAR) };

        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/llama")
            .join(target_dir_name())
            .join("bin");
        let llama_log = repo_root.join("llama-download.log");
        fs::create_dir_all(&build_bin).expect("build bin");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        write_executable(
            &build_bin.join(executable_name("llama-cli")),
            &format!(
                "#!/bin/sh\n: > '{log}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\ndone\nprintf 'LLAMA_CACHE=%s\\n' \"${{LLAMA_CACHE:-}}\" >> '{log}'\nmkdir -p \"$LLAMA_CACHE\"\nprintf 'synthetic model' > \"$LLAMA_CACHE/downloaded-model.gguf\"\n",
                log = llama_log.display()
            ),
        );
        fs::write(
            repo_root.join("scripts/build_llama.sh"),
            "#!/bin/sh\nexit 0\n",
        )
        .expect("build script");

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

        assert!(error.contains("scripts/build_llama.sh"));
    }

    #[test]
    fn extracts_summary_text_from_cli_stdout_with_truncated_prompt_echo() {
        let stdout = "Loading model...\n\n> 당신은 회의 녹취를 정리하는 한국어 회의록 작성 도우미다.\n- 출력은 반드시 한국어 평문으� ... (truncated)\n\n## 회의록\n\n**개요**\n내용\n\n**핵심 논의**\n항목\n\n[ Prompt: 10.0 t/s | Generation: 20.0 t/s ]\n\nExiting...\n";

        let extracted = extract_summary_text(stdout, "길어서 일치하지 않는 원본 프롬프트");

        assert_eq!(extracted, "개요\n내용\n\n핵심 논의\n항목");
    }

    fn temp_workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!("recordroute-llama-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).expect("temp workspace");
        path
    }

    fn write_executable(path: &Path, contents: &str) {
        fs::write(path, contents).expect("write executable");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("permissions");
        }
    }
}
