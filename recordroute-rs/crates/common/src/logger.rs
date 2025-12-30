use crate::error::RecordRouteError;
use std::path::Path;
use tracing::Level;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

/// Initialize logging system
///
/// Sets up logging to both console and file
///
/// # Arguments
/// * `log_dir` - Directory where log files will be stored
/// * `log_level` - Log level (trace, debug, info, warn, error)
pub fn setup_logging(log_dir: &Path, log_level: &str) -> Result<(), RecordRouteError> {
    // Create log directory
    if !log_dir.exists() {
        std::fs::create_dir_all(log_dir).map_err(|e| {
            RecordRouteError::config(format!(
                "Failed to create log directory {}: {}",
                log_dir.display(),
                e
            ))
        })?;
    }

    // Log file path
    let log_file_path = log_dir.join("recordroute.log");
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .map_err(|e| {
            RecordRouteError::config(format!(
                "Failed to open log file {}: {}",
                log_file_path.display(),
                e
            ))
        })?;

    // Environment filter setup (RUST_LOG env var takes precedence)
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    // Console output layer
    let console_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_filter(env_filter.clone());

    // File output layer
    let file_layer = fmt::layer()
        .with_writer(log_file)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_ansi(false) // Remove ANSI color codes in files
        .with_span_events(FmtSpan::FULL)
        .with_filter(env_filter);

    // Initialize subscriber
    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();

    tracing::info!("Logging initialized: level={}, log_file={}", log_level, log_file_path.display());

    Ok(())
}

/// Simple logging setup (console only)
///
/// For development and testing environments
pub fn setup_console_logging(log_level: &str) -> Result<(), RecordRouteError> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    tracing_subscriber::fmt()
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_env_filter(env_filter)
        .init();

    tracing::info!("Console logging initialized: level={}", log_level);

    Ok(())
}

/// Parse string to tracing Level
pub fn parse_log_level(level: &str) -> Level {
    match level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" | "warning" => Level::WARN,
        "error" => Level::ERROR,
        _ => {
            eprintln!("Invalid log level '{}', defaulting to INFO", level);
            Level::INFO
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_log_level() {
        assert_eq!(parse_log_level("trace"), Level::TRACE);
        assert_eq!(parse_log_level("debug"), Level::DEBUG);
        assert_eq!(parse_log_level("info"), Level::INFO);
        assert_eq!(parse_log_level("warn"), Level::WARN);
        assert_eq!(parse_log_level("error"), Level::ERROR);
        assert_eq!(parse_log_level("invalid"), Level::INFO);
    }

    #[test]
    fn test_parse_log_level_case_insensitive() {
        assert_eq!(parse_log_level("INFO"), Level::INFO);
        assert_eq!(parse_log_level("InFo"), Level::INFO);
        assert_eq!(parse_log_level("WARNING"), Level::WARN);
    }
}
