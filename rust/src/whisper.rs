use crate::ffmpeg::target_dir_name;
use std::ffi::OsString;
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
        let build_script_path = repo_root.join("scripts/build_whisper.sh");
        let download_script_path = repo_root.join("whisper.cpp/models/download-ggml-model.sh");
        let whisper_cli_path = repo_root
            .join(".build/whisper")
            .join(target_dir_name())
            .join("bin")
            .join(executable_name("whisper-cli"));

        if !whisper_cli_path.is_file() {
            return Err(format!(
                "local whisper toolchain not found. Build it first with {}",
                build_script_path.display()
            ));
        }

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

        let output = Command::new("sh")
            .arg(&self.download_script_path)
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
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                repo_root.join(path)
            }
        }
        _ => repo_root.join(DEFAULT_MODEL_RELATIVE_PATH),
    }
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
    use std::fs;
    use std::sync::{Mutex, OnceLock};
    use uuid::Uuid;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

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
        write_executable(
            &build_bin.join(executable_name("whisper-cli")),
            "#!/bin/sh\nexit 0\n",
        );
        fs::write(
            repo_root.join("scripts/build_whisper.sh"),
            "#!/bin/sh\nexit 0\n",
        )
        .expect("build script");
        fs::write(
            repo_root.join("whisper.cpp/models/download-ggml-model.sh"),
            "#!/bin/sh\nexit 0\n",
        )
        .expect("download script");

        let toolchain = Toolchain::discover(&repo_root).expect("toolchain");

        assert_eq!(
            toolchain.model_path,
            repo_root.join("models/whisper/ggml-base.bin")
        );
        assert_eq!(
            toolchain.whisper_cli_path,
            build_bin.join(executable_name("whisper-cli"))
        );
    }

    #[test]
    fn discover_uses_env_override_for_relative_model_path() {
        let _guard = env_lock().lock().expect("env lock");
        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/whisper")
            .join(target_dir_name())
            .join("bin");
        fs::create_dir_all(&build_bin).expect("build bin");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        fs::create_dir_all(repo_root.join("whisper.cpp/models")).expect("models dir");
        write_executable(
            &build_bin.join(executable_name("whisper-cli")),
            "#!/bin/sh\nexit 0\n",
        );
        fs::write(
            repo_root.join("scripts/build_whisper.sh"),
            "#!/bin/sh\nexit 0\n",
        )
        .expect("build script");
        fs::write(
            repo_root.join("whisper.cpp/models/download-ggml-model.sh"),
            "#!/bin/sh\nexit 0\n",
        )
        .expect("download script");

        unsafe { std::env::set_var(MODEL_ENV_VAR, "custom/ggml-base.bin") };
        let toolchain = Toolchain::discover(&repo_root).expect("toolchain");
        unsafe { std::env::remove_var(MODEL_ENV_VAR) };

        assert_eq!(toolchain.model_path, repo_root.join("custom/ggml-base.bin"));
    }

    #[test]
    fn reports_missing_whisper_toolchain() {
        let repo_root = temp_workspace();
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");

        let error = Toolchain::discover(&repo_root).expect_err("toolchain should fail");

        assert!(error.contains("scripts/build_whisper.sh"));
    }

    #[test]
    fn downloads_missing_model_to_configured_location() {
        let repo_root = temp_workspace();
        let build_bin = repo_root
            .join(".build/whisper")
            .join(target_dir_name())
            .join("bin");
        let models_dir = repo_root.join("whisper.cpp/models");
        let download_log = repo_root.join("download.log");
        fs::create_dir_all(&build_bin).expect("build bin");
        fs::create_dir_all(&models_dir).expect("models dir");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        write_executable(
            &build_bin.join(executable_name("whisper-cli")),
            "#!/bin/sh\nexit 0\n",
        );
        fs::write(
            repo_root.join("scripts/build_whisper.sh"),
            "#!/bin/sh\nexit 0\n",
        )
        .expect("build script");
        write_executable(
            &models_dir.join("download-ggml-model.sh"),
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nmkdir -p \"$2\"\n: > \"$2/ggml-$1.bin\"\n",
                download_log.display()
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
            &build_bin.join(executable_name("whisper-cli")),
            "#!/bin/sh\nnext=''\nout=''\nfor arg in \"$@\"; do\n  if [ \"$next\" = 'of' ]; then\n    out=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -of)\n      next='of'\n      ;;\n  esac\ndone\nmkdir -p \"$(dirname \"$out\")\"\nprintf 'synthetic transcript' > \"$out.txt\"\n",
        );

        let toolchain = Toolchain {
            whisper_cli_path: build_bin.join(executable_name("whisper-cli")),
            build_script_path: repo_root.join("scripts/build_whisper.sh"),
            download_script_path: repo_root.join("whisper.cpp/models/download-ggml-model.sh"),
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

    fn write_executable(path: &Path, content: &str) {
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
