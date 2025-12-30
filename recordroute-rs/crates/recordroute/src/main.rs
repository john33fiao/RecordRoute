use recordroute_common::{AppConfig, logger};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 설정 로드
    let config = AppConfig::from_env()?;

    // 로깅 초기화
    logger::setup_logging(&config.log_dir, &config.log_level)?;

    tracing::info!("RecordRoute starting...");
    tracing::info!("Server will bind to: {}", config.server_bind_address());

    // TODO: Phase 5에서 서버 시작 코드 추가

    tracing::info!("RecordRoute initialized successfully (Phase 1 complete)");

    Ok(())
}
