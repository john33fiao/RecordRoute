use crate::ffmpeg::{build_script_path, locate_command, target_dir_name};
use crate::tool_runtime::{apply_cpu_fallback_env, command_output_details, should_retry_with_cpu};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const MODEL_ENV_VAR: &str = "RECORDROUTE_WHISPER_MODEL";
const DEFAULT_MODEL_RELATIVE_PATH: &str = "models/whisper/ggml-base.bin";
const MODEL_URL_TEMPLATE_ENV_VAR: &str = "RECORDROUTE_WHISPER_MODEL_URL_TEMPLATE";
const MODEL_SOURCE_DIR_ENV_VAR: &str = "RECORDROUTE_WHISPER_MODEL_SOURCE_DIR";
const DEFAULT_MODEL_URL_TEMPLATE: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{model}.bin";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toolchain {
    pub whisper_cli_path: PathBuf,
    pub build_script_path: PathBuf,
    pub model_path: PathBuf,
}

impl Toolchain {
    pub fn discover(repo_root: &Path) -> Result<Self, String> {
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

        Ok(Self {
            whisper_cli_path,
            build_script_path,
            model_path: resolve_model_path(repo_root),
        })
    }

    pub fn is_model_ready(&self) -> bool {
        self.model_path.is_file()
    }

    pub fn can_prepare_model(&self) -> Result<(), String> {
        if self.is_model_ready() {
            return Ok(());
        }

        let _ = managed_model_name(&self.model_path)?;
        let _ = model_directory(&self.model_path)?;
        Ok(())
    }

    pub fn ensure_model(&self) -> Result<(), String> {
        if self.is_model_ready() {
            return Ok(());
        }

        let model_name = managed_model_name(&self.model_path)?;
        let model_dir = model_directory(&self.model_path)?;
        download_model(self, &model_name, model_dir)
    }
}

pub fn run_transcription(
    toolchain: &Toolchain,
    input: &Path,
    output_text: &Path,
) -> Result<(), String> {
    if !input.is_file() {
        return Err(format!(
            "input audio file not found for transcription: {}",
            input.display()
        ));
    }

    let output_prefix = transcript_output_prefix(output_text)?;
    if let Some(parent) = output_text.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create transcription output directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let _ = fs::remove_file(output_text);

    match attempt_transcription(toolchain, input, &output_prefix) {
        Ok(()) => {}
        Err(error) if should_refresh_managed_model(toolchain, &error) => {
            refresh_managed_model(toolchain)?;
            attempt_transcription(toolchain, input, &output_prefix).map_err(|retry_error| {
                format!(
                    "{retry_error} (after refreshing managed model cache {})",
                    toolchain.model_path.display()
                )
            })?;
        }
        Err(error) => return Err(error),
    }

    if output_text.is_file() {
        postprocess_transcript(output_text)?;
        return Ok(());
    }

    Err(format!(
        "whisper-cli completed without creating transcript {}",
        output_text.display()
    ))
}

fn postprocess_transcript(output_text: &Path) -> Result<(), String> {
    let original = fs::read_to_string(output_text).map_err(|error| {
        format!(
            "failed to read transcript for post-processing {}: {error}",
            output_text.display()
        )
    })?;
    let deduplicated = deduplicate_consecutive_lines(&original);
    if deduplicated == original {
        return Ok(());
    }

    fs::write(output_text, deduplicated).map_err(|error| {
        format!(
            "failed to write post-processed transcript {}: {error}",
            output_text.display()
        )
    })
}

fn deduplicate_consecutive_lines(content: &str) -> String {
    let mut deduplicated = String::with_capacity(content.len());
    let mut previous_line: Option<String> = None;

    for segment in content.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let normalized = line.trim_end_matches('\r').trim();
        let is_duplicate = !normalized.is_empty() && previous_line.as_deref() == Some(normalized);
        if is_duplicate {
            continue;
        }

        deduplicated.push_str(segment);
        previous_line = if normalized.is_empty() {
            None
        } else {
            Some(normalized.to_string())
        };
    }

    deduplicated
}

fn resolve_model_path(repo_root: &Path) -> PathBuf {
    match std::env::var_os(MODEL_ENV_VAR) {
        Some(path) if !path.is_empty() => {
            resolve_model_override_path(repo_root, PathBuf::from(path))
        }
        _ => repo_root.join(DEFAULT_MODEL_RELATIVE_PATH),
    }
}

fn attempt_transcription(
    toolchain: &Toolchain,
    input: &Path,
    output_prefix: &Path,
) -> Result<(), String> {
    match execute_transcription(
        toolchain,
        input,
        output_prefix,
        WhisperRuntimeBackend::Preferred,
    ) {
        Ok(()) => Ok(()),
        Err(primary_error) if should_retry_transcription_on_cpu(&primary_error) => {
            execute_transcription(
                toolchain,
                input,
                output_prefix,
                WhisperRuntimeBackend::CpuFallback,
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

fn execute_transcription(
    toolchain: &Toolchain,
    input: &Path,
    output_prefix: &Path,
    backend: WhisperRuntimeBackend,
) -> Result<(), String> {
    let mut command = Command::new(&toolchain.whisper_cli_path);
    command
        .arg("-m")
        .arg(&toolchain.model_path)
        .arg("-f")
        .arg(input)
        .arg("-l")
        .arg("auto")
        .arg("-otxt")
        .arg("-np")
        .arg("-of")
        .arg(output_prefix);
    configure_runtime_backend(&mut command, backend);

    let output = command.output().map_err(|error| {
        format!(
            "failed to execute whisper-cli {}: {error}",
            toolchain.whisper_cli_path.display()
        )
    })?;

    if output.status.success() {
        return Ok(());
    }

    Err(format!(
        "whisper transcription failed for {}: {}",
        input.display(),
        command_output_details(&output)
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WhisperRuntimeBackend {
    Preferred,
    CpuFallback,
}

fn configure_runtime_backend(command: &mut Command, backend: WhisperRuntimeBackend) {
    match backend {
        WhisperRuntimeBackend::CpuFallback => {
            command.arg("-ng");
            apply_cpu_fallback_env(command);
        }
        WhisperRuntimeBackend::Preferred => {}
    }
}

fn should_retry_transcription_on_cpu(error: &str) -> bool {
    should_retry_with_cpu(
        error,
        &[
            "metal",
            "ggml-metal",
            "ggml_metal",
            "mtl",
            "failed to initialize whisper context",
        ],
        &["cuda", "cublas", "ggml-cuda", "ggml_cuda", "nvidia"],
    )
}

fn should_refresh_managed_model(toolchain: &Toolchain, error: &str) -> bool {
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

fn refresh_managed_model(toolchain: &Toolchain) -> Result<(), String> {
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

fn download_model(toolchain: &Toolchain, model_name: &str, model_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(model_dir).map_err(|error| {
        format!(
            "failed to create whisper model directory {}: {error}",
            model_dir.display()
        )
    })?;

    if let Some(source_dir) = std::env::var_os(MODEL_SOURCE_DIR_ENV_VAR) {
        let source = PathBuf::from(source_dir).join(format!("ggml-{model_name}.bin"));
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

fn managed_model_name(model_path: &Path) -> Result<String, String> {
    infer_model_name(model_path).ok_or_else(|| {
        format!(
            "whisper model not found at {} and the configured path does not match ggml-<model>.bin. Update {} or use {}",
            model_path.display(),
            MODEL_ENV_VAR,
            DEFAULT_MODEL_RELATIVE_PATH
        )
    })
}

fn model_directory(model_path: &Path) -> Result<&Path, String> {
    model_path.parent().ok_or_else(|| {
        format!(
            "whisper model path has no parent directory: {}",
            model_path.display()
        )
    })
}

fn managed_repo_root(toolchain: &Toolchain) -> Option<PathBuf> {
    toolchain
        .build_script_path
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
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

    let normalized_name = normalized_model_file_name(file_name);
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

fn infer_model_name(model_path: &Path) -> Option<String> {
    let file_name = model_path.file_name()?.to_str()?;
    let suffix = ".bin";
    let prefix = "ggml-";

    file_name
        .strip_prefix(prefix)?
        .strip_suffix(suffix)
        .map(ToOwned::to_owned)
}

fn transcript_output_prefix(output_text: &Path) -> Result<PathBuf, String> {
    let parent = output_text.parent().ok_or_else(|| {
        format!(
            "transcript output path has no parent directory: {}",
            output_text.display()
        )
    })?;
    let stem = output_text.file_stem().ok_or_else(|| {
        format!(
            "transcript output path has no file stem: {}",
            output_text.display()
        )
    })?;

    Ok(parent.join(stem))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn discovers_toolchain_with_default_model_path() {
        let _guard = crate::test_support::env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        unsafe { std::env::remove_var(MODEL_ENV_VAR) };
        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/whisper")
            .join(target_dir_name())
            .join("bin");
        fs::create_dir_all(&build_bin).expect("build bin");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        write_build_script(&build_script_path(&repo_root, "whisper"));
        write_executable(
            &fake_whisper_cli_path(&build_bin),
            "#!/bin/sh\nexit 0\n",
            "@echo off\nexit /b 0\n",
        );

        let toolchain = Toolchain::discover(&repo_root).expect("toolchain");

        assert_eq!(
            toolchain.model_path,
            repo_root.join("models/whisper/ggml-base.bin")
        );
        assert_eq!(
            toolchain.whisper_cli_path,
            fake_whisper_cli_path(&build_bin)
        );
    }

    #[test]
    fn discover_uses_env_override_for_relative_model_path() {
        let _guard = crate::test_support::env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/whisper")
            .join(target_dir_name())
            .join("bin");
        fs::create_dir_all(&build_bin).expect("build bin");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        write_build_script(&build_script_path(&repo_root, "whisper"));
        write_executable(
            &fake_whisper_cli_path(&build_bin),
            "#!/bin/sh\nexit 0\n",
            "@echo off\nexit /b 0\n",
        );

        unsafe { std::env::set_var(MODEL_ENV_VAR, "custom/ggml-base.bin") };
        let toolchain = Toolchain::discover(&repo_root).expect("toolchain");
        unsafe { std::env::remove_var(MODEL_ENV_VAR) };

        assert_eq!(toolchain.model_path, repo_root.join("custom/ggml-base.bin"));
    }

    #[test]
    fn discover_normalizes_shorthand_relative_model_path() {
        let _guard = crate::test_support::env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/whisper")
            .join(target_dir_name())
            .join("bin");
        fs::create_dir_all(&build_bin).expect("build bin");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        write_build_script(&build_script_path(&repo_root, "whisper"));
        write_executable(
            &fake_whisper_cli_path(&build_bin),
            "#!/bin/sh\nexit 0\n",
            "@echo off\nexit /b 0\n",
        );

        unsafe { std::env::set_var(MODEL_ENV_VAR, "models/whisper/large-v3-turbo") };
        let toolchain = Toolchain::discover(&repo_root).expect("toolchain");
        unsafe { std::env::remove_var(MODEL_ENV_VAR) };

        assert_eq!(
            toolchain.model_path,
            repo_root.join("models/whisper/ggml-large-v3-turbo.bin")
        );
    }

    #[test]
    fn reports_missing_whisper_toolchain() {
        let repo_root = temp_workspace();
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");

        let error = Toolchain::discover(&repo_root).expect_err("toolchain should fail");

        assert!(
            error.contains(
                build_script_path(&repo_root, "whisper")
                    .to_string_lossy()
                    .as_ref()
            )
        );
    }

    #[test]
    fn downloads_missing_model_to_configured_location() {
        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/whisper")
            .join(target_dir_name())
            .join("bin");
        let source_dir = repo_root.join("source-models");
        fs::create_dir_all(&build_bin).expect("build bin");
        fs::create_dir_all(&source_dir).expect("models dir");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        write_build_script(&build_script_path(&repo_root, "whisper"));
        write_executable(
            &fake_whisper_cli_path(&build_bin),
            "#!/bin/sh\nexit 0\n",
            "@echo off\nexit /b 0\n",
        );
        fs::write(source_dir.join("ggml-base.bin"), "cached-model").expect("source model");

        unsafe { std::env::set_var(MODEL_SOURCE_DIR_ENV_VAR, &source_dir) };
        let toolchain = Toolchain::discover(&repo_root).expect("toolchain");
        toolchain.ensure_model().expect("model download");
        unsafe { std::env::remove_var(MODEL_SOURCE_DIR_ENV_VAR) };

        assert!(toolchain.model_path.is_file());
        assert_eq!(
            fs::read_to_string(&toolchain.model_path).expect("model content"),
            "cached-model"
        );
    }

    #[test]
    fn run_transcription_creates_text_output() {
        let _guard = crate::test_support::env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/whisper")
            .join(target_dir_name())
            .join("bin");
        let input = repo_root.join("sample.wav");
        let output = repo_root.join("stt/sample.txt");
        let log = repo_root.join("whisper.log");
        fs::create_dir_all(&build_bin).expect("build bin");
        write_test_audio(&input);
        write_executable(
            &fake_whisper_cli_path(&build_bin),
            &format!(
                "#!/bin/sh\nnext=''\nout=''\nlog='{}'\n: > \"$log\"\nprintf 'GGML_METAL=%s\\n' \"${{GGML_METAL-}}\" >> \"$log\"\nprintf 'GGML_METAL_DEVICES=%s\\n' \"${{GGML_METAL_DEVICES-}}\" >> \"$log\"\nfor arg in \"$@\"; do\n  printf 'ARG=%s\\n' \"$arg\" >> \"$log\"\n  if [ \"$next\" = 'of' ]; then\n    out=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -of)\n      next='of'\n      ;;\n  esac\ndone\nmkdir -p \"$(dirname \"$out\")\"\nprintf 'synthetic transcript' > \"$out.txt\"\n",
                log.display()
            ),
            &format!(
                "@echo off\r\nsetlocal EnableExtensions EnableDelayedExpansion\r\nset \"out=\"\r\nset \"next=\"\r\n> \"{log}\" echo GGML_METAL=%GGML_METAL%\r\n>> \"{log}\" echo GGML_METAL_DEVICES=%GGML_METAL_DEVICES%\r\n:loop\r\nif \"%~1\"==\"\" goto done\r\n>> \"{log}\" echo ARG=%~1\r\nif /I \"!next!\"==\"of\" (\r\n  set \"out=%~1\"\r\n  set \"next=\"\r\n) else if /I \"%~1\"==\"-of\" (\r\n  set \"next=of\"\r\n)\r\nshift\r\ngoto loop\r\n:done\r\nif defined out (\r\n  for %%I in (\"!out!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\r\n  > \"!out!.txt\" <nul set /p =synthetic transcript\r\n)\r\nexit /b 0\r\n",
                log = log.display()
            ),
        );

        let toolchain = Toolchain {
            whisper_cli_path: fake_whisper_cli_path(&build_bin),
            build_script_path: build_script_path(&repo_root, "whisper"),
            model_path: repo_root.join("models/whisper/ggml-base.bin"),
        };
        fs::create_dir_all(repo_root.join("models/whisper")).expect("models dir");
        fs::write(&toolchain.model_path, "model").expect("model file");

        run_transcription(&toolchain, &input, &output).expect("transcription");

        assert_eq!(
            fs::read_to_string(output).expect("transcript"),
            "synthetic transcript"
        );

        let log = fs::read_to_string(log).expect("whisper log");
        assert!(log.contains("ARG=-m"));
        assert!(log.contains("ARG=-l"));
        assert!(log.contains("ARG=auto"));
        assert!(!log.contains("ARG=-ng"));
        assert!(!log.contains("GGML_METAL=0"));
        assert!(!log.contains("GGML_METAL_DEVICES=0"));
    }

    #[test]
    fn run_transcription_deduplicates_consecutive_transcript_lines() {
        let _guard = crate::test_support::env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/whisper")
            .join(target_dir_name())
            .join("bin");
        let input = repo_root.join("sample.wav");
        let output = repo_root.join("stt/sample.txt");
        fs::create_dir_all(&build_bin).expect("build bin");
        write_test_audio(&input);
        write_executable(
            &fake_whisper_cli_path(&build_bin),
            "#!/bin/sh\nnext=''\nout=''\nfor arg in \"$@\"; do\n  if [ \"$next\" = 'of' ]; then\n    out=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -of)\n      next='of'\n      ;;\n  esac\ndone\nmkdir -p \"$(dirname \"$out\")\"\nprintf 'intro\\nrepeat me\\nrepeat me\\n\\nrepeat me\\noutro\\n' > \"$out.txt\"\n",
            "@echo off\r\nsetlocal EnableExtensions EnableDelayedExpansion\r\nset \"out=\"\r\nset \"next=\"\r\n:loop\r\nif \"%~1\"==\"\" goto done\r\nif /I \"!next!\"==\"of\" (\r\n  set \"out=%~1\"\r\n  set \"next=\"\r\n) else if /I \"%~1\"==\"-of\" (\r\n  set \"next=of\"\r\n)\r\nshift\r\ngoto loop\r\n:done\r\nif defined out (\r\n  for %%I in (\"!out!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\r\n  > \"!out!.txt\" echo intro\r\n  >> \"!out!.txt\" echo repeat me\r\n  >> \"!out!.txt\" echo repeat me\r\n  >> \"!out!.txt\" echo.\r\n  >> \"!out!.txt\" echo repeat me\r\n  >> \"!out!.txt\" echo outro\r\n)\r\nexit /b 0\r\n",
        );

        let toolchain = Toolchain {
            whisper_cli_path: fake_whisper_cli_path(&build_bin),
            build_script_path: build_script_path(&repo_root, "whisper"),
            model_path: repo_root.join("models/whisper/ggml-base.bin"),
        };
        fs::create_dir_all(repo_root.join("models/whisper")).expect("models dir");
        fs::write(&toolchain.model_path, "model").expect("model file");

        run_transcription(&toolchain, &input, &output).expect("transcription");

        assert_eq!(
            fs::read_to_string(output)
                .expect("transcript")
                .replace("\r\n", "\n"),
            "intro\nrepeat me\n\nrepeat me\noutro\n"
        );
    }

    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn run_transcription_retries_on_cpu_after_backend_failure() {
        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/whisper")
            .join(target_dir_name())
            .join("bin");
        let input = repo_root.join("sample.wav");
        let output = repo_root.join("stt/sample.txt");
        let log = repo_root.join("whisper-retry.log");
        let count = repo_root.join("whisper-retry.count");
        let failure_marker = if cfg!(target_os = "macos") {
            "metal backend failure"
        } else {
            "cuda backend failure"
        };
        fs::create_dir_all(&build_bin).expect("build bin");
        write_test_audio(&input);
        write_executable(
            &fake_whisper_cli_path(&build_bin),
            &format!(
                "#!/bin/sh\ncount=0\nif [ -f '{count}' ]; then\n  count=$(cat '{count}')\nfi\ncount=$((count + 1))\nprintf '%s' \"$count\" > '{count}'\nnext=''\nout=''\ncpu='0'\nlog='{log}'\nprintf 'CALL=%s\\n' \"$count\" >> \"$log\"\nprintf 'GGML_METAL=%s\\n' \"${{GGML_METAL-}}\" >> \"$log\"\nprintf 'GGML_METAL_DEVICES=%s\\n' \"${{GGML_METAL_DEVICES-}}\" >> \"$log\"\nfor arg in \"$@\"; do\n  printf 'CALL_%s_ARG=%s\\n' \"$count\" \"$arg\" >> \"$log\"\n  if [ \"$next\" = 'of' ]; then\n    out=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -of)\n      next='of'\n      ;;\n    -ng)\n      cpu='1'\n      ;;\n  esac\ndone\nif [ \"$count\" = '1' ]; then\n  if [ \"$cpu\" = '1' ]; then\n    printf 'unexpected cpu fallback on first attempt\\n' >&2\n    exit 2\n  fi\n  printf 'error: {failure}\\n' >&2\n  exit 1\nfi\nif [ \"$cpu\" != '1' ]; then\n  printf 'error: second attempt still used gpu backend\\n' >&2\n  exit 3\nfi\nmkdir -p \"$(dirname \"$out\")\"\nprintf 'cpu fallback transcript' > \"$out.txt\"\n",
                count = count.display(),
                log = log.display(),
                failure = failure_marker,
            ),
            &format!(
                "@echo off\r\nsetlocal EnableExtensions EnableDelayedExpansion\r\nset \"count=0\"\r\nif exist \"{count}\" set /p count=<\"{count}\"\r\nset /a count+=1\r\n> \"{count}\" <nul set /p =!count!\r\nset \"out=\"\r\nset \"next=\"\r\nset \"cpu=0\"\r\n>> \"{log}\" echo CALL=!count!\r\n>> \"{log}\" echo GGML_METAL=%GGML_METAL%\r\n>> \"{log}\" echo GGML_METAL_DEVICES=%GGML_METAL_DEVICES%\r\n:loop\r\nif \"%~1\"==\"\" goto after\r\nset \"arg=%~1\"\r\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\r\n>> \"{log}\" echo CALL_!count!_ARG=!arg!\r\nif /I \"!next!\"==\"of\" (\r\n  set \"out=!arg!\"\r\n  set \"next=\"\r\n) else (\r\n  if /I \"!arg!\"==\"-of\" set \"next=of\"\r\n  if /I \"!arg!\"==\"-ng\" set \"cpu=1\"\r\n)\r\nshift\r\ngoto loop\r\n:after\r\nif \"!count!\"==\"1\" (\r\n  if \"!cpu!\"==\"1\" (\r\n    echo unexpected cpu fallback on first attempt 1>&2\r\n    exit /b 2\r\n  )\r\n  echo error: {failure} 1>&2\r\n  exit /b 1\r\n)\r\nif not \"!cpu!\"==\"1\" (\r\n  echo error: second attempt still used gpu backend 1>&2\r\n  exit /b 3\r\n)\r\nif defined out (\r\n  for %%I in (\"!out!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\r\n  > \"!out!.txt\" <nul set /p =cpu fallback transcript\r\n)\r\nexit /b 0\r\n",
                count = count.display(),
                log = log.display(),
                failure = failure_marker,
            ),
        );

        let toolchain = Toolchain {
            whisper_cli_path: fake_whisper_cli_path(&build_bin),
            build_script_path: build_script_path(&repo_root, "whisper"),
            model_path: repo_root.join("models/whisper/ggml-base.bin"),
        };
        fs::create_dir_all(repo_root.join("models/whisper")).expect("models dir");
        fs::write(&toolchain.model_path, "model").expect("model file");

        run_transcription(&toolchain, &input, &output).expect("transcription");

        assert_eq!(
            fs::read_to_string(output).expect("transcript"),
            "cpu fallback transcript"
        );

        let log = fs::read_to_string(log).expect("retry log");
        assert!(log.contains("CALL=1"));
        assert!(log.contains("CALL=2"));
        assert!(!log.contains("CALL_1_ARG=-ng"));
        assert!(log.contains("CALL_2_ARG=-ng"));
        if cfg!(target_os = "macos") {
            assert!(log.contains("GGML_METAL=0"));
            assert!(log.contains("GGML_METAL_DEVICES=0"));
        }
    }

    #[test]
    fn run_transcription_refreshes_invalid_managed_model_once() {
        let _guard = crate::test_support::env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/whisper")
            .join(target_dir_name())
            .join("bin");
        let input = repo_root.join("sample.wav");
        let output = repo_root.join("stt/sample.txt");
        let model_path = repo_root.join("models/whisper/ggml-base.bin");
        fs::create_dir_all(&build_bin).expect("build bin");
        let source_dir = repo_root.join("source-models");
        fs::create_dir_all(&source_dir).expect("download dir");
        fs::create_dir_all(repo_root.join("models/whisper")).expect("model dir");
        write_test_audio(&input);
        fs::write(&model_path, "broken").expect("broken model");
        fs::write(source_dir.join("ggml-base.bin"), "healthy").expect("healthy model");
        write_executable(
            &fake_whisper_cli_path(&build_bin),
            &format!(
                "#!/bin/sh\nnext=''\nout=''\nfor arg in \"$@\"; do\n  if [ \"$next\" = 'of' ]; then\n    out=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -of)\n      next='of'\n      ;;\n  esac\ndone\nif [ \"$(cat '{}')\" != 'healthy' ]; then\n  printf 'error: failed to initialize whisper context\\n' >&2\n  exit 1\nfi\nmkdir -p \"$(dirname \"$out\")\"\nprintf 'recovered transcript' > \"$out.txt\"\n",
                model_path.display()
            ),
            &format!(
                "@echo off\r\nsetlocal EnableExtensions EnableDelayedExpansion\r\nset \"out=\"\r\nset \"next=\"\r\nfor /f \"usebackq delims=\" %%A in (\"{model}\") do set \"model_state=%%A\"\r\nif /I not \"!model_state!\"==\"healthy\" (\r\necho error: failed to initialize whisper context 1>&2\r\nexit /b 1\r\n)\r\n:loop\r\nif \"%~1\"==\"\" goto done\r\nif /I \"!next!\"==\"of\" (\r\n  set \"out=%~1\"\r\n  set \"next=\"\r\n) else if /I \"%~1\"==\"-of\" (\r\n  set \"next=of\"\r\n)\r\nshift\r\ngoto loop\r\n:done\r\nif defined out (\r\n  for %%I in (\"!out!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\r\n  > \"!out!.txt\" <nul set /p =recovered transcript\r\n)\r\nexit /b 0\r\n",
                model = model_path.display()
            ),
        );
        let toolchain = Toolchain {
            whisper_cli_path: fake_whisper_cli_path(&build_bin),
            build_script_path: build_script_path(&repo_root, "whisper"),
            model_path,
        };

        unsafe { std::env::set_var(MODEL_SOURCE_DIR_ENV_VAR, &source_dir) };
        run_transcription(&toolchain, &input, &output).expect("transcription");
        unsafe { std::env::remove_var(MODEL_SOURCE_DIR_ENV_VAR) };

        assert_eq!(
            fs::read_to_string(output).expect("transcript"),
            "recovered transcript"
        );
        assert_eq!(
            fs::read_to_string(&toolchain.model_path).expect("refreshed model"),
            "healthy"
        );
    }

    fn temp_workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!("recordroute-whisper-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).expect("temp workspace");
        path
    }

    fn fake_whisper_cli_path(build_bin: &Path) -> PathBuf {
        crate::ffmpeg::fake_command_path(build_bin, "whisper-cli")
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
        fs::write(path, content).expect("script");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut perms = fs::metadata(path).expect("metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms).expect("permissions");
        }
    }

    fn write_test_audio(path: &Path) {
        fs::write(path, b"RIFFsyntheticWAVE").expect("audio file");
    }
}
