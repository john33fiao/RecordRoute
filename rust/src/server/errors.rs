use super::types::ErrorResponse;
use crate::error::{AppError, AppErrorKind};
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub(crate) fn status_code(error: &AppError) -> StatusCode {
    match error.kind() {
        AppErrorKind::BadRequest => StatusCode::BAD_REQUEST,
        AppErrorKind::NotFound => StatusCode::NOT_FOUND,
        AppErrorKind::DependencyUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        AppErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(crate) fn error_response(error: AppError) -> Response {
    let status = status_code(&error);
    let body = ErrorResponse {
        code: status.as_u16().to_string(),
        message: error.message().to_string(),
    };
    (status, Json(body)).into_response()
}
