use std::collections::BTreeMap;
use std::fs;
use std::sync::{Arc, RwLock};

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{routing::get, Json, Router};
use recordroute_core::config::AppConfig;
use recordroute_core::error::CoreError;
use recordroute_storage::{HistoryRecord, ResolvedFile, SegmentItem, StorageContext};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub storage: StorageContext,
    task_registry: Arc<RwLock<BTreeMap<String, TaskProgressSnapshot>>>,
}

impl AppState {
    pub fn from_env() -> Self {
        Self::from_config(AppConfig::from_env())
    }

    pub fn from_config(config: AppConfig) -> Self {
        Self::with_task_snapshots(config, Vec::new())
    }

    pub fn with_task_snapshots<I>(config: AppConfig, tasks: I) -> Self
    where
        I: IntoIterator<Item = TaskProgressSnapshot>,
    {
        let task_registry = tasks
            .into_iter()
            .map(|task| (task.task_id.clone(), task))
            .collect();

        Self {
            storage: StorageContext::new(config.clone()),
            config,
            task_registry: Arc::new(RwLock::new(task_registry)),
        }
    }

    pub fn task_snapshots(&self) -> Vec<TaskProgressSnapshot> {
        self.task_registry
            .read()
            .expect("task registry lock poisoned")
            .values()
            .cloned()
            .collect()
    }

    pub fn task_progress(&self, task_id: &str) -> Result<TaskProgressSnapshot, CoreError> {
        self.task_registry
            .read()
            .expect("task registry lock poisoned")
            .get(task_id)
            .cloned()
            .ok_or_else(|| CoreError::TaskNotFound {
                task_id: task_id.to_string(),
            })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StandardErrorPayload {
    pub message: String,
    pub code: Option<String>,
    pub retryable: Option<bool>,
    pub failed_step: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TaskProgressSnapshot {
    pub task_id: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_percent: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_step: Option<String>,
    pub error: Option<StandardErrorPayload>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse<'a> {
    pub status: &'a str,
    pub db_root: String,
    pub model_root: String,
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/history", get(history))
        .route("/tasks", get(tasks))
        .route("/progress/{task_id}", get(progress))
        .route("/segments/{*file_identifier}", get(segments))
        .route("/download/{*uuid_or_path}", get(download))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse<'static>> {
    Json(HealthResponse {
        status: "ok",
        db_root: state.config.db_root.display().to_string(),
        model_root: state.config.model_root.display().to_string(),
    })
}

async fn history(State(state): State<AppState>) -> Result<Json<Vec<HistoryRecord>>, ApiError> {
    Ok(Json(state.storage.load_upload_history()?))
}

async fn tasks(State(state): State<AppState>) -> Json<Vec<TaskProgressSnapshot>> {
    Json(state.task_snapshots())
}

async fn progress(
    Path(task_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<TaskProgressSnapshot>, ApiError> {
    Ok(Json(state.task_progress(&task_id)?))
}

async fn segments(
    Path(file_identifier): Path<String>,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    match state.storage.load_segments(&file_identifier)? {
        Some(items) => Ok(Json(items).into_response()),
        None => Ok((StatusCode::NOT_FOUND, Json(Vec::<SegmentItem>::new())).into_response()),
    }
}

async fn download(
    Path(uuid_or_path): Path<String>,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let Some(resolved_file) = state.storage.resolve_file_identifier(&uuid_or_path)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    build_download_response(&resolved_file).map_err(ApiError::from)
}

fn build_download_response(resolved_file: &ResolvedFile) -> Result<Response, CoreError> {
    let bytes = fs::read(&resolved_file.full_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            CoreError::FileNotFound {
                path: resolved_file.full_path.clone(),
            }
        } else {
            CoreError::Io(error)
        }
    })?;

    let content_type = mime_guess::from_path(&resolved_file.full_path)
        .first_or_octet_stream()
        .to_string();
    let filename = resolved_file
        .original_filename
        .clone()
        .or_else(|| {
            resolved_file
                .full_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "download.bin".to_string());

    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"{}\"",
            sanitize_filename(&filename)
        ))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
    );

    Ok(response)
}

fn sanitize_filename(filename: &str) -> String {
    filename.replace(['"', '\\'], "_")
}

#[derive(Debug)]
struct ApiError(CoreError);

impl From<CoreError> for ApiError {
    fn from(value: CoreError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self.0 {
            CoreError::InvalidRecordPath => (
                StatusCode::BAD_REQUEST,
                "invalid_record_path",
                "record path must start with DB/".to_string(),
            ),
            CoreError::RecordPathEscapesDbRoot { path } => {
                (StatusCode::BAD_REQUEST, "invalid_record_path", path)
            }
            CoreError::InvalidFileIdentifier => (
                StatusCode::BAD_REQUEST,
                "invalid_file_identifier",
                "file identifier must be a non-empty UUID or DB path".to_string(),
            ),
            CoreError::TaskNotFound { task_id } => (
                StatusCode::NOT_FOUND,
                "task_not_found",
                format!("task not found: {task_id}"),
            ),
            CoreError::FileNotFound { path } => (
                StatusCode::NOT_FOUND,
                "file_not_found",
                format!("file not found: {}", path.display()),
            ),
            CoreError::Io(error) if error.kind() == std::io::ErrorKind::NotFound => {
                (StatusCode::NOT_FOUND, "file_not_found", error.to_string())
            }
            CoreError::Io(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "io_error",
                error.to_string(),
            ),
            CoreError::Json(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "json_error",
                error.to_string(),
            ),
        };

        (
            status,
            Json(ErrorEnvelope {
                success: false,
                error: ErrorDetail {
                    code: code.to_string(),
                    message,
                },
            }),
        )
            .into_response()
    }
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    success: bool,
    error: ErrorDetail,
}

#[derive(Debug, Serialize)]
struct ErrorDetail {
    code: String,
    message: String,
}

pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::{ApiError, StandardErrorPayload, TaskProgressSnapshot};
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use recordroute_core::error::CoreError;

    #[test]
    fn maps_task_not_found_to_404() {
        let response = ApiError::from(CoreError::TaskNotFound {
            task_id: "missing-task".to_string(),
        })
        .into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn keeps_error_field_serializable_when_null_or_present() {
        let without_error = TaskProgressSnapshot {
            task_id: "task".to_string(),
            message: "running".to_string(),
            stage: Some("summary".to_string()),
            progress_percent: Some(66),
            eta_seconds: Some(12),
            error_code: None,
            retryable: None,
            failed_step: None,
            error: None,
        };
        let with_error = TaskProgressSnapshot {
            error: Some(StandardErrorPayload {
                message: "failed".to_string(),
                code: Some("fatal_error".to_string()),
                retryable: Some(false),
                failed_step: Some("summary".to_string()),
            }),
            ..without_error.clone()
        };

        let without_error_json = serde_json::to_value(&without_error).unwrap();
        let with_error_json = serde_json::to_value(&with_error).unwrap();

        assert!(without_error_json.get("error").unwrap().is_null());
        assert_eq!(
            with_error_json["error"]["code"],
            serde_json::Value::String("fatal_error".to_string())
        );
    }
}
