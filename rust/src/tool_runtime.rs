use std::process::{Command, Output};

pub fn apply_cpu_fallback_env(command: &mut Command) {
    if cfg!(target_os = "macos") {
        command.env("GGML_METAL", "0");
        command.env("GGML_METAL_DEVICES", "0");
    }
}

pub fn should_retry_with_cpu(
    error: &str,
    macos_markers: &[&str],
    windows_markers: &[&str],
) -> bool {
    let backend_markers = if cfg!(target_os = "macos") {
        macos_markers
    } else if cfg!(windows) {
        windows_markers
    } else {
        return false;
    };

    let lower = error.to_ascii_lowercase();
    backend_markers.iter().any(|marker| lower.contains(marker))
}

pub fn command_output_details(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        stdout
    } else {
        "unknown error".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Output;

    #[test]
    fn command_output_details_prefers_stderr() {
        let output = Output {
            status: success_status(),
            stdout: b"stdout".to_vec(),
            stderr: b"stderr".to_vec(),
        };

        assert_eq!(command_output_details(&output), "stderr");
    }

    #[test]
    fn command_output_details_falls_back_to_unknown_error() {
        let output = Output {
            status: success_status(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        };

        assert_eq!(command_output_details(&output), "unknown error");
    }

    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn should_retry_with_cpu_matches_platform_markers() {
        let error = if cfg!(target_os = "macos") {
            "metal backend failure"
        } else {
            "cuda backend failure"
        };

        assert!(should_retry_with_cpu(error, &["metal"], &["cuda"]));
    }

    fn success_status() -> std::process::ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;

            return std::process::ExitStatus::from_raw(0);
        }

        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;

            return std::process::ExitStatus::from_raw(0);
        }
    }
}
