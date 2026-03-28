use super::app_api;
use super::types::{
    AppState, FileListResponse, SttProgressResponse, SttRequest, SttTranscriptListResponse,
    SummaryRequest, SummarySearchRequest, SummarySearchResponse, SummarySearchResultResponse,
    SummaryTextResponse, build_summary_embedding_status_response,
    build_summary_embedding_submission_response, build_task_status_response,
    build_task_submission_response,
};
use super::{error_response, run_blocking, run_blocking_app};
use crate::index::{IndexStore, TaskType};
use axum::Json;
use axum::body::Body;
use axum::extract::Path as AxumPath;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

fn maybe_wake_queue(state: &AppState, should_execute: bool) {
    if should_execute {
        state.queue_dispatcher.wake();
    }
}

pub(crate) async fn post_stt(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
    payload: Result<Json<SttRequest>, JsonRejection>,
) -> Response {
    let subset = match payload {
        Ok(Json(request)) => match resolve_stt_subset(&request) {
            Ok(subset) => (subset, request.keywords),
            Err(error) => return error_response(crate::error::AppError::bad_request(error)),
        },
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(state.invalid_request_body.clone()),
            )
                .into_response();
        }
    };
    let (subset, keywords) = subset;
    let repo_root = state.repo_root.clone();
    let submit_job_id = job_id.clone();
    let submission = match run_blocking_app(move || {
        app_api::submit_stt_job(&repo_root, &submit_job_id, subset, keywords)
    })
    .await
    {
        Ok(submission) => submission,
        Err(error) => return error_response(error),
    };

    maybe_wake_queue(&state, submission.should_execute());

    (
        StatusCode::ACCEPTED,
        Json(build_task_submission_response(
            job_id,
            TaskType::Stt,
            &submission,
            "stt accepted",
            "stt outputs reused",
            "stt already running",
        )),
    )
        .into_response()
}

pub(crate) fn resolve_stt_subset(request: &SttRequest) -> Result<Option<Vec<String>>, String> {
    if request.mono_mix_only && !request.audio_files.is_empty() {
        return Err("audio_files cannot be combined with mono_mix_only".to_string());
    }
    if request.mono_mix_only {
        return Ok(Some(vec!["mono_mix.wav".to_string()]));
    }
    if request.audio_files.is_empty() {
        return Ok(None);
    }
    Ok(Some(request.audio_files.clone()))
}

pub(crate) async fn get_stt(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let lookup_job_id = job_id.clone();
    let job = match run_blocking(move || IndexStore::new(&repo_root).find_job(&lookup_job_id)).await
    {
        Ok(Some(job)) => job,
        Ok(None) => {
            return error_response(crate::error::AppError::not_found(format!(
                "job not found: {job_id}"
            )));
        }
        Err(error) => return error_response(crate::error::AppError::internal(error)),
    };

    Json(build_task_status_response(
        job_id,
        TaskType::Stt,
        "stt task status",
        job.task(TaskType::Stt).cloned(),
    ))
    .into_response()
}

pub(crate) async fn get_stt_progress(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let lookup_job_id = job_id.clone();
    let progress: SttProgressResponse = match run_blocking_app(move || {
        super::files::read_stt_progress_snapshot(&repo_root, &lookup_job_id)
    })
    .await
    {
        Ok(progress) => progress,
        Err(error) => return error_response(error),
    };

    Json(progress).into_response()
}

pub(crate) async fn get_stt_texts(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let lookup_job_id = job_id.clone();
    let transcripts = match run_blocking_app(move || {
        super::files::read_stt_transcripts(&repo_root, &lookup_job_id)
    })
    .await
    {
        Ok(transcripts) => transcripts,
        Err(error) => return error_response(error),
    };

    Json(SttTranscriptListResponse {
        job_id,
        transcripts,
    })
    .into_response()
}

pub(crate) async fn get_stt_text(
    State(state): State<AppState>,
    AxumPath((job_id, transcript_id)): AxumPath<(String, String)>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let lookup_job_id = job_id.clone();
    let lookup_transcript_id = transcript_id.clone();
    let transcript = match run_blocking_app(move || {
        super::files::read_stt_transcript(&repo_root, &lookup_job_id, &lookup_transcript_id)
    })
    .await
    {
        Ok(transcript) => transcript,
        Err(error) => return error_response(error),
    };
    Json(transcript).into_response()
}

pub(crate) async fn post_summary(
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
    let submission = match run_blocking_app(move || {
        app_api::submit_summary_job(&repo_root, &submit_job_id, force_regenerate)
    })
    .await
    {
        Ok(submission) => submission,
        Err(error) => return error_response(error),
    };

    maybe_wake_queue(&state, submission.should_execute());

    (
        StatusCode::ACCEPTED,
        Json(build_task_submission_response(
            job_id,
            TaskType::Summary,
            &submission,
            "summary accepted",
            "summary reused",
            "summary already running",
        )),
    )
        .into_response()
}

pub(crate) async fn get_summary(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let lookup_job_id = job_id.clone();
    let job = match run_blocking(move || IndexStore::new(&repo_root).find_job(&lookup_job_id)).await
    {
        Ok(Some(job)) => job,
        Ok(None) => {
            return error_response(crate::error::AppError::not_found(format!(
                "job not found: {job_id}"
            )));
        }
        Err(error) => return error_response(crate::error::AppError::internal(error)),
    };

    Json(build_task_status_response(
        job_id,
        TaskType::Summary,
        "summary task status",
        job.task(TaskType::Summary).cloned(),
    ))
    .into_response()
}

pub(crate) async fn get_summary_text(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let lookup_job_id = job_id.clone();
    let summary =
        match run_blocking_app(move || super::files::read_summary_text(&repo_root, &lookup_job_id))
            .await
        {
            Ok(summary) => summary,
            Err(error) => return error_response(error),
        };

    Json(SummaryTextResponse {
        job_id,
        file_name: crate::app::artifacts::summary_file_name().to_string(),
        text: summary,
    })
    .into_response()
}

pub(crate) async fn post_summary_embedding(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let submit_job_id = job_id.clone();
    let submission = match run_blocking_app(move || {
        app_api::submit_summary_embedding_job(&repo_root, &submit_job_id)
    })
    .await
    {
        Ok(submission) => submission,
        Err(error) => return error_response(error),
    };

    maybe_wake_queue(&state, submission.should_execute());

    (
        StatusCode::ACCEPTED,
        Json(build_summary_embedding_submission_response(
            job_id,
            &submission,
        )),
    )
        .into_response()
}

pub(crate) async fn get_summary_embedding(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let lookup_job_id = job_id.clone();
    let job = match run_blocking(move || IndexStore::new(&repo_root).find_job(&lookup_job_id)).await
    {
        Ok(Some(job)) => job,
        Ok(None) => {
            return error_response(crate::error::AppError::not_found(format!(
                "job not found: {job_id}"
            )));
        }
        Err(error) => return error_response(crate::error::AppError::internal(error)),
    };
    Json(build_summary_embedding_status_response(job_id, &job)).into_response()
}

pub(crate) async fn post_summary_search(
    State(state): State<AppState>,
    payload: Result<Json<SummarySearchRequest>, JsonRejection>,
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
    let limit = request.limit.unwrap_or(10).clamp(1, 50);
    let repo_root = state.repo_root.clone();
    let query = request.query.clone();
    let min_score = request.min_score;
    let results = match run_blocking_app(move || {
        app_api::search_summaries(&repo_root, &query, limit, min_score)
    })
    .await
    {
        Ok(results) => results,
        Err(error) => return error_response(error),
    };
    Json(SummarySearchResponse {
        query: request.query,
        results: results
            .into_iter()
            .map(|row| SummarySearchResultResponse {
                job_id: row.job_id,
                score: row.score,
                source_file_name: row.source_file_name,
                summary_file_name: row.summary_file_name,
                summary_excerpt: row.summary_excerpt,
            })
            .collect(),
    })
    .into_response()
}

pub(crate) async fn get_job_files(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let lookup_job_id = job_id.clone();
    let files =
        match run_blocking_app(move || super::files::collect_job_files(&repo_root, &lookup_job_id))
            .await
        {
            Ok(files) => files,
            Err(error) => return error_response(error),
        };
    Json(FileListResponse { job_id, files }).into_response()
}

pub(crate) async fn get_job_file(
    State(state): State<AppState>,
    AxumPath((job_id, file_name)): AxumPath<(String, String)>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let lookup_job_id = job_id.clone();
    let lookup_file = file_name.clone();
    let result = match run_blocking_app(move || {
        super::files::read_job_file(&repo_root, &lookup_job_id, &lookup_file)
    })
    .await
    {
        Ok(result) => result,
        Err(error) => return error_response(error),
    };
    (StatusCode::OK, Body::from(result)).into_response()
}
