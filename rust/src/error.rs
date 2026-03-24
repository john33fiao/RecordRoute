use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error("internal server error")]
    Internal(String),
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn internal<E>(error: E) -> Self
    where
        E: std::fmt::Display,
    {
        Self::Internal(error.to_string())
    }
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    error: &'a str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message.as_str()),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message.as_str()),
            Self::Internal(message) => {
                tracing::error!("{message}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
            }
        };

        (status, Json(ErrorEnvelope { error: message })).into_response()
    }
}
