pub mod api;
pub mod config;
pub mod error;
pub mod jobs;
pub mod models;
pub mod pipeline;
pub mod sidecar_clients;
pub mod storage;

use std::sync::Arc;

use anyhow::Context;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tracing_subscriber::{fmt, EnvFilter};

use crate::config::AppConfig;
use crate::jobs::JobWorker;
use crate::pipeline::PipelineProcessor;
use crate::sidecar_clients::{
    EmbeddingClient, HttpLlamaEmbeddingClient, HttpLlamaSummaryClient, HttpWhisperClient,
    SummaryClient, TranscriptionClient,
};
use crate::storage::{LocalFileStore, RecordingRepository, SqliteRepository};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub repo: Arc<dyn RecordingRepository>,
    pub file_store: Arc<LocalFileStore>,
    pub transcription_client: Arc<dyn TranscriptionClient>,
    pub summary_client: Arc<dyn SummaryClient>,
    pub embedding_client: Arc<dyn EmbeddingClient>,
    pub worker_notify: Arc<Notify>,
}

pub async fn run() -> anyhow::Result<()> {
    init_tracing();

    let config = Arc::new(AppConfig::from_env()?);
    let file_store = Arc::new(LocalFileStore::new(config.app_storage_root.clone()));
    file_store.ensure_root().await?;

    let repo: Arc<dyn RecordingRepository> = Arc::new(
        SqliteRepository::new(config.app_db_path.clone(), config.app_storage_root.clone())
            .await
            .with_context(|| "failed to initialize sqlite repository")?,
    );

    let transcription_client: Arc<dyn TranscriptionClient> =
        Arc::new(HttpWhisperClient::new(&config));
    let summary_client: Arc<dyn SummaryClient> = Arc::new(HttpLlamaSummaryClient::new(&config));
    let embedding_client: Arc<dyn EmbeddingClient> =
        Arc::new(HttpLlamaEmbeddingClient::new(&config));
    let worker_notify = Arc::new(Notify::new());

    let state = AppState {
        config: Arc::clone(&config),
        repo: Arc::clone(&repo),
        file_store: Arc::clone(&file_store),
        transcription_client: Arc::clone(&transcription_client),
        summary_client: Arc::clone(&summary_client),
        embedding_client: Arc::clone(&embedding_client),
        worker_notify: Arc::clone(&worker_notify),
    };

    let processor = PipelineProcessor::new(
        Arc::clone(&repo),
        Arc::clone(&file_store),
        Arc::clone(&transcription_client),
        Arc::clone(&summary_client),
        Arc::clone(&embedding_client),
        config.ffmpeg_bin.clone(),
    );

    let worker = JobWorker::new(
        Arc::clone(&repo),
        processor,
        Arc::clone(&worker_notify),
        config.worker_poll_interval,
        config.max_job_attempts,
        config.job_retry_backoff,
    );
    worker.spawn();

    let app = api::router(state);
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("failed to bind to {}", config.bind_addr))?;

    tracing::info!("listening on http://{}", config.bind_addr);

    axum::serve(listener, app)
        .await
        .with_context(|| "server exited unexpectedly")?;

    Ok(())
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,hyper=warn,reqwest=warn"));

    let _ = fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .compact()
        .try_init();
}
