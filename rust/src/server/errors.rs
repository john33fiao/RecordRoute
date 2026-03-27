use super::types::ErrorResponse;
use crate::app;
use crate::error::{AppError, AppErrorKind};
use crate::index::{JobRecord, ModelPreparationRecord, TaskRecord};
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub(crate) const SETUP_REQUIRED_MESSAGE: &str = "환경 준비가 필요합니다. setup을 다시 실행하세요.";

pub(crate) fn sanitize_dependency_message(message: &str) -> String {
    if is_setup_related_error(message) {
        SETUP_REQUIRED_MESSAGE.to_string()
    } else {
        message.to_string()
    }
}

pub(crate) fn sanitize_optional_dependency_message(message: Option<String>) -> Option<String> {
    message.map(|value| sanitize_dependency_message(&value))
}

pub(crate) fn sanitize_task(mut task: TaskRecord) -> TaskRecord {
    task.last_error = sanitize_optional_dependency_message(task.last_error);
    task
}

pub(crate) fn sanitize_job(mut job: JobRecord) -> JobRecord {
    job.error_message = sanitize_optional_dependency_message(job.error_message);
    job.tasks = job.tasks.into_iter().map(sanitize_task).collect();
    job
}

pub(crate) fn sanitize_jobs(jobs: Vec<JobRecord>) -> Vec<JobRecord> {
    jobs.into_iter().map(sanitize_job).collect()
}

pub(crate) fn sanitize_model_preparation(
    mut preparation: ModelPreparationRecord,
) -> ModelPreparationRecord {
    preparation.last_error = sanitize_optional_dependency_message(preparation.last_error);
    preparation
}

pub(crate) fn sanitize_model_status_snapshot(
    mut snapshot: app::ModelStatusSnapshot,
) -> app::ModelStatusSnapshot {
    snapshot.whisper = sanitize_model_status_entry(snapshot.whisper);
    snapshot.llama = sanitize_model_status_entry(snapshot.llama);
    snapshot
}

fn sanitize_model_status_entry(mut entry: app::ModelStatusEntry) -> app::ModelStatusEntry {
    entry.error = sanitize_optional_dependency_message(entry.error);
    entry.embedding_error = sanitize_optional_dependency_message(entry.embedding_error);
    entry.preparation = sanitize_model_preparation(entry.preparation);
    entry
}

pub(crate) fn sanitize_system_errors(errors: Vec<String>) -> Vec<String> {
    let mut sanitized = Vec::new();
    for error in errors {
        let sanitized_error = sanitize_dependency_message(&error);
        if !sanitized
            .iter()
            .any(|existing| existing == &sanitized_error)
        {
            sanitized.push(sanitized_error);
        }
    }
    sanitized
}

pub(crate) fn is_setup_related_error(message: &str) -> bool {
    [
        "local ffmpeg toolchain not found.",
        "local whisper toolchain not found.",
        "local llama toolchain not found.",
        "local llama embedding toolchain not found.",
        "failed to execute ffmpeg ",
        "failed to execute ffprobe ",
        "failed to execute whisper-cli ",
        "failed to execute whisper model download script ",
        "failed to download whisper model ",
        "failed to remove invalid whisper model cache ",
        "failed to create whisper model directory ",
        "whisper model not found at ",
        "whisper model path has no parent directory:",
        "failed to execute llama-cli ",
        "failed to execute llama-embedding ",
        "failed to download llama model ",
        "failed to create llama model cache directory ",
        "failed to create llama download cache directory ",
        "failed to move downloaded llama model ",
        "failed to read llama cache directory ",
        "failed to inspect llama cache directory entry in ",
        "downloaded llama model was not written to expected cache path ",
        "llama download cache unexpectedly became empty:",
        "llama cache path has no parent directory:",
        "llama model file not found:",
        "llama model cache path is unavailable",
        "llama model cache not found for ",
        "llama embedding model file not found:",
        "llama embedding model cache path is unavailable",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix))
}

pub(crate) fn status_code(error: &AppError) -> StatusCode {
    match error.kind() {
        AppErrorKind::BadRequest => StatusCode::BAD_REQUEST,
        AppErrorKind::NotFound => StatusCode::NOT_FOUND,
        AppErrorKind::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        AppErrorKind::DependencyUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        AppErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(crate) fn error_response(error: AppError) -> Response {
    let status = status_code(&error);
    let message = match error.kind() {
        AppErrorKind::DependencyUnavailable => sanitize_dependency_message(error.message()),
        _ => error.message().to_string(),
    };
    let body = ErrorResponse {
        code: status.as_u16().to_string(),
        message,
    };
    (status, Json(body)).into_response()
}
