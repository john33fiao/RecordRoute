use std::error::Error;
use std::fmt;

pub(crate) const SETUP_REQUIRED_MESSAGE: &str = "환경 준비가 필요합니다. setup을 다시 실행하세요.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppErrorKind {
    BadRequest,
    NotFound,
    PayloadTooLarge,
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

    pub(crate) fn payload_too_large(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::PayloadTooLarge, message)
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

pub(crate) fn is_dependency_unavailable_message(message: &str) -> bool {
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

pub(crate) fn sanitize_dependency_message(message: &str) -> String {
    if is_dependency_unavailable_message(message) {
        SETUP_REQUIRED_MESSAGE.to_string()
    } else {
        message.to_string()
    }
}

pub(crate) fn dependency_unavailable_or_internal(message: impl Into<String>) -> AppError {
    let message = message.into();
    if is_dependency_unavailable_message(&message) {
        AppError::dependency_unavailable(message)
    } else {
        AppError::internal(message)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for AppError {}
