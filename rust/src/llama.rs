use crate::ffmpeg::target_dir_name;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const MODEL_ENV_VAR: &str = "RECORDROUTE_LLAMA_MODEL";
const DEFAULT_MODEL_REPOSITORY: &str = "ggml-org/gemma-3-4b-it-GGUF";
const DEFAULT_PREDICT_TOKENS: &str = "1024";
const HF_CACHE_RELATIVE_DIR: &str = "models/llama/hf";

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
            command.arg("-hf").arg(repo);
        }
    }

    let output = command
        .arg("-f")
        .arg(prompt_file)
        .arg("-o")
        .arg(output_file)
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

    if output_file.is_file() {
        return Ok(());
    }

    Err(format!(
        "llama-cli completed without creating summary {}",
        output_file.display()
    ))
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

    let output = Command::new(&toolchain.llama_cli_path)
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
        .arg("-m")
        .arg(cache_path)
        .arg("-p")
        .arg("")
        .output()
        .map_err(|error| {
            format!(
                "failed to execute llama-cli {} for model download: {error}",
                toolchain.llama_cli_path.display()
            )
        })?;

    if output.status.success() && cache_path.is_file() {
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
                "#!/bin/sh\n: > '{log}'\nmodel=''\nnext=''\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\n  if [ \"$next\" = 'm' ]; then\n    model=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -m)\n      next='m'\n      ;;\n  esac\ndone\nmkdir -p \"$(dirname \"$model\")\"\nprintf 'synthetic model' > \"$model\"\n",
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
        assert!(log.contains("-m"));
        assert!(log.contains(cache_path.to_string_lossy().as_ref()));
    }

    #[test]
    fn reports_missing_llama_toolchain() {
        let repo_root = temp_workspace();
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");

        let error = Toolchain::discover(&repo_root).expect_err("toolchain should fail");

        assert!(error.contains("scripts/build_llama.sh"));
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
