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

    if executable_dir.join(RUNTIME_ROOT_MARKER).is_file() {
        return Ok(executable_dir);
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to resolve repository root".to_string())
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
}
