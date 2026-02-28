use std::{
    env,
    fmt,
    net::{AddrParseError, SocketAddr},
};

use crate::EngineKind;

/// All validation failures found when loading configuration from environment
/// variables.  Collecting every error at once lets the operator fix them all
/// in a single restart rather than discovering them one by one.
#[derive(Debug)]
pub(crate) struct ConfigError(Vec<String>);

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid configuration ({} error(s)):", self.0.len())?;
        for msg in &self.0 {
            write!(f, "\n  - {msg}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ConfigError {}

#[derive(Clone, Debug)]
pub(crate) struct AppConfig {
    pub(crate) host: String,
    pub(crate) api_port: u16,
    pub(crate) stt_queue_capacity: usize,
    pub(crate) summarize_queue_capacity: usize,
    pub(crate) embed_queue_capacity: usize,
    pub(crate) stt_concurrency: usize,
    pub(crate) summarize_concurrency: usize,
    pub(crate) embed_concurrency: usize,
    pub(crate) engine_timeout_secs: u64,
    pub(crate) job_timeout_secs: u64,
    pub(crate) job_timeout_min_secs: u64,
    pub(crate) job_timeout_max_secs: u64,
    pub(crate) stt_timeout_per_audio_sec_ms: u64,
    pub(crate) stt_timeout_buffer_ms: u64,
    pub(crate) engine_connect_timeout_ms: u64,
    pub(crate) engine_retry_count: usize,
    pub(crate) engine_base_backoff_ms: u64,
    pub(crate) stt_engine_url: String,
    pub(crate) summarize_engine_url: String,
    pub(crate) embed_engine_url: String,
    pub(crate) engine_supervision_enabled: bool,
    pub(crate) stt_engine_command: String,
    pub(crate) stt_engine_args: Vec<String>,
    pub(crate) summarize_engine_command: String,
    pub(crate) summarize_engine_args: Vec<String>,
    pub(crate) embed_engine_command: String,
    pub(crate) embed_engine_args: Vec<String>,
    pub(crate) engine_startup_timeout_secs: u64,
    pub(crate) engine_readiness_poll_ms: u64,
    pub(crate) engine_shutdown_grace_secs: u64,
    pub(crate) engine_restart_backoff_base_ms: u64,
    pub(crate) engine_restart_backoff_max_ms: u64,
}

impl AppConfig {
    /// Load configuration from environment variables and validate all values.
    ///
    /// Returns a [`ConfigError`] listing every invalid value found so the
    /// operator can fix them all before restarting.
    pub(crate) fn from_env() -> Result<Self, ConfigError> {
        let host = env::var("RECORDROUTE_API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let api_port = env::var("RECORDROUTE_API_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(18_000);

        let cfg = Self {
            host,
            api_port,
            stt_queue_capacity: read_usize_env("RECORDROUTE_STT_QUEUE_CAPACITY", 1_024),
            summarize_queue_capacity: read_usize_env(
                "RECORDROUTE_SUMMARIZE_QUEUE_CAPACITY",
                1_024,
            ),
            embed_queue_capacity: read_usize_env("RECORDROUTE_EMBED_QUEUE_CAPACITY", 1_024),
            stt_concurrency: read_usize_env("RECORDROUTE_STT_CONCURRENCY", 2),
            summarize_concurrency: read_usize_env("RECORDROUTE_SUMMARIZE_CONCURRENCY", 2),
            embed_concurrency: read_usize_env("RECORDROUTE_EMBED_CONCURRENCY", 2),
            engine_timeout_secs: read_u64_env("RECORDROUTE_ENGINE_TIMEOUT_SECS", 60),
            job_timeout_secs: read_u64_env("RECORDROUTE_JOB_TIMEOUT_SECS", 120),
            job_timeout_min_secs: read_u64_env("RECORDROUTE_JOB_TIMEOUT_MIN_SECS", 30),
            job_timeout_max_secs: read_u64_env("RECORDROUTE_JOB_TIMEOUT_MAX_SECS", 900),
            stt_timeout_per_audio_sec_ms: read_u64_env(
                "RECORDROUTE_STT_TIMEOUT_PER_AUDIO_SEC_MS",
                1_500,
            ),
            stt_timeout_buffer_ms: read_u64_env("RECORDROUTE_STT_TIMEOUT_BUFFER_MS", 5_000),
            engine_connect_timeout_ms: read_u64_env(
                "RECORDROUTE_ENGINE_CONNECT_TIMEOUT_MS",
                1_500,
            ),
            engine_retry_count: read_usize_env("RECORDROUTE_ENGINE_RETRY_COUNT", 1),
            engine_base_backoff_ms: read_u64_env("RECORDROUTE_ENGINE_BACKOFF_MS", 200),
            stt_engine_url: env::var("RECORDROUTE_STT_ENGINE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:18103/infer".to_string()),
            summarize_engine_url: env::var("RECORDROUTE_SUMMARIZE_ENGINE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:18101/infer".to_string()),
            embed_engine_url: env::var("RECORDROUTE_EMBED_ENGINE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:18102/infer".to_string()),
            engine_supervision_enabled: env::var("RECORDROUTE_ENGINE_SUPERVISION_ENABLED")
                .ok()
                .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
                .unwrap_or(false),
            stt_engine_command: env::var("RECORDROUTE_STT_ENGINE_COMMAND")
                .unwrap_or_else(|_| "whisper-server".to_string()),
            stt_engine_args: read_csv_env("RECORDROUTE_STT_ENGINE_ARGS"),
            summarize_engine_command: env::var("RECORDROUTE_SUMMARIZE_ENGINE_COMMAND")
                .unwrap_or_else(|_| "llama-text".to_string()),
            summarize_engine_args: read_csv_env("RECORDROUTE_SUMMARIZE_ENGINE_ARGS"),
            embed_engine_command: env::var("RECORDROUTE_EMBED_ENGINE_COMMAND")
                .unwrap_or_else(|_| "llama-embed".to_string()),
            embed_engine_args: read_csv_env("RECORDROUTE_EMBED_ENGINE_ARGS"),
            engine_startup_timeout_secs: read_u64_env(
                "RECORDROUTE_ENGINE_STARTUP_TIMEOUT_SECS",
                20,
            ),
            engine_readiness_poll_ms: read_u64_env("RECORDROUTE_ENGINE_READINESS_POLL_MS", 500),
            engine_shutdown_grace_secs: read_u64_env("RECORDROUTE_ENGINE_SHUTDOWN_GRACE_SECS", 5),
            engine_restart_backoff_base_ms: read_u64_env(
                "RECORDROUTE_ENGINE_RESTART_BACKOFF_BASE_MS",
                500,
            ),
            engine_restart_backoff_max_ms: read_u64_env(
                "RECORDROUTE_ENGINE_RESTART_BACKOFF_MAX_MS",
                15_000,
            ),
        };

        cfg.validate()?;
        Ok(cfg)
    }

    /// Validate all loaded values and collect every error found.
    fn validate(&self) -> Result<(), ConfigError> {
        let mut errors: Vec<String> = Vec::new();

        // ── Host / port ────────────────────────────────────────────────────
        if self.host.trim().is_empty() {
            errors.push("RECORDROUTE_API_HOST must not be empty".to_string());
        }
        if self.api_port < 1024 {
            errors.push(format!(
                "RECORDROUTE_API_PORT={} is below the minimum 1024 (privileged port range)",
                self.api_port
            ));
        }

        // ── Queue capacities ───────────────────────────────────────────────
        if self.stt_queue_capacity < 1 {
            errors.push("RECORDROUTE_STT_QUEUE_CAPACITY must be >= 1".to_string());
        }
        if self.summarize_queue_capacity < 1 {
            errors.push("RECORDROUTE_SUMMARIZE_QUEUE_CAPACITY must be >= 1".to_string());
        }
        if self.embed_queue_capacity < 1 {
            errors.push("RECORDROUTE_EMBED_QUEUE_CAPACITY must be >= 1".to_string());
        }

        // ── Worker concurrency ─────────────────────────────────────────────
        if self.stt_concurrency < 1 {
            errors.push("RECORDROUTE_STT_CONCURRENCY must be >= 1".to_string());
        }
        if self.summarize_concurrency < 1 {
            errors.push("RECORDROUTE_SUMMARIZE_CONCURRENCY must be >= 1".to_string());
        }
        if self.embed_concurrency < 1 {
            errors.push("RECORDROUTE_EMBED_CONCURRENCY must be >= 1".to_string());
        }

        // ── Engine / connection timeouts ───────────────────────────────────
        if self.engine_timeout_secs < 1 {
            errors.push("RECORDROUTE_ENGINE_TIMEOUT_SECS must be >= 1".to_string());
        }
        if self.engine_connect_timeout_ms < 1 {
            errors.push("RECORDROUTE_ENGINE_CONNECT_TIMEOUT_MS must be >= 1".to_string());
        }

        // ── Job timeout ordering: min <= base <= max ───────────────────────
        if self.job_timeout_min_secs < 1 {
            errors.push("RECORDROUTE_JOB_TIMEOUT_MIN_SECS must be >= 1".to_string());
        }
        if self.job_timeout_max_secs < self.job_timeout_min_secs {
            errors.push(format!(
                "RECORDROUTE_JOB_TIMEOUT_MAX_SECS ({}) must be >= \
                 RECORDROUTE_JOB_TIMEOUT_MIN_SECS ({})",
                self.job_timeout_max_secs, self.job_timeout_min_secs
            ));
        }
        if self.job_timeout_secs < self.job_timeout_min_secs {
            errors.push(format!(
                "RECORDROUTE_JOB_TIMEOUT_SECS ({}) must be >= \
                 RECORDROUTE_JOB_TIMEOUT_MIN_SECS ({})",
                self.job_timeout_secs, self.job_timeout_min_secs
            ));
        }
        if self.job_timeout_secs > self.job_timeout_max_secs {
            errors.push(format!(
                "RECORDROUTE_JOB_TIMEOUT_SECS ({}) must be <= \
                 RECORDROUTE_JOB_TIMEOUT_MAX_SECS ({})",
                self.job_timeout_secs, self.job_timeout_max_secs
            ));
        }

        // ── Engine restart backoff ordering: base <= max ───────────────────
        if self.engine_restart_backoff_max_ms < self.engine_restart_backoff_base_ms {
            errors.push(format!(
                "RECORDROUTE_ENGINE_RESTART_BACKOFF_MAX_MS ({}) must be >= \
                 RECORDROUTE_ENGINE_RESTART_BACKOFF_BASE_MS ({})",
                self.engine_restart_backoff_max_ms, self.engine_restart_backoff_base_ms
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigError(errors))
        }
    }

    pub(crate) fn api_addr(&self) -> Result<SocketAddr, AddrParseError> {
        format!("{}:{}", self.host, self.api_port).parse()
    }

    pub(crate) fn queue_capacity(&self, engine: EngineKind) -> usize {
        match engine {
            EngineKind::Stt => self.stt_queue_capacity,
            EngineKind::Summarize => self.summarize_queue_capacity,
            EngineKind::Embed => self.embed_queue_capacity,
        }
    }

    pub(crate) fn concurrency(&self, engine: EngineKind) -> usize {
        match engine {
            EngineKind::Stt => self.stt_concurrency,
            EngineKind::Summarize => self.summarize_concurrency,
            EngineKind::Embed => self.embed_concurrency,
        }
    }
}

fn read_usize_env(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

fn read_u64_env(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

fn read_csv_env(key: &str) -> Vec<String> {
    env::var(key)
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_config() -> AppConfig {
        AppConfig {
            host: "0.0.0.0".to_string(),
            api_port: 18_000,
            stt_queue_capacity: 1,
            summarize_queue_capacity: 1,
            embed_queue_capacity: 1,
            stt_concurrency: 1,
            summarize_concurrency: 1,
            embed_concurrency: 1,
            engine_timeout_secs: 1,
            job_timeout_secs: 30,
            job_timeout_min_secs: 1,
            job_timeout_max_secs: 900,
            stt_timeout_per_audio_sec_ms: 1_500,
            stt_timeout_buffer_ms: 5_000,
            engine_connect_timeout_ms: 1,
            engine_retry_count: 0,
            engine_base_backoff_ms: 1,
            stt_engine_url: "http://127.0.0.1:18103/infer".to_string(),
            summarize_engine_url: "http://127.0.0.1:18101/infer".to_string(),
            embed_engine_url: "http://127.0.0.1:18102/infer".to_string(),
            engine_supervision_enabled: false,
            stt_engine_command: "whisper-server".to_string(),
            stt_engine_args: vec![],
            summarize_engine_command: "llama-text".to_string(),
            summarize_engine_args: vec![],
            embed_engine_command: "llama-embed".to_string(),
            embed_engine_args: vec![],
            engine_startup_timeout_secs: 20,
            engine_readiness_poll_ms: 1,
            engine_shutdown_grace_secs: 1,
            engine_restart_backoff_base_ms: 100,
            engine_restart_backoff_max_ms: 500,
        }
    }

    #[test]
    fn valid_config_passes_validation() {
        assert!(base_config().validate().is_ok());
    }

    #[test]
    fn empty_host_is_rejected() {
        let mut cfg = base_config();
        cfg.host = "   ".to_string();
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("RECORDROUTE_API_HOST"));
    }

    #[test]
    fn privileged_port_is_rejected() {
        let mut cfg = base_config();
        cfg.api_port = 80;
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("RECORDROUTE_API_PORT"));
        assert!(err.to_string().contains("1024"));
    }

    #[test]
    fn port_1024_is_accepted() {
        let mut cfg = base_config();
        cfg.api_port = 1024;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn zero_queue_capacity_is_rejected() {
        let mut cfg = base_config();
        cfg.stt_queue_capacity = 0;
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("RECORDROUTE_STT_QUEUE_CAPACITY"));
    }

    #[test]
    fn zero_concurrency_is_rejected() {
        let mut cfg = base_config();
        cfg.embed_concurrency = 0;
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("RECORDROUTE_EMBED_CONCURRENCY"));
    }

    #[test]
    fn job_timeout_min_greater_than_max_is_rejected() {
        let mut cfg = base_config();
        cfg.job_timeout_min_secs = 100;
        cfg.job_timeout_max_secs = 50;
        cfg.job_timeout_secs = 75;
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("JOB_TIMEOUT_MAX_SECS"));
    }

    #[test]
    fn job_timeout_base_outside_bounds_is_rejected() {
        let mut cfg = base_config();
        cfg.job_timeout_min_secs = 10;
        cfg.job_timeout_max_secs = 100;
        cfg.job_timeout_secs = 5;
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("JOB_TIMEOUT_SECS"));
    }

    #[test]
    fn engine_restart_backoff_inverted_is_rejected() {
        let mut cfg = base_config();
        cfg.engine_restart_backoff_base_ms = 1_000;
        cfg.engine_restart_backoff_max_ms = 500;
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("RESTART_BACKOFF_MAX_MS"));
    }

    #[test]
    fn multiple_errors_are_reported_together() {
        let mut cfg = base_config();
        cfg.api_port = 22;
        cfg.stt_queue_capacity = 0;
        cfg.embed_concurrency = 0;
        let err = cfg.validate().unwrap_err();
        assert!(err.0.len() >= 3);
    }

    #[test]
    fn read_csv_env_splits_and_trims() {
        std::env::set_var("_RR_TEST_CSV", " a , b , c ");
        let result = read_csv_env("_RR_TEST_CSV");
        std::env::remove_var("_RR_TEST_CSV");
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn read_csv_env_returns_empty_for_missing_key() {
        std::env::remove_var("_RR_TEST_MISSING_CSV");
        assert!(read_csv_env("_RR_TEST_MISSING_CSV").is_empty());
    }
}
