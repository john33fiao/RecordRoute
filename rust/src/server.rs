use crate::app::{
    self, execute_ffmpeg_job, execute_stt_job, execute_summary_job, submit_ffmpeg_job,
    submit_stt_job, submit_summary_job,
};
use crate::index::{IndexStore, JobOutputs, JobProbe, JobRecord, JobStatus, TaskRecord, TaskType};
use axum::extract::Path as AxumPath;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router, body::Body};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const SERVER_BIND: &str = "127.0.0.1:38080";

#[derive(Debug, Clone, Deserialize)]
pub struct PingRequest {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PingResponse {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateJobRequest {
    input_path: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SummaryRequest {
    #[serde(default)]
    force_regenerate: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct SttRequest {
    #[serde(default)]
    audio_files: Vec<String>,
    #[serde(default)]
    mono_mix_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TaskSubmissionResponse {
    pub job_id: String,
    pub task_type: TaskType,
    pub status: String,
    pub message: String,
    pub reused: bool,
    pub deduplicated: bool,
    pub task: Option<TaskRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct FileListResponse {
    pub job_id: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct JobStatusResponse {
    pub job_id: String,
    pub job_status: JobStatus,
    pub tasks: Vec<TaskRecord>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobSubmissionResponse {
    pub job_id: String,
    pub status: JobStatus,
    pub message: String,
    pub reused: bool,
    pub deduplicated: bool,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub source_path: String,
    pub source_file_name: String,
    pub probe: JobProbe,
    pub outputs: JobOutputs,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobListResponse {
    pub jobs: Vec<JobRecord>,
}

#[derive(Debug, Clone)]
struct AppState {
    repo_root: PathBuf,
    invalid_request_body: ErrorResponse,
}

pub fn router() -> Router {
    let repo_root = app::repo_root().expect("failed to resolve repository root for server");
    router_with_repo_root(repo_root)
}

pub(crate) fn router_with_repo_root(repo_root: PathBuf) -> Router {
    Router::new()
        .route("/server/ping", post(post_server_ping))
        .route("/jobs", post(post_jobs).get(get_jobs))
        .route("/jobs/{job_id}", get(get_job))
        .route("/jobs/{job_id}/status", get(get_job_status))
        .route("/jobs/{job_id}/stt", post(post_stt).get(get_stt))
        .route(
            "/jobs/{job_id}/summary",
            post(post_summary).get(get_summary),
        )
        .route("/jobs/{job_id}/files", get(get_job_files))
        .route("/jobs/{job_id}/files/{*file_name}", get(get_job_file))
        .with_state(AppState {
            repo_root,
            invalid_request_body: ErrorResponse {
                code: "400".to_string(),
                message: "invalid request body".to_string(),
            },
        })
}

pub async fn serve() -> Result<(), String> {
    let listener = tokio::net::TcpListener::bind(SERVER_BIND)
        .await
        .map_err(|error| format!("failed to bind {SERVER_BIND}: {error}"))?;

    axum::serve(listener, router())
        .await
        .map_err(|error| format!("server error: {error}"))
}

async fn post_server_ping(
    State(state): State<AppState>,
    payload: Result<Json<PingRequest>, JsonRejection>,
) -> Response {
    match payload {
        Ok(Json(request)) => {
            let _request_code = request.code;
            let response = PingResponse {
                code: "200".to_string(),
                message: format!("Welcome... The message you sent - {}", request.message),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(state.invalid_request_body.clone()),
        )
            .into_response(),
    }
}

async fn post_jobs(
    State(state): State<AppState>,
    payload: Result<Json<CreateJobRequest>, JsonRejection>,
) -> Response {
    let request = match payload {
        Ok(Json(request)) => request,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(state.invalid_request_body.clone()),
            )
                .into_response();
        }
    };

    let repo_root = state.repo_root.clone();
    let input_path = PathBuf::from(request.input_path);
    let submission = match run_blocking(move || submit_ffmpeg_job(&repo_root, &input_path)).await {
        Ok(submission) => submission,
        Err(error) => {
            let status = classify_create_job_error(&error);
            return error_response(status, error);
        }
    };

    if submission.should_execute() {
        let repo_root = state.repo_root.clone();
        let job_id = submission.job.job_id.clone();
        let input_path = submission.input_path.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(error) = execute_ffmpeg_job(&repo_root, &job_id, &input_path) {
                eprintln!("{error}");
            }
        });
    }

    let status = if submission.reused() {
        StatusCode::OK
    } else {
        StatusCode::ACCEPTED
    };
    let response = build_job_submission_response(
        &submission.job,
        submission.reused(),
        submission.deduplicated(),
    );

    (status, Json(response)).into_response()
}

async fn get_jobs(State(state): State<AppState>) -> Response {
    let repo_root = state.repo_root.clone();
    match run_blocking(move || IndexStore::new(&repo_root).list_jobs()).await {
        Ok(jobs) => Json(JobListResponse { jobs }).into_response(),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, error),
    }
}

async fn get_job(State(state): State<AppState>, AxumPath(job_id): AxumPath<String>) -> Response {
    let repo_root = state.repo_root.clone();
    let job_id_for_lookup = job_id.clone();

    match run_blocking(move || IndexStore::new(&repo_root).find_job(&job_id_for_lookup)).await {
        Ok(Some(job)) => Json(job).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, format!("job not found: {job_id}")),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, error),
    }
}

async fn get_job_status(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let lookup_job_id = job_id.clone();

    match run_blocking(move || IndexStore::new(&repo_root).find_job(&lookup_job_id)).await {
        Ok(Some(job)) => Json(JobStatusResponse {
            job_id,
            job_status: job.status,
            tasks: job.tasks,
            error_message: job.error_message,
        })
        .into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, format!("job not found: {job_id}")),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, error),
    }
}

async fn post_stt(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
    payload: Result<Json<SttRequest>, JsonRejection>,
) -> Response {
    let subset = match payload {
        Ok(Json(request)) => match resolve_stt_subset(request) {
            Ok(subset) => subset,
            Err(error) => return error_response(StatusCode::BAD_REQUEST, error),
        },
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(state.invalid_request_body.clone()),
            )
                .into_response();
        }
    };
    let repo_root = state.repo_root.clone();
    let submit_job_id = job_id.clone();
    let submission =
        match run_blocking(move || submit_stt_job(&repo_root, &submit_job_id, subset)).await {
            Ok(submission) => submission,
            Err(error) => return error_response(StatusCode::BAD_REQUEST, error),
        };

    if submission.should_execute() {
        let repo_root = state.repo_root.clone();
        let execute_job_id = job_id.clone();
        let planned_audio_files = submission.planned_audio_files.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(error) = execute_stt_job(&repo_root, &execute_job_id, &planned_audio_files) {
                if let Ok(store_job) = IndexStore::new(&repo_root).find_job(&execute_job_id)
                    && let Some(mut job) = store_job
                {
                    if let Ok(now) = crate::app::now_rfc3339() {
                        job.fail_task(TaskType::Stt, now, error.clone());
                        let _ = IndexStore::new(&repo_root)
                            .update_job(&execute_job_id, |_| job.clone());
                    }
                }
                eprintln!("{error}");
            }
        });
    }

    let task = submission.job.task(TaskType::Stt).cloned();
    let body = TaskSubmissionResponse {
        job_id: job_id.clone(),
        task_type: TaskType::Stt,
        status: "accepted".to_string(),
        message: if submission.reused() {
            "stt outputs reused".to_string()
        } else if submission.deduplicated() {
            "stt already running".to_string()
        } else {
            "stt accepted".to_string()
        },
        reused: submission.reused(),
        deduplicated: submission.deduplicated(),
        task,
    };
    (StatusCode::ACCEPTED, Json(body)).into_response()
}

fn resolve_stt_subset(request: SttRequest) -> Result<Option<Vec<String>>, String> {
    if request.mono_mix_only && !request.audio_files.is_empty() {
        return Err("audio_files cannot be combined with mono_mix_only".to_string());
    }
    if request.mono_mix_only {
        return Ok(Some(vec!["mono_mix.wav".to_string()]));
    }
    if request.audio_files.is_empty() {
        return Ok(None);
    }
    Ok(Some(request.audio_files))
}

async fn get_stt(State(state): State<AppState>, AxumPath(job_id): AxumPath<String>) -> Response {
    let repo_root = state.repo_root.clone();
    let lookup_job_id = job_id.clone();
    let job = match run_blocking(move || IndexStore::new(&repo_root).find_job(&lookup_job_id)).await
    {
        Ok(Some(job)) => job,
        Ok(None) => {
            return error_response(StatusCode::NOT_FOUND, format!("job not found: {job_id}"));
        }
        Err(error) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, error),
    };

    let body = TaskSubmissionResponse {
        job_id,
        task_type: TaskType::Stt,
        status: "ok".to_string(),
        message: "stt task status".to_string(),
        reused: false,
        deduplicated: false,
        task: job.task(TaskType::Stt).cloned(),
    };
    Json(body).into_response()
}

async fn post_summary(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
    payload: Result<Json<SummaryRequest>, JsonRejection>,
) -> Response {
    let force_regenerate = match payload {
        Ok(Json(request)) => request.force_regenerate,
        Err(_) => false,
    };
    let repo_root = state.repo_root.clone();
    let submit_job_id = job_id.clone();
    let submission = match run_blocking(move || {
        submit_summary_job(&repo_root, &submit_job_id, force_regenerate)
    })
    .await
    {
        Ok(submission) => submission,
        Err(error) => return error_response(StatusCode::BAD_REQUEST, error),
    };

    if submission.should_execute() {
        let repo_root = state.repo_root.clone();
        let execute_job_id = job_id.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(error) = execute_summary_job(&repo_root, &execute_job_id, force_regenerate) {
                if let Ok(store_job) = IndexStore::new(&repo_root).find_job(&execute_job_id)
                    && let Some(mut job) = store_job
                {
                    if let Ok(now) = crate::app::now_rfc3339() {
                        job.fail_task(TaskType::Summary, now, error.clone());
                        let _ = IndexStore::new(&repo_root)
                            .update_job(&execute_job_id, |_| job.clone());
                    }
                }
                eprintln!("{error}");
            }
        });
    }

    let body = TaskSubmissionResponse {
        job_id: job_id.clone(),
        task_type: TaskType::Summary,
        status: "accepted".to_string(),
        message: if submission.reused() {
            "summary reused".to_string()
        } else if submission.deduplicated() {
            "summary already running".to_string()
        } else {
            "summary accepted".to_string()
        },
        reused: submission.reused(),
        deduplicated: submission.deduplicated(),
        task: submission.job.task(TaskType::Summary).cloned(),
    };
    (StatusCode::ACCEPTED, Json(body)).into_response()
}

async fn get_summary(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let lookup_job_id = job_id.clone();
    let job = match run_blocking(move || IndexStore::new(&repo_root).find_job(&lookup_job_id)).await
    {
        Ok(Some(job)) => job,
        Ok(None) => {
            return error_response(StatusCode::NOT_FOUND, format!("job not found: {job_id}"));
        }
        Err(error) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, error),
    };

    Json(TaskSubmissionResponse {
        job_id,
        task_type: TaskType::Summary,
        status: "ok".to_string(),
        message: "summary task status".to_string(),
        reused: false,
        deduplicated: false,
        task: job.task(TaskType::Summary).cloned(),
    })
    .into_response()
}

async fn get_job_files(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let lookup_job_id = job_id.clone();
    let files = match run_blocking(move || collect_job_files(&repo_root, &lookup_job_id)).await {
        Ok(files) => files,
        Err(error) if error.starts_with("job not found:") => {
            return error_response(StatusCode::NOT_FOUND, error);
        }
        Err(error) => return error_response(StatusCode::BAD_REQUEST, error),
    };
    Json(FileListResponse { job_id, files }).into_response()
}

async fn get_job_file(
    State(state): State<AppState>,
    AxumPath((job_id, file_name)): AxumPath<(String, String)>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let lookup_job_id = job_id.clone();
    let lookup_file = file_name.clone();
    let result =
        match run_blocking(move || read_job_file(&repo_root, &lookup_job_id, &lookup_file)).await {
            Ok(result) => result,
            Err(error) if error.contains("not found") => {
                return error_response(StatusCode::NOT_FOUND, error);
            }
            Err(error) => return error_response(StatusCode::BAD_REQUEST, error),
        };
    (StatusCode::OK, Body::from(result)).into_response()
}

async fn run_blocking<T>(
    task: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|error| format!("blocking task failed: {error}"))?
}

fn build_job_submission_response(
    job: &JobRecord,
    reused: bool,
    deduplicated: bool,
) -> JobSubmissionResponse {
    JobSubmissionResponse {
        job_id: job.job_id.clone(),
        status: job.status.clone(),
        message: if reused {
            "completed job reused".to_string()
        } else if deduplicated {
            "job already running".to_string()
        } else {
            "job accepted".to_string()
        },
        reused,
        deduplicated,
        started_at: job.started_at.clone(),
        finished_at: job.finished_at.clone(),
        source_path: job.source_path.clone(),
        source_file_name: job.source_file_name.clone(),
        probe: job.probe.clone(),
        outputs: job.outputs.clone(),
        error_message: job.error_message.clone(),
    }
}

fn classify_create_job_error(error: &str) -> StatusCode {
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

fn collect_job_files(repo_root: &std::path::Path, job_id: &str) -> Result<Vec<String>, String> {
    let store = IndexStore::new(repo_root);
    let job = store
        .find_job(job_id)?
        .ok_or_else(|| format!("job not found: {job_id}"))?;
    let job_dir = PathBuf::from(job.job_dir);
    let mut files = Vec::new();

    let root_candidates = ["mono_mix.wav"];
    for file in root_candidates {
        let path = job_dir.join(file);
        if path.is_file() {
            files.push(file.to_string());
        }
    }
    for entry in fs::read_dir(&job_dir)
        .map_err(|error| format!("failed to read {}: {error}", job_dir.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to read job dir entry: {error}"))?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == "wav")
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("channel_"))
        {
            files.push(
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_string(),
            );
        }
    }
    for sub in ["stt", "summary"] {
        let sub_dir = job_dir.join(sub);
        if !sub_dir.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&sub_dir)
            .map_err(|error| format!("failed to read {}: {error}", sub_dir.display()))?
        {
            let entry = entry.map_err(|error| format!("failed to read {sub} entry: {error}"))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                files.push(format!("{sub}/{name}"));
            }
        }
    }
    files.sort();
    Ok(files)
}

fn read_job_file(
    repo_root: &std::path::Path,
    job_id: &str,
    file_name: &str,
) -> Result<Vec<u8>, String> {
    let store = IndexStore::new(repo_root);
    let job = store
        .find_job(job_id)?
        .ok_or_else(|| format!("job not found: {job_id}"))?;

    let path = sanitize_file_name(file_name)?;
    let allowed_prefix = ["stt/", "summary/"];
    let is_allowed = path == "mono_mix.wav"
        || path.starts_with("channel_")
        || allowed_prefix.iter().any(|prefix| path.starts_with(prefix));
    if !is_allowed {
        return Err(format!("file not allowed: {file_name}"));
    }

    let absolute = PathBuf::from(job.job_dir).join(path);
    fs::read(&absolute).map_err(|error| format!("file not found: {} ({error})", absolute.display()))
}

fn sanitize_file_name(file_name: &str) -> Result<&str, String> {
    if file_name.contains('\\') || file_name.starts_with('/') || file_name.contains("..") {
        return Err("invalid file path".to_string());
    }
    Ok(file_name)
}

fn error_response(status: StatusCode, message: impl Into<String>) -> Response {
    let body = ErrorResponse {
        code: status.as_u16().to_string(),
        message: message.into(),
    };
    (status, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use http_body_util::BodyExt;
    use std::fs::{self, File};
    use std::path::Path;
    use std::time::{Duration, Instant};
    use tower::util::ServiceExt;
    use uuid::Uuid;

    #[test]
    fn resolve_stt_subset_defaults_to_all_audio_files() {
        let subset = resolve_stt_subset(SttRequest {
            audio_files: Vec::new(),
            mono_mix_only: false,
        })
        .expect("subset");
        assert_eq!(subset, None);
    }

    #[test]
    fn resolve_stt_subset_supports_mono_mix_only_mode() {
        let subset = resolve_stt_subset(SttRequest {
            audio_files: Vec::new(),
            mono_mix_only: true,
        })
        .expect("subset");
        assert_eq!(subset, Some(vec!["mono_mix.wav".to_string()]));
    }

    #[test]
    fn resolve_stt_subset_rejects_mixed_modes() {
        let error = resolve_stt_subset(SttRequest {
            audio_files: vec!["channel_01.wav".to_string()],
            mono_mix_only: true,
        })
        .expect_err("mixed mode should fail");
        assert!(error.contains("audio_files cannot be combined with mono_mix_only"));
    }

    #[tokio::test]
    async fn ping_returns_expected_success_payload() {
        let app = router();
        let request = Request::builder()
            .method(Method::POST)
            .uri("/server/ping")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"code":"100","message":"ping test"}"#))
            .expect("request");

        let response = app.oneshot(request).await.expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect")
            .to_bytes();
        assert_eq!(
            std::str::from_utf8(&body).expect("utf8"),
            r#"{"code":"200","message":"Welcome... The message you sent - ping test"}"#
        );
    }

    #[tokio::test]
    async fn ping_returns_400_for_malformed_json() {
        let app = router();
        let request = Request::builder()
            .method(Method::POST)
            .uri("/server/ping")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"code":"100","message":"ping test""#))
            .expect("request");

        let response = app.oneshot(request).await.expect("response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect")
            .to_bytes();
        assert_eq!(
            std::str::from_utf8(&body).expect("utf8"),
            r#"{"code":"400","message":"invalid request body"}"#
        );
    }

    #[tokio::test]
    async fn ping_returns_400_when_message_is_missing() {
        let app = router();
        let request = Request::builder()
            .method(Method::POST)
            .uri("/server/ping")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"code":"100"}"#))
            .expect("request");

        let response = app.oneshot(request).await.expect("response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect")
            .to_bytes();
        assert_eq!(
            std::str::from_utf8(&body).expect("utf8"),
            r#"{"code":"400","message":"invalid request body"}"#
        );
    }

    #[tokio::test]
    async fn ping_returns_400_when_code_is_missing() {
        let app = router();
        let request = Request::builder()
            .method(Method::POST)
            .uri("/server/ping")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"message":"ping test"}"#))
            .expect("request");

        let response = app.oneshot(request).await.expect("response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect")
            .to_bytes();
        assert_eq!(
            std::str::from_utf8(&body).expect("utf8"),
            r#"{"code":"400","message":"invalid request body"}"#
        );
    }

    #[tokio::test]
    async fn ping_rejects_get_method() {
        let app = router();
        let request = Request::builder()
            .method(Method::GET)
            .uri("/server/ping")
            .body(Body::empty())
            .expect("request");

        let response = app.oneshot(request).await.expect("response");

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn post_jobs_returns_accepted_then_job_transitions_to_completed() {
        let repo_root = temp_workspace();
        let scripts_dir = repo_root.join("scripts");
        let build_bin = repo_root
            .join(".build/ffmpeg")
            .join(crate::ffmpeg::target_dir_name())
            .join("install/bin");
        let input = repo_root.join("fixture.wav");
        let gate = repo_root.join("ffmpeg.gate");

        fs::create_dir_all(&scripts_dir).expect("scripts dir");
        fs::create_dir_all(&build_bin).expect("toolchain dir");
        write_build_script(&build_script_path(&repo_root, "ffmpeg"));
        write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
        write_blocking_ffmpeg(&fake_command_path(&build_bin, "ffmpeg"), &gate);
        write_test_wav(&input, 2);
        fs::write(&gate, "wait").expect("gate");

        let app = router_with_repo_root(repo_root.clone());

        let response = app
            .clone()
            .oneshot(post_json_request(
                "/jobs",
                &serde_json::json!({ "input_path": input }),
            ))
            .await
            .expect("post jobs response");

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let submitted: JobSubmissionResponse = read_json(response).await;
        assert_eq!(submitted.status, JobStatus::Running);
        assert!(!submitted.reused);
        assert!(!submitted.deduplicated);

        let running: JobRecord = read_json(
            app.clone()
                .oneshot(get_request(&format!("/jobs/{}", submitted.job_id)))
                .await
                .expect("get job response"),
        )
        .await;
        assert_eq!(running.status, JobStatus::Running);

        fs::remove_file(&gate).expect("remove gate");
        let completed = wait_for_job_completion(&app, &submitted.job_id).await;

        assert_eq!(completed.status, JobStatus::Completed);
        assert_eq!(completed.probe.channels, Some(2));
        assert!(
            Path::new(
                completed
                    .outputs
                    .merged_mono_wav
                    .as_deref()
                    .expect("merged output"),
            )
            .is_file()
        );
        assert_eq!(completed.outputs.split_mono_wavs.len(), 2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn post_jobs_deduplicates_running_job() {
        let repo_root = temp_workspace();
        let scripts_dir = repo_root.join("scripts");
        let build_bin = repo_root
            .join(".build/ffmpeg")
            .join(crate::ffmpeg::target_dir_name())
            .join("install/bin");
        let input = repo_root.join("fixture.wav");
        let gate = repo_root.join("ffmpeg.gate");

        fs::create_dir_all(&scripts_dir).expect("scripts dir");
        fs::create_dir_all(&build_bin).expect("toolchain dir");
        write_build_script(&build_script_path(&repo_root, "ffmpeg"));
        write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 1, Some("mono"));
        write_blocking_ffmpeg(&fake_command_path(&build_bin, "ffmpeg"), &gate);
        write_test_wav(&input, 1);
        fs::write(&gate, "wait").expect("gate");

        let app = router_with_repo_root(repo_root.clone());

        let first: JobSubmissionResponse = read_json(
            app.clone()
                .oneshot(post_json_request(
                    "/jobs",
                    &serde_json::json!({ "input_path": input }),
                ))
                .await
                .expect("first response"),
        )
        .await;
        let second: JobSubmissionResponse = read_json(
            app.clone()
                .oneshot(post_json_request(
                    "/jobs",
                    &serde_json::json!({ "input_path": input }),
                ))
                .await
                .expect("second response"),
        )
        .await;

        assert_eq!(first.job_id, second.job_id);
        assert_eq!(second.status, JobStatus::Running);
        assert!(!second.reused);
        assert!(second.deduplicated);

        fs::remove_file(&gate).expect("remove gate");
        let completed = wait_for_job_completion(&app, &first.job_id).await;
        assert_eq!(completed.status, JobStatus::Completed);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn post_jobs_reuses_completed_job() {
        let repo_root = temp_workspace();
        let scripts_dir = repo_root.join("scripts");
        let build_bin = repo_root
            .join(".build/ffmpeg")
            .join(crate::ffmpeg::target_dir_name())
            .join("install/bin");
        let input = repo_root.join("fixture.wav");

        fs::create_dir_all(&scripts_dir).expect("scripts dir");
        fs::create_dir_all(&build_bin).expect("toolchain dir");
        write_build_script(&build_script_path(&repo_root, "ffmpeg"));
        write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
        write_fake_ffmpeg(&fake_command_path(&build_bin, "ffmpeg"));
        write_test_wav(&input, 2);

        crate::app::run_with_repo_root(&repo_root, &input).expect("seed completed job");

        fs::remove_file(fake_command_path(&build_bin, "ffmpeg")).expect("remove ffmpeg");
        fs::remove_file(fake_command_path(&build_bin, "ffprobe")).expect("remove ffprobe");

        let app = router_with_repo_root(repo_root);
        let response = app
            .oneshot(post_json_request(
                "/jobs",
                &serde_json::json!({ "input_path": input }),
            ))
            .await
            .expect("reuse response");

        assert_eq!(response.status(), StatusCode::OK);
        let body: JobSubmissionResponse = read_json(response).await;
        assert_eq!(body.status, JobStatus::Completed);
        assert!(body.reused);
        assert!(!body.deduplicated);
        assert!(body.outputs.merged_mono_wav.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_jobs_returns_latest_first() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);

        store
            .insert_job(JobRecord::new(
                "job-1".to_string(),
                "2026-01-01T00:00:00Z".to_string(),
                PathBuf::from("/tmp/first.wav"),
                store.job_dir("job-1"),
            ))
            .expect("insert first job");
        store
            .insert_job(JobRecord::new(
                "job-2".to_string(),
                "2026-01-01T00:00:01Z".to_string(),
                PathBuf::from("/tmp/second.wav"),
                store.job_dir("job-2"),
            ))
            .expect("insert second job");

        let app = router_with_repo_root(repo_root);
        let response = app
            .oneshot(get_request("/jobs"))
            .await
            .expect("get jobs response");

        assert_eq!(response.status(), StatusCode::OK);
        let body: JobListResponse = read_json(response).await;
        assert_eq!(body.jobs.len(), 2);
        assert_eq!(body.jobs[0].job_id, "job-2");
        assert_eq!(body.jobs[1].job_id, "job-1");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_job_status_returns_integrated_stage_status() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);

        let mut job = JobRecord::new(
            "job-status".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/status.wav"),
            store.job_dir("job-status"),
        );
        job.upsert_running_task(TaskType::Stt, "2026-01-01T00:00:01Z".to_string());
        job.complete_task(TaskType::Stt, "2026-01-01T00:00:02Z".to_string());
        job.upsert_running_task(TaskType::Summary, "2026-01-01T00:00:03Z".to_string());
        store.insert_job(job).expect("insert status job");

        let app = router_with_repo_root(repo_root);
        let response = app
            .oneshot(get_request("/jobs/job-status/status"))
            .await
            .expect("get job status response");

        assert_eq!(response.status(), StatusCode::OK);
        let body: JobStatusResponse = read_json(response).await;
        assert_eq!(body.job_id, "job-status");
        assert_eq!(body.job_status, JobStatus::Running);
        assert_eq!(body.tasks.len(), 3);
        assert!(
            body.tasks
                .iter()
                .any(|task| task.task_type == TaskType::Ffmpeg)
        );
        assert!(
            body.tasks
                .iter()
                .any(|task| task.task_type == TaskType::Stt)
        );
        assert!(
            body.tasks
                .iter()
                .any(|task| task.task_type == TaskType::Summary)
        );
        assert_eq!(body.error_message, None);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_job_status_returns_404_for_missing_job() {
        let repo_root = temp_workspace();
        let app = router_with_repo_root(repo_root);

        let response = app
            .oneshot(get_request("/jobs/missing/status"))
            .await
            .expect("missing status response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body: ErrorResponse = read_json(response).await;
        assert_eq!(body.code, "404");
        assert!(body.message.contains("job not found: missing"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn post_jobs_marks_failed_job_and_preserves_error_message() {
        let repo_root = temp_workspace();
        let scripts_dir = repo_root.join("scripts");
        let build_bin = repo_root
            .join(".build/ffmpeg")
            .join(crate::ffmpeg::target_dir_name())
            .join("install/bin");
        let input = repo_root.join("fixture.wav");

        fs::create_dir_all(&scripts_dir).expect("scripts dir");
        fs::create_dir_all(&build_bin).expect("toolchain dir");
        write_build_script(&build_script_path(&repo_root, "ffmpeg"));
        write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
        write_failing_ffmpeg(&fake_command_path(&build_bin, "ffmpeg"));
        write_test_wav(&input, 2);

        let app = router_with_repo_root(repo_root.clone());
        let submission: JobSubmissionResponse = read_json(
            app.clone()
                .oneshot(post_json_request(
                    "/jobs",
                    &serde_json::json!({ "input_path": input }),
                ))
                .await
                .expect("submit response"),
        )
        .await;

        let failed = wait_for_job_terminal_state(&app, &submission.job_id).await;
        assert_eq!(failed.status, JobStatus::Failed);
        assert!(
            failed
                .error_message
                .as_deref()
                .expect("error message")
                .contains("ffmpeg conversion failed")
        );

        let job_dir = PathBuf::from(&failed.job_dir);
        assert!(!job_dir.join("channel_01.wav").exists());
        assert!(!job_dir.join("channel_02.wav").exists());
        assert!(!job_dir.join("mono_mix.wav").exists());
    }

    fn temp_workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!("recordroute-server-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).expect("temp workspace");
        path
    }

    fn build_script_path(repo_root: &Path, tool: &str) -> PathBuf {
        crate::ffmpeg::build_script_path(repo_root, tool)
    }

    fn fake_command_path(base_dir: &Path, name: &str) -> PathBuf {
        crate::ffmpeg::fake_command_path(base_dir, name)
    }

    fn write_build_script(path: &Path) {
        write_platform_script(path, "#!/bin/sh\nexit 0\n", "@echo off\nexit /b 0\n");
    }

    fn write_fake_ffprobe(path: &Path, channels: u32, channel_layout: Option<&str>) {
        let json = match channel_layout {
            Some(layout) => format!(
                "{{\"streams\":[{{\"channels\":{channels},\"channel_layout\":\"{layout}\"}}]}}"
            ),
            None => format!("{{\"streams\":[{{\"channels\":{channels}}}]}}"),
        };
        let unix_script = format!(
            "#!/bin/sh\nprintf '%s' '{}'\n",
            json.replace('\'', "'\"'\"'")
        );
        let windows_script = format!("@echo off\necho {json}\n");
        write_platform_script(path, &unix_script, &windows_script);
    }

    fn write_fake_ffmpeg(path: &Path) {
        let unix_script = "#!/bin/sh\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    *.wav)\n      mkdir -p \"$(dirname \"$arg\")\"\n      : > \"$arg\"\n      ;;\n  esac\ndone\n";
        let windows_script = "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\nfor %%I in (\"!arg!\") do (\n  if /I \"%%~xI\"==\".wav\" (\n    if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n    > \"%%~fI\" type nul\n  )\n)\nshift\ngoto loop\n:done\nexit /b 0\n";
        write_platform_script(path, unix_script, windows_script);
    }

    fn write_blocking_ffmpeg(path: &Path, gate_path: &Path) {
        let gate = gate_path.display();
        let unix_script = format!(
            "#!/bin/sh\nwhile [ -f '{gate}' ]; do\n  sleep 0.05\ndone\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    *.wav)\n      mkdir -p \"$(dirname \"$arg\")\"\n      : > \"$arg\"\n      ;;\n  esac\ndone\n"
        );
        let windows_script = format!(
            "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\n:wait\nif exist \"{gate}\" (\n  powershell -NoProfile -Command \"Start-Sleep -Milliseconds 50\" >nul 2>&1\n  goto wait\n)\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\nfor %%I in (\"!arg!\") do (\n  if /I \"%%~xI\"==\".wav\" (\n    if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n    > \"%%~fI\" type nul\n  )\n)\nshift\ngoto loop\n:done\nexit /b 0\n"
        );
        write_platform_script(path, &unix_script, &windows_script);
    }

    fn write_failing_ffmpeg(path: &Path) {
        let unix_script = "#!/bin/sh\nlast=''\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    *.wav)\n      last=\"$arg\"\n      ;;\n  esac\ndone\nif [ -n \"$last\" ]; then\n  mkdir -p \"$(dirname \"$last\")\"\n  : > \"$last\"\nfi\nprintf 'synthetic ffmpeg failure' >&2\nexit 1\n";
        let windows_script = "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nset \"last=\"\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\nfor %%I in (\"!arg!\") do if /I \"%%~xI\"==\".wav\" set \"last=%%~fI\"\nshift\ngoto loop\n:done\nif defined last (\n  for %%I in (\"!last!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n  > \"!last!\" type nul\n)\necho synthetic ffmpeg failure 1>&2\nexit /b 1\n";
        write_platform_script(path, unix_script, windows_script);
    }

    fn write_test_wav(path: &Path, channels: u16) {
        let mut file = File::create(path).expect("fixture wav");
        let sample_rate: u32 = 16_000;
        let bits_per_sample: u16 = 16;
        let samples_per_channel: u32 = 16;
        let bytes_per_sample = u32::from(bits_per_sample / 8);
        let data_size = samples_per_channel * u32::from(channels) * bytes_per_sample;
        let byte_rate = sample_rate * u32::from(channels) * bytes_per_sample;
        let block_align = channels * (bits_per_sample / 8);

        use std::io::Write as _;
        file.write_all(b"RIFF").expect("riff");
        file.write_all(&(36 + data_size).to_le_bytes())
            .expect("chunk size");
        file.write_all(b"WAVE").expect("wave");
        file.write_all(b"fmt ").expect("fmt");
        file.write_all(&16u32.to_le_bytes())
            .expect("fmt chunk size");
        file.write_all(&1u16.to_le_bytes()).expect("pcm");
        file.write_all(&channels.to_le_bytes()).expect("channels");
        file.write_all(&sample_rate.to_le_bytes()).expect("rate");
        file.write_all(&byte_rate.to_le_bytes()).expect("byte rate");
        file.write_all(&block_align.to_le_bytes())
            .expect("block align");
        file.write_all(&bits_per_sample.to_le_bytes())
            .expect("bits");
        file.write_all(b"data").expect("data");
        file.write_all(&data_size.to_le_bytes()).expect("data size");
        file.write_all(&vec![0u8; data_size as usize])
            .expect("samples");
    }

    fn write_platform_script(path: &Path, unix_content: &str, windows_content: &str) {
        let content = if cfg!(windows) {
            windows_content.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            unix_content.to_string()
        };
        fs::write(path, content).expect("script");
        make_executable(path);
    }

    fn make_executable(path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("permissions");
        }
    }

    fn post_json_request(uri: &str, body: &serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("request")
    }

    fn get_request(uri: &str) -> Request<Body> {
        Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .expect("request")
    }

    async fn read_json<T>(response: Response) -> T
    where
        T: for<'de> Deserialize<'de>,
    {
        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect")
            .to_bytes();

        serde_json::from_slice(&body).unwrap_or_else(|error| {
            panic!(
                "failed to parse JSON for status {}: {}",
                status,
                String::from_utf8_lossy(&body)
                    .into_owned()
                    .replace('\n', "\\n")
                    + &format!(" ({error})")
            )
        })
    }

    async fn wait_for_job_completion(app: &Router, job_id: &str) -> JobRecord {
        let job = wait_for_job_terminal_state(app, job_id).await;
        assert_eq!(job.status, JobStatus::Completed);
        job
    }

    async fn wait_for_job_terminal_state(app: &Router, job_id: &str) -> JobRecord {
        let deadline = Instant::now() + Duration::from_secs(5);

        loop {
            let response = app
                .clone()
                .oneshot(get_request(&format!("/jobs/{job_id}")))
                .await
                .expect("job status response");
            let job: JobRecord = read_json(response).await;

            if job.status != JobStatus::Running {
                return job;
            }

            assert!(
                Instant::now() < deadline,
                "timed out waiting for job {job_id}"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}
