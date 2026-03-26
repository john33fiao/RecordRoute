use super::app_api;
use super::types::{
    AppState, FileListResponse, SttProgressResponse, SttRequest, SttTranscriptListResponse,
    SummaryEmbeddingResponse, SummaryRequest, SummarySearchRequest, SummarySearchResponse,
    SummarySearchResultResponse, SummaryTextResponse, TaskSubmissionResponse,
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

pub(crate) async fn post_stt(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
    payload: Result<Json<SttRequest>, JsonRejection>,
) -> Response {
    let subset = match payload {
        Ok(Json(request)) => match resolve_stt_subset(request) {
            Ok(subset) => subset,
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
    let repo_root = state.repo_root.clone();
    let submit_job_id = job_id.clone();
    let submission =
        match run_blocking_app(move || app_api::submit_stt_job(&repo_root, &submit_job_id, subset))
            .await
        {
            Ok(submission) => submission,
            Err(error) => return error_response(error),
        };

    if submission.should_execute() {
        let repo_root = state.repo_root.clone();
        let execute_job_id = job_id.clone();
        let planned_audio_files = submission.planned_audio_files.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(error) =
                app_api::execute_stt_job(&repo_root, &execute_job_id, &planned_audio_files)
            {
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

pub(crate) fn resolve_stt_subset(request: SttRequest) -> Result<Option<Vec<String>>, String> {
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

    if submission.should_execute() {
        let repo_root = state.repo_root.clone();
        let execute_job_id = job_id.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(error) =
                app_api::execute_summary_job(&repo_root, &execute_job_id, force_regenerate)
            {
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

    if submission.should_execute() {
        let repo_root = state.repo_root.clone();
        let execute_job_id = job_id.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(error) = app_api::execute_summary_embedding_job(&repo_root, &execute_job_id)
            {
                eprintln!("{error}");
            }
        });
    }

    let body = SummaryEmbeddingResponse {
        job_id: job_id.clone(),
        status: "accepted".to_string(),
        message: if submission.reused() {
            "summary embedding reused".to_string()
        } else if submission.deduplicated() {
            "summary embedding already running".to_string()
        } else {
            "summary embedding accepted".to_string()
        },
        reused: submission.reused(),
        deduplicated: submission.deduplicated(),
        task: submission.job.task(TaskType::Embedding).cloned(),
        metadata: submission.job.summary_embedding,
    };
    (StatusCode::ACCEPTED, Json(body)).into_response()
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
    Json(SummaryEmbeddingResponse {
        job_id,
        status: "ok".to_string(),
        message: "summary embedding task status".to_string(),
        reused: false,
        deduplicated: false,
        task: job.task(TaskType::Embedding).cloned(),
        metadata: job.summary_embedding,
    })
    .into_response()
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
