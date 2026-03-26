use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppErrorKind {
    BadRequest,
    NotFound,
    DependencyUnavailable,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppError {
    kind: AppErrorKind,
    message: String,
}

pub(crate) type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub(crate) fn new(kind: AppErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::BadRequest, message)
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::NotFound, message)
    }

    pub(crate) fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::DependencyUnavailable, message)
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::Internal, message)
    }

    pub(crate) fn kind(&self) -> AppErrorKind {
        self.kind
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for AppError {}
