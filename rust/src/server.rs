#[path = "server/app_api.rs"]
mod app_api;
#[path = "server/routes/dictionary.rs"]
mod dictionary;
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
#[path = "server/queue.rs"]
mod queue_dispatcher;
#[path = "server/routes/queue.rs"]
mod queue_routes;
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
use axum::extract::DefaultBodyLimit;
use axum::response::Response;
use axum::routing::MethodRouter;
use axum::routing::{delete, get, post};
use std::path::PathBuf;
use types::AppState;

pub const SERVER_BIND: &str = "127.0.0.1:38080";
pub(crate) const QUEUE_START_PAUSED_ENV_VAR: &str = "RECORDROUTE_QUEUE_START_PAUSED";

pub(crate) fn router_with_repo_root(repo_root: PathBuf) -> Router {
    router_with_repo_root_and_upload_limits(repo_root, upload::DEFAULT_UPLOAD_LIMITS)
}

pub(crate) fn router_with_repo_root_and_upload_limits(
    repo_root: PathBuf,
    upload_limits: upload::UploadLimits,
) -> Router {
    Router::new()
        .route("/", get(web::get_index))
        .route("/app.js", get(web::get_app_js))
        .route("/app.css", get(web::get_app_css))
        .route("/new", get(web::get_new_index))
        .route("/new/", get(web::get_new_index))
        .route("/new/{*path}", get(web::get_new_file))
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
        .route(
            "/jobs",
            post(jobs::post_jobs).get(jobs::get_jobs_with_query),
        )
        .route("/jobs/completed", get(jobs::get_completed_jobs))
        .route("/jobs/upload", jobs_upload_route(upload_limits))
        .route("/jobs/batch-process", post(jobs::post_jobs_batch_process))
        .route("/queue", get(queue_routes::get_queue))
        .route("/queue/pause", post(queue_routes::post_queue_pause))
        .route(
            "/queue/cancel-pending",
            post(queue_routes::post_queue_cancel_pending),
        )
        .route(
            "/dictionary/keywords",
            get(dictionary::get_stt_dictionary_keywords)
                .post(dictionary::post_stt_dictionary_keyword),
        )
        .route(
            "/dictionary/keywords/auto/{keyword}/promote",
            post(dictionary::post_promote_auto_stt_dictionary_keyword),
        )
        .route(
            "/dictionary/keywords/auto/{keyword}",
            delete(dictionary::delete_auto_stt_dictionary_keyword),
        )
        .route(
            "/dictionary/keywords/{keyword}",
            delete(dictionary::delete_stt_dictionary_keyword),
        )
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
        .with_state(AppState::new(repo_root, upload_limits))
}

fn jobs_upload_route(upload_limits: upload::UploadLimits) -> MethodRouter<AppState> {
    post(jobs::post_jobs_upload).layer(DefaultBodyLimit::max(upload_limits.request_max_bytes))
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
