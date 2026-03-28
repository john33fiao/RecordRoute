use super::app_api;
use super::types::{
    AppState, CreateJobRequest, JobListResponse, JobStatusResponse,
    build_batch_queue_submission_response, build_job_submission_response,
};
use super::{error_response, run_blocking, run_blocking_app};
use crate::app::FfmpegJobSubmission;
use crate::index::IndexStore;
use axum::Json;
use axum::extract::Multipart;
use axum::extract::Path as AxumPath;
use axum::extract::Query;
use axum::extract::State;
use axum::extract::multipart::MultipartRejection;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::path::PathBuf;

fn maybe_wake_queue(state: &AppState, should_execute: bool) {
    if should_execute {
        state.queue_dispatcher.wake();
    }
}

fn build_job_submission_http_response(submission: FfmpegJobSubmission) -> Response {
    let status = if submission.reused() {
        StatusCode::OK
    } else {
        StatusCode::ACCEPTED
    };
    let response = build_job_submission_response(
        &submission.job,
        submission.reused(),
        submission.deduplicated(),
        submission.queue.clone(),
    );

    (status, Json(response)).into_response()
}

pub(crate) async fn post_jobs(
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
    let submission =
        match run_blocking_app(move || app_api::submit_ffmpeg_job(&repo_root, &input_path)).await {
            Ok(submission) => submission,
            Err(error) => return error_response(error),
        };

    maybe_wake_queue(&state, submission.should_execute());
    build_job_submission_http_response(submission)
}

pub(crate) async fn post_jobs_upload(
    State(state): State<AppState>,
    multipart: Result<Multipart, MultipartRejection>,
) -> Response {
    let upload_path = match super::upload::persist_uploaded_file(
        &state.repo_root,
        state.upload_limits,
        multipart,
    )
    .await
    {
        Ok(upload_path) => upload_path,
        Err(error) => return error_response(error),
    };

    let repo_root = state.repo_root.clone();
    let submission = match run_blocking_app(move || {
        app_api::submit_uploaded_ffmpeg_job(&repo_root, upload_path)
    })
    .await
    {
        Ok(submission) => submission,
        Err(error) => return error_response(error),
    };

    maybe_wake_queue(&state, submission.should_execute());
    build_job_submission_http_response(submission)
}

pub(crate) async fn post_jobs_batch_process(State(state): State<AppState>) -> Response {
    let repo_root = state.repo_root.clone();
    let submission =
        match run_blocking_app(move || app_api::submit_batch_pipeline_jobs(&repo_root)).await {
            Ok(submission) => submission,
            Err(error) => return error_response(error),
        };

    maybe_wake_queue(
        &state,
        submission.ffmpeg_queued > 0
            || submission.stt_queued > 0
            || submission.summary_queued > 0
            || submission.embedding_queued > 0,
    );

    (
        StatusCode::ACCEPTED,
        Json(build_batch_queue_submission_response(&submission)),
    )
        .into_response()
}

pub(crate) async fn get_jobs_with_query(
    State(state): State<AppState>,
    query: Result<Query<super::types::JobsQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let source_ref = match query {
        Ok(Query(query)) => query.source_ref,
        Err(_) => None,
    };
    let repo_root = state.repo_root.clone();
    match run_blocking(move || {
        let store = IndexStore::new(&repo_root);
        if let Some(source_ref) = source_ref.as_deref() {
            store.list_jobs_by_source_ref(source_ref)
        } else {
            store.list_jobs()
        }
    })
    .await
    {
        Ok(jobs) => Json(JobListResponse {
            jobs: super::errors::sanitize_jobs(jobs),
        })
        .into_response(),
        Err(error) => error_response(crate::error::AppError::internal(error)),
    }
}

pub(crate) async fn get_completed_jobs(State(state): State<AppState>) -> Response {
    let repo_root = state.repo_root.clone();
    match run_blocking(move || IndexStore::new(&repo_root).list_completed_jobs()).await {
        Ok(jobs) => Json(JobListResponse {
            jobs: super::errors::sanitize_jobs(jobs),
        })
        .into_response(),
        Err(error) => error_response(crate::error::AppError::internal(error)),
    }
}

pub(crate) async fn get_job(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let job_id_for_lookup = job_id.clone();

    match run_blocking(move || IndexStore::new(&repo_root).find_job(&job_id_for_lookup)).await {
        Ok(Some(job)) => Json(super::errors::sanitize_job(job)).into_response(),
        Ok(None) => error_response(crate::error::AppError::not_found(format!(
            "job not found: {job_id}"
        ))),
        Err(error) => error_response(crate::error::AppError::internal(error)),
    }
}

pub(crate) async fn get_job_status(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let lookup_job_id = job_id.clone();

    match run_blocking(move || IndexStore::new(&repo_root).find_job(&lookup_job_id)).await {
        Ok(Some(job)) => {
            let job = super::errors::sanitize_job(job);
            Json(JobStatusResponse {
                job_id,
                job_status: job.status,
                tasks: job.tasks,
                error_message: job.error_message,
            })
            .into_response()
        }
        Ok(None) => error_response(crate::error::AppError::not_found(format!(
            "job not found: {job_id}"
        ))),
        Err(error) => error_response(crate::error::AppError::internal(error)),
    }
}
