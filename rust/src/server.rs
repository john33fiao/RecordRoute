#[path = "server/app_api.rs"]
mod app_api;
#[path = "server/errors.rs"]
mod errors;
#[path = "server/files.rs"]
mod files;
#[path = "server/routes/jobs.rs"]
mod jobs;
#[path = "server/routes/models.rs"]
mod models;
#[path = "server/routes/ping.rs"]
mod ping;
#[path = "server/routes/stages.rs"]
mod stages;
#[path = "server/types.rs"]
mod types;
#[path = "server/upload.rs"]
mod upload;
#[path = "server/routes/web.rs"]
mod web;

use crate::app;
use crate::error::{AppError, AppResult};
use axum::Router;
use axum::response::Response;
use axum::routing::{get, post};
use std::path::PathBuf;
use types::AppState;

pub const SERVER_BIND: &str = "127.0.0.1:38080";

pub(crate) fn router_with_repo_root(repo_root: PathBuf) -> Router {
    Router::new()
        .route("/", get(web::get_index))
        .route("/app.js", get(web::get_app_js))
        .route("/app.css", get(web::get_app_css))
        .route("/server/ping", post(ping::post_server_ping))
        .route("/system/status", get(models::get_system_status))
        .route("/models/status", get(models::get_models_status))
        .route(
            "/models/whisper/prepare",
            post(models::post_prepare_whisper_model),
        )
        .route(
            "/models/llama/prepare",
            post(models::post_prepare_llama_model),
        )
        .route("/jobs", post(jobs::post_jobs).get(jobs::get_jobs))
        .route("/jobs/completed", get(jobs::get_completed_jobs))
        .route("/jobs/by-source", get(jobs::get_jobs_by_source))
        .route("/jobs/upload", post(jobs::post_jobs_upload))
        .route("/jobs/{job_id}", get(jobs::get_job))
        .route("/jobs/{job_id}/status", get(jobs::get_job_status))
        .route(
            "/jobs/{job_id}/stt",
            post(stages::post_stt).get(stages::get_stt),
        )
        .route("/jobs/{job_id}/stt/progress", get(stages::get_stt_progress))
        .route("/jobs/{job_id}/stt/texts", get(stages::get_stt_texts))
        .route(
            "/jobs/{job_id}/stt/texts/{transcript_id}",
            get(stages::get_stt_text),
        )
        .route(
            "/jobs/{job_id}/summary",
            post(stages::post_summary).get(stages::get_summary),
        )
        .route(
            "/jobs/{job_id}/summary/embedding",
            post(stages::post_summary_embedding).get(stages::get_summary_embedding),
        )
        .route("/jobs/{job_id}/summary/text", get(stages::get_summary_text))
        .route("/summary/search", post(stages::post_summary_search))
        .route("/jobs/{job_id}/files", get(stages::get_job_files))
        .route(
            "/jobs/{job_id}/files/{*file_name}",
            get(stages::get_job_file),
        )
        .with_state(AppState::new(repo_root))
}

pub async fn serve() -> Result<(), String> {
    let repo_root = app::repo_root()?;
    serve_with_runtime_root(repo_root).await
}

pub async fn serve_with_runtime_root(repo_root: PathBuf) -> Result<(), String> {
    let listener = tokio::net::TcpListener::bind(SERVER_BIND)
        .await
        .map_err(|error| format!("failed to bind {SERVER_BIND}: {error}"))?;

    axum::serve(listener, router_with_repo_root(repo_root))
        .await
        .map_err(|error| format!("server error: {error}"))
}

pub(crate) async fn run_blocking_app<T>(
    task: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|error| AppError::internal(format!("blocking task failed: {error}")))?
}

pub(crate) async fn run_blocking<T>(
    task: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|error| format!("blocking task failed: {error}"))?
}

pub(crate) fn error_response(error: AppError) -> Response {
    errors::error_response(error)
}

#[cfg(test)]
pub(crate) use stages::resolve_stt_subset;

#[cfg(test)]
mod tests;
