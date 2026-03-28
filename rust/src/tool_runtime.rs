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

pub fn run_with_cpu_fallback<T>(
    preferred: impl FnOnce() -> Result<T, String>,
    cpu_fallback: impl FnOnce() -> Result<T, String>,
    should_retry: impl Fn(&str) -> bool,
) -> Result<T, String> {
    match preferred() {
        Ok(value) => Ok(value),
        Err(primary_error) if should_retry(&primary_error) => {
            cpu_fallback().map_err(|cpu_error| {
                format!(
                    "{cpu_error} (after retrying on CPU because the preferred backend failed: {primary_error})"
                )
            })
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
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

    #[test]
    fn run_with_cpu_fallback_returns_preferred_success_without_retrying() {
        let retried = Cell::new(false);

        let result = run_with_cpu_fallback(
            || Ok::<_, String>("preferred"),
            || {
                retried.set(true);
                Ok("cpu")
            },
            |_| true,
        )
        .expect("preferred result");

        assert_eq!(result, "preferred");
        assert!(!retried.get());
    }

    #[test]
    fn run_with_cpu_fallback_retries_when_predicate_matches() {
        let result = run_with_cpu_fallback(
            || Err::<&str, _>("gpu backend failure".to_string()),
            || Ok::<_, String>("cpu"),
            |error| error.contains("gpu backend"),
        )
        .expect("cpu result");

        assert_eq!(result, "cpu");
    }

    #[test]
    fn run_with_cpu_fallback_preserves_retry_context_on_cpu_failure() {
        let error = run_with_cpu_fallback(
            || Err::<(), _>("gpu backend failure".to_string()),
            || Err::<(), _>("cpu backend failure".to_string()),
            |error| error.contains("gpu backend"),
        )
        .expect_err("retry failure");

        assert_eq!(
            error,
            "cpu backend failure (after retrying on CPU because the preferred backend failed: gpu backend failure)"
        );
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
