use crate::ffmpeg::{build_script_path, locate_command, target_dir_name};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const MODEL_ENV_VAR: &str = "RECORDROUTE_WHISPER_MODEL";
const DEFAULT_MODEL_RELATIVE_PATH: &str = "models/whisper/ggml-base.bin";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toolchain {
    pub whisper_cli_path: PathBuf,
    pub build_script_path: PathBuf,
    pub download_script_path: PathBuf,
    pub model_path: PathBuf,
}

impl Toolchain {
    pub fn discover(repo_root: &Path) -> Result<Self, String> {
        let build_script_path = build_script_path(repo_root, "whisper");
        let download_script_path = download_script_path(repo_root);
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
            download_script_path,
            model_path: resolve_model_path(repo_root),
        })
    }

    pub fn ensure_model(&self) -> Result<(), String> {
        if self.model_path.is_file() {
            return Ok(());
        }

        if !self.download_script_path.is_file() {
            return Err(format!(
                "whisper model not found at {} and download script is missing: {}",
                self.model_path.display(),
                self.download_script_path.display()
            ));
        }

        let model_name = infer_model_name(&self.model_path).ok_or_else(|| {
            format!(
                "whisper model not found at {} and the configured path does not match ggml-<model>.bin. Update {} or use {}",
                self.model_path.display(),
                MODEL_ENV_VAR,
                DEFAULT_MODEL_RELATIVE_PATH
            )
        })?;
        let model_dir = self.model_path.parent().ok_or_else(|| {
            format!(
                "whisper model path has no parent directory: {}",
                self.model_path.display()
            )
        })?;

        fs::create_dir_all(model_dir).map_err(|error| {
            format!(
                "failed to create whisper model directory {}: {error}",
                model_dir.display()
            )
        })?;

        let output = Command::new(&self.download_script_path)
            .arg(&model_name)
            .arg(model_dir)
            .output()
            .map_err(|error| {
                format!(
                    "failed to execute whisper model download script {}: {error}",
                    self.download_script_path.display()
                )
            })?;

        if output.status.success() && self.model_path.is_file() {
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
            "failed to download whisper model {} to {}: {}",
            model_name,
            self.model_path.display(),
            details
        ))
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

    let output = Command::new(&toolchain.whisper_cli_path)
        .arg("-m")
        .arg(&toolchain.model_path)
        .arg("-f")
        .arg(input)
        .arg("-l")
        .arg("auto")
        .arg("-otxt")
        .arg("-np")
        .arg("-of")
        .arg(&output_prefix)
        .output()
        .map_err(|error| {
            format!(
                "failed to execute whisper-cli {}: {error}",
                toolchain.whisper_cli_path.display()
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(format!(
            "whisper transcription failed for {}: {}",
            input.display(),
            if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                "unknown error".to_string()
            }
        ));
    }

    if output_text.is_file() {
        return Ok(());
    }

    Err(format!(
        "whisper-cli completed without creating transcript {}",
        output_text.display()
    ))
}

fn resolve_model_path(repo_root: &Path) -> PathBuf {
    match std::env::var_os(MODEL_ENV_VAR) {
        Some(path) if !path.is_empty() => {
            resolve_model_override_path(repo_root, PathBuf::from(path))
        }
        _ => repo_root.join(DEFAULT_MODEL_RELATIVE_PATH),
    }
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

fn download_script_path(repo_root: &Path) -> PathBuf {
    repo_root
        .join("whisper.cpp/models")
        .join(download_script_name())
}

fn download_script_name() -> &'static str {
    if cfg!(windows) {
        "download-ggml-model.cmd"
    } else {
        "download-ggml-model.sh"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn discovers_toolchain_with_default_model_path() {
        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/whisper")
            .join(target_dir_name())
            .join("bin");
        fs::create_dir_all(&build_bin).expect("build bin");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        fs::create_dir_all(repo_root.join("whisper.cpp/models")).expect("models dir");
        write_build_script(&build_script_path(&repo_root, "whisper"));
        write_executable(
            &fake_whisper_cli_path(&build_bin),
            "#!/bin/sh\nexit 0\n",
            "@echo off\nexit /b 0\n",
        );
        write_executable(
            &download_script_path(&repo_root),
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
        fs::create_dir_all(repo_root.join("whisper.cpp/models")).expect("models dir");
        write_build_script(&build_script_path(&repo_root, "whisper"));
        write_executable(
            &fake_whisper_cli_path(&build_bin),
            "#!/bin/sh\nexit 0\n",
            "@echo off\nexit /b 0\n",
        );
        write_executable(
            &download_script_path(&repo_root),
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
        fs::create_dir_all(repo_root.join("whisper.cpp/models")).expect("models dir");
        write_build_script(&build_script_path(&repo_root, "whisper"));
        write_executable(
            &fake_whisper_cli_path(&build_bin),
            "#!/bin/sh\nexit 0\n",
            "@echo off\nexit /b 0\n",
        );
        write_executable(
            &download_script_path(&repo_root),
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
        let download_log = repo_root.join("download.log");
        fs::create_dir_all(&build_bin).expect("build bin");
        fs::create_dir_all(repo_root.join("whisper.cpp/models")).expect("models dir");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        write_build_script(&build_script_path(&repo_root, "whisper"));
        write_executable(
            &fake_whisper_cli_path(&build_bin),
            "#!/bin/sh\nexit 0\n",
            "@echo off\nexit /b 0\n",
        );
        write_executable(
            &download_script_path(&repo_root),
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nmkdir -p \"$2\"\n: > \"$2/ggml-$1.bin\"\n",
                download_log.display()
            ),
            &format!(
                "@echo off\r\nsetlocal EnableExtensions\r\n> \"{log}\" echo %1 %2\r\nif not exist \"%~2\" mkdir \"%~2\"\r\n> \"%~2\\ggml-%~1.bin\" type nul\r\nexit /b 0\r\n",
                log = download_log.display()
            ),
        );

        let toolchain = Toolchain::discover(&repo_root).expect("toolchain");
        toolchain.ensure_model().expect("model download");

        assert!(toolchain.model_path.is_file());
        let log = fs::read_to_string(download_log).expect("download log");
        assert!(log.contains("base"));
    }

    #[test]
    fn run_transcription_creates_text_output() {
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
            "#!/bin/sh\nnext=''\nout=''\nfor arg in \"$@\"; do\n  if [ \"$next\" = 'of' ]; then\n    out=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -of)\n      next='of'\n      ;;\n  esac\ndone\nmkdir -p \"$(dirname \"$out\")\"\nprintf 'synthetic transcript' > \"$out.txt\"\n",
            "@echo off\r\nsetlocal EnableExtensions EnableDelayedExpansion\r\nset \"out=\"\r\nset \"next=\"\r\n:loop\r\nif \"%~1\"==\"\" goto done\r\nif /I \"!next!\"==\"of\" (\r\n  set \"out=%~1\"\r\n  set \"next=\"\r\n) else if /I \"%~1\"==\"-of\" (\r\n  set \"next=of\"\r\n)\r\nshift\r\ngoto loop\r\n:done\r\nif defined out (\r\n  for %%I in (\"!out!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\r\n  > \"!out!.txt\" <nul set /p =synthetic transcript\r\n)\r\nexit /b 0\r\n",
        );

        let toolchain = Toolchain {
            whisper_cli_path: fake_whisper_cli_path(&build_bin),
            build_script_path: build_script_path(&repo_root, "whisper"),
            download_script_path: download_script_path(&repo_root),
            model_path: repo_root.join("models/whisper/ggml-base.bin"),
        };
        fs::create_dir_all(repo_root.join("models/whisper")).expect("models dir");
        fs::write(&toolchain.model_path, "model").expect("model file");

        run_transcription(&toolchain, &input, &output).expect("transcription");

        assert_eq!(
            fs::read_to_string(output).expect("transcript"),
            "synthetic transcript"
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
