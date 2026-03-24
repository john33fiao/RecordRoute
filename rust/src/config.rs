use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: SocketAddr,
    pub app_db_path: PathBuf,
    pub app_storage_root: PathBuf,
    pub ffmpeg_bin: String,
    pub whisper_base_url: String,
    pub llama_summary_base_url: String,
    pub llama_embed_base_url: String,
    pub whisper_model: String,
    pub summary_model: String,
    pub embed_model: String,
    pub sidecar_timeout: Duration,
    pub worker_poll_interval: Duration,
    pub max_upload_size_bytes: usize,
    pub max_job_attempts: i32,
    pub job_retry_backoff: Duration,
    pub max_search_limit: usize,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let bind_addr = env::var("APP_BIND_ADDR")
            .ok()
            .map(|value| value.parse())
            .transpose()
            .context("invalid APP_BIND_ADDR")?
            .unwrap_or_else(|| SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000));
        let app_storage_root = PathBuf::from(required_env("APP_STORAGE_ROOT")?);
        let app_db_path = env::var("APP_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| app_storage_root.join("record-route.db"));

        Ok(Self {
            bind_addr,
            app_db_path,
            app_storage_root,
            ffmpeg_bin: env::var("FFMPEG_BIN").unwrap_or_else(|_| "ffmpeg".to_string()),
            whisper_base_url: required_env("WHISPER_BASE_URL")?,
            llama_summary_base_url: required_env("LLAMA_SUMMARY_BASE_URL")?,
            llama_embed_base_url: required_env("LLAMA_EMBED_BASE_URL")?,
            whisper_model: required_env("WHISPER_MODEL")?,
            summary_model: required_env("SUMMARY_MODEL")?,
            embed_model: required_env("EMBED_MODEL")?,
            sidecar_timeout: Duration::from_secs(env_parse("SIDECAR_TIMEOUT_SECS", 300_u64)?),
            worker_poll_interval: Duration::from_millis(env_parse("WORKER_POLL_INTERVAL_MS", 1_000_u64)?),
            max_upload_size_bytes: env_parse("MAX_UPLOAD_SIZE_BYTES", 100_usize * 1024 * 1024)?,
            max_job_attempts: env_parse("MAX_JOB_ATTEMPTS", 3_i32)?,
            job_retry_backoff: Duration::from_secs(env_parse("JOB_RETRY_BACKOFF_SECS", 15_u64)?),
            max_search_limit: env_parse("MAX_SEARCH_LIMIT", 50_usize)?,
        })
    }
}

fn required_env(key: &str) -> Result<String> {
    env::var(key).with_context(|| format!("missing required environment variable `{key}`"))
}

fn env_parse<T>(key: &str, default: T) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(key) {
        Ok(value) => value
            .parse::<T>()
            .map_err(|err| anyhow::anyhow!("invalid `{key}` value `{value}`: {err}")),
        Err(_) => Ok(default),
    }
}
