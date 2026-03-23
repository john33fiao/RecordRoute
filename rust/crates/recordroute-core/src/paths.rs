use std::path::{Path, PathBuf};

use crate::error::CoreError;

pub fn resolve_record_path(db_root: &Path, alias: &str) -> Result<PathBuf, CoreError> {
    let relative = alias
        .strip_prefix("DB/")
        .ok_or(CoreError::InvalidRecordPath)?;
    Ok(db_root.join(relative))
}

pub fn to_record_path(db_root: &Path, absolute: &Path) -> Result<String, CoreError> {
    let relative = absolute
        .strip_prefix(db_root)
        .map_err(|_| CoreError::InvalidRecordPath)?;
    Ok(format!(
        "DB/{}",
        relative.to_string_lossy().replace('\\', "/")
    ))
}

#[cfg(test)]
mod tests {
    use super::{resolve_record_path, to_record_path};
    use std::path::{Path, PathBuf};

    #[test]
    fn resolves_db_alias() {
        let resolved = resolve_record_path(Path::new("DB"), "DB/uploads/file.wav").unwrap();
        assert_eq!(resolved, PathBuf::from("DB/uploads/file.wav"));
    }

    #[test]
    fn rejects_non_db_alias() {
        let error = resolve_record_path(Path::new("DB"), "uploads/file.wav").unwrap_err();
        assert_eq!(error.to_string(), "record path must start with DB/");
    }

    #[test]
    fn converts_absolute_path_back_to_alias() {
        let alias =
            to_record_path(Path::new("/tmp/DB"), Path::new("/tmp/DB/uploads/file.wav")).unwrap();
        assert_eq!(alias, "DB/uploads/file.wav");
    }
}
