use std::path::{Path, PathBuf};

pub const RUNTIME_ROOT_ENV_VAR: &str = "RECORDROUTE_RUNTIME_ROOT";
pub const RUNTIME_ROOT_MARKER: &str = ".recordroute-runtime-root";

pub fn resolve_runtime_root() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(RUNTIME_ROOT_ENV_VAR) {
        let path = PathBuf::from(path);
        if !path.as_os_str().is_empty() {
            return Ok(path);
        }
    }

    let executable_dir = std::env::current_exe()
        .map_err(|error| format!("failed to resolve current executable path: {error}"))?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to resolve executable directory".to_string())?;

    if let Some(runtime_root) = detect_runtime_root_from_executable_dir(&executable_dir) {
        return Ok(runtime_root);
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to resolve repository root".to_string())
}

fn detect_runtime_root_from_executable_dir(executable_dir: &Path) -> Option<PathBuf> {
    if executable_dir.join(RUNTIME_ROOT_MARKER).is_file() {
        return Some(executable_dir.to_path_buf());
    }

    let server_executable = if cfg!(windows) {
        "RecordRouteServer.exe"
    } else {
        "RecordRouteServer"
    };

    if executable_dir.join(server_executable).is_file() {
        return Some(executable_dir.to_path_buf());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn env_override_takes_precedence() {
        let _guard = crate::test_support::env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let path =
            std::env::temp_dir().join(format!("recordroute-runtime-root-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).expect("runtime root");

        unsafe { std::env::set_var(RUNTIME_ROOT_ENV_VAR, &path) };
        let resolved = resolve_runtime_root().expect("runtime root");
        unsafe { std::env::remove_var(RUNTIME_ROOT_ENV_VAR) };

        assert_eq!(resolved, path);
    }

    #[test]
    fn runtime_marker_in_executable_dir_is_detected() {
        let temp_dir = std::env::temp_dir().join(format!(
            "recordroute-runtime-root-marker-{}",
            Uuid::now_v7()
        ));
        fs::create_dir_all(&temp_dir).expect("runtime root");
        fs::write(temp_dir.join(RUNTIME_ROOT_MARKER), "").expect("runtime marker");

        let resolved =
            detect_runtime_root_from_executable_dir(&temp_dir).expect("runtime root detection");
        assert_eq!(resolved, temp_dir);
    }

    #[test]
    fn server_binary_in_executable_dir_is_detected_without_marker() {
        let temp_dir = std::env::temp_dir().join(format!(
            "recordroute-runtime-root-server-{}",
            Uuid::now_v7()
        ));
        fs::create_dir_all(&temp_dir).expect("runtime root");
        let server_executable = if cfg!(windows) {
            "RecordRouteServer.exe"
        } else {
            "RecordRouteServer"
        };
        fs::write(temp_dir.join(server_executable), "").expect("server binary");

        let resolved =
            detect_runtime_root_from_executable_dir(&temp_dir).expect("runtime root detection");
        assert_eq!(resolved, temp_dir);
    }
}
