use std::path::{Path, PathBuf};

pub const METADATA_DRIVER_ENV_VAR: &str = "RECORDROUTE_METADATA_DRIVER";
pub const METADATA_SQLITE_PATH_ENV_VAR: &str = "RECORDROUTE_METADATA_SQLITE_PATH";
pub const METADATA_POSTGRES_URL_ENV_VAR: &str = "RECORDROUTE_METADATA_POSTGRES_URL";
pub const AUDIO_ROOT_ENV_VAR: &str = "RECORDROUTE_AUDIO_ROOT";
pub const AUDIO_CACHE_ROOT_ENV_VAR: &str = "RECORDROUTE_AUDIO_CACHE_ROOT";
pub const AUDIO_SPOOL_ROOT_ENV_VAR: &str = "RECORDROUTE_AUDIO_SPOOL_ROOT";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataDriver {
    Sqlite,
    Postgres,
}

impl MetadataDriver {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "sqlite" => Ok(Self::Sqlite),
            "postgres" | "postgresql" => Ok(Self::Postgres),
            other => Err(format!(
                "unsupported metadata driver: {other} (expected sqlite or postgres)"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataConfig {
    pub driver: MetadataDriver,
    pub sqlite_path: PathBuf,
    pub postgres_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioConfig {
    pub root: PathBuf,
    pub cache_root: PathBuf,
    pub spool_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageConfig {
    pub metadata: MetadataConfig,
    pub audio: AudioConfig,
}

impl StorageConfig {
    pub fn load(repo_root: &Path) -> Result<Self, String> {
        let db_root = repo_root.join("db");
        let driver = std::env::var(METADATA_DRIVER_ENV_VAR)
            .map_err(|error| format!("failed to read {METADATA_DRIVER_ENV_VAR}: {error}"))
            .and_then(|value| MetadataDriver::parse(&value))
            .unwrap_or(MetadataDriver::Sqlite);
        let sqlite_path = std::env::var_os(METADATA_SQLITE_PATH_ENV_VAR)
            .map(PathBuf::from)
            .map(|path| resolve_path(repo_root, path))
            .unwrap_or_else(|| db_root.join("index.sqlite3"));
        let postgres_url = std::env::var(METADATA_POSTGRES_URL_ENV_VAR)
            .ok()
            .filter(|value| !value.trim().is_empty());
        if driver == MetadataDriver::Postgres && postgres_url.is_none() {
            return Err(format!(
                "{METADATA_POSTGRES_URL_ENV_VAR} is required when {METADATA_DRIVER_ENV_VAR}=postgres"
            ));
        }

        let audio_root = std::env::var_os(AUDIO_ROOT_ENV_VAR)
            .map(PathBuf::from)
            .map(|path| resolve_path(repo_root, path))
            .unwrap_or_else(|| db_root.join("audio"));
        let audio_cache_root = std::env::var_os(AUDIO_CACHE_ROOT_ENV_VAR)
            .map(PathBuf::from)
            .map(|path| resolve_path(repo_root, path))
            .unwrap_or_else(|| db_root.join("audio-cache"));
        let audio_spool_root = std::env::var_os(AUDIO_SPOOL_ROOT_ENV_VAR)
            .map(PathBuf::from)
            .map(|path| resolve_path(repo_root, path))
            .unwrap_or_else(|| db_root.join("audio-spool"));

        Ok(Self {
            metadata: MetadataConfig {
                driver,
                sqlite_path,
                postgres_url,
            },
            audio: AudioConfig {
                root: audio_root,
                cache_root: audio_cache_root,
                spool_root: audio_spool_root,
            },
        })
    }
}

fn resolve_path(repo_root: &Path, configured: PathBuf) -> PathBuf {
    if configured.is_absolute() {
        configured
    } else {
        repo_root.join(configured)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::env_lock;
    use uuid::Uuid;

    #[test]
    fn loads_default_paths_under_db_root() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_env();
        let repo_root = temp_repo_root();

        let config = StorageConfig::load(&repo_root).expect("storage config");

        assert_eq!(config.metadata.driver, MetadataDriver::Sqlite);
        assert_eq!(config.metadata.sqlite_path, repo_root.join("db/index.sqlite3"));
        assert_eq!(config.audio.root, repo_root.join("db/audio"));
        assert_eq!(config.audio.cache_root, repo_root.join("db/audio-cache"));
        assert_eq!(config.audio.spool_root, repo_root.join("db/audio-spool"));
    }

    #[test]
    fn env_overrides_take_precedence() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_env();
        let repo_root = temp_repo_root();
        unsafe { std::env::set_var(METADATA_DRIVER_ENV_VAR, "postgres") };
        unsafe { std::env::set_var(METADATA_POSTGRES_URL_ENV_VAR, "postgres://localhost/test") };
        unsafe { std::env::set_var(METADATA_SQLITE_PATH_ENV_VAR, "custom/ignored.sqlite3") };
        unsafe { std::env::set_var(AUDIO_ROOT_ENV_VAR, "/tmp/rr-audio") };
        unsafe { std::env::set_var(AUDIO_CACHE_ROOT_ENV_VAR, "cache-audio") };
        unsafe { std::env::set_var(AUDIO_SPOOL_ROOT_ENV_VAR, "spool-audio") };

        let config = StorageConfig::load(&repo_root).expect("storage config");

        assert_eq!(config.metadata.driver, MetadataDriver::Postgres);
        assert_eq!(
            config.metadata.postgres_url.as_deref(),
            Some("postgres://localhost/test")
        );
        assert_eq!(
            config.metadata.sqlite_path,
            repo_root.join("custom/ignored.sqlite3")
        );
        assert_eq!(config.audio.root, PathBuf::from("/tmp/rr-audio"));
        assert_eq!(config.audio.cache_root, repo_root.join("cache-audio"));
        assert_eq!(config.audio.spool_root, repo_root.join("spool-audio"));

        clear_env();
    }

    fn clear_env() {
        for key in [
            METADATA_DRIVER_ENV_VAR,
            METADATA_SQLITE_PATH_ENV_VAR,
            METADATA_POSTGRES_URL_ENV_VAR,
            AUDIO_ROOT_ENV_VAR,
            AUDIO_CACHE_ROOT_ENV_VAR,
            AUDIO_SPOOL_ROOT_ENV_VAR,
        ] {
            unsafe { std::env::remove_var(key) };
        }
    }

    fn temp_repo_root() -> PathBuf {
        std::env::temp_dir().join(format!("recordroute-storage-{}", Uuid::now_v7()))
    }
}
