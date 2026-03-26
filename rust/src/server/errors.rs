use super::ErrorResponse;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub(crate) fn classify_upload_job_error(error: &str) -> StatusCode {
    if error.starts_with("multipart field 'file' is required")
        || error.starts_with("multipart field 'file' must appear only once")
        || error.starts_with("uploaded file is empty")
        || error.starts_with("input file not found:")
        || error.starts_with("input path is not a file:")
        || error.starts_with("failed to resolve input path ")
    {
        StatusCode::BAD_REQUEST
    } else if error.starts_with("local ffmpeg toolchain not found.") {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

pub(crate) fn classify_create_job_error(error: &str) -> StatusCode {
    if error.starts_with("input file not found:")
        || error.starts_with("input path is not a file:")
        || error.starts_with("failed to resolve input path ")
    {
        StatusCode::BAD_REQUEST
    } else if error.starts_with("local ffmpeg toolchain not found.") {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

pub(crate) fn classify_model_prepare_error(error: &str) -> StatusCode {
    if error.starts_with("local whisper toolchain not found.")
        || error.starts_with("local llama toolchain not found.")
        || error.starts_with("whisper model not found at ")
        || error.starts_with("whisper model path has no parent directory:")
        || error.starts_with("llama model file not found:")
        || error.starts_with("llama model cache path is unavailable")
    {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

pub(crate) fn error_response(status: StatusCode, message: impl Into<String>) -> Response {
    let body = ErrorResponse {
        code: status.as_u16().to_string(),
        message: message.into(),
    };
    (status, Json(body)).into_response()
}
