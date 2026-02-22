use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() {
    init_tracing();
    tracing::info!("RecordRoute Rust orchestrator bootstrap: READY");
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
}
