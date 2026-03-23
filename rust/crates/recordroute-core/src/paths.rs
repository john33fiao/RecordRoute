use std::path::{Path, PathBuf};

use crate::error::CoreError;

pub fn normalize_record_path(db_root: &Path, path_str: &str) -> Result<String, CoreError> {
    let normalized = path_str.trim().replace('\\', "/");
    if normalized.is_empty() {
        return Err(CoreError::InvalidRecordPath);
    }

    if let Ok(relative) = Path::new(&normalized).strip_prefix(db_root) {
        return build_record_path(relative, &normalized);
    }

    if let Some(relative) = normalized.strip_prefix("DB/") {
        return build_record_path(Path::new(relative), &normalized);
    }

    build_record_path(Path::new(normalized.trim_start_matches('/')), &normalized)
}

pub fn resolve_record_path(db_root: &Path, alias: &str) -> Result<PathBuf, CoreError> {
    let normalized = normalize_record_path(db_root, alias)?;
    let relative = normalized
        .strip_prefix("DB/")
        .ok_or(CoreError::InvalidRecordPath)?;
    Ok(db_root.join(relative))
}

pub fn to_record_path(db_root: &Path, absolute: &Path) -> Result<String, CoreError> {
    let relative = absolute
        .strip_prefix(db_root)
        .map_err(|_| CoreError::InvalidRecordPath)?;
    build_record_path(relative, absolute.display().to_string())
}

fn build_record_path(relative: &Path, original: impl Into<String>) -> Result<String, CoreError> {
    let original = original.into();
    let mut components = Vec::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(part) => {
                components.push(part.to_string_lossy().into_owned())
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::Prefix(_)
            | std::path::Component::RootDir => {
                return Err(CoreError::RecordPathEscapesDbRoot { path: original });
            }
        }
    }

    if components.is_empty() {
        return Err(CoreError::InvalidRecordPath);
    }

    Ok(format!("DB/{}", components.join("/")))
}

#[cfg(test)]
mod tests {
    use super::{normalize_record_path, resolve_record_path, to_record_path};
    use std::path::{Path, PathBuf};

    #[test]
    fn resolves_db_alias() {
        let resolved = resolve_record_path(Path::new("DB"), "DB/uploads/file.wav").unwrap();
        assert_eq!(resolved, PathBuf::from("DB/uploads/file.wav"));
    }

    #[test]
    fn normalizes_relative_paths_into_db_aliases() {
        let alias = normalize_record_path(Path::new("DB"), "uploads/file.wav").unwrap();
        assert_eq!(alias, "DB/uploads/file.wav");
    }

    #[test]
    fn converts_absolute_path_back_to_alias() {
        let alias =
            to_record_path(Path::new("/tmp/DB"), Path::new("/tmp/DB/uploads/file.wav")).unwrap();
        assert_eq!(alias, "DB/uploads/file.wav");
    }

    #[test]
    fn rejects_parent_dir_segments() {
        let error = normalize_record_path(Path::new("DB"), "../outside/file.wav").unwrap_err();
        assert_eq!(
            error.to_string(),
            "record path escapes DB root: ../outside/file.wav"
        );
    }
}
