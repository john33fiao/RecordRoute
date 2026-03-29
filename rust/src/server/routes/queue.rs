use super::app_api;
use super::types::{
    AppState, QueueBatchResponse, QueueCancelPendingResponse, QueueEntryResponse,
    QueuePauseRequest, QueueStatusResponse,
};
use super::{error_response, run_blocking, run_blocking_app};
use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub(crate) async fn get_queue(State(state): State<AppState>) -> Response {
    let repo_root = state.repo_root.clone();
    match run_blocking(move || crate::app::queue_snapshot(&repo_root)).await {
        Ok(snapshot) => Json(build_queue_status_response(snapshot)).into_response(),
        Err(error) => error_response(crate::error::AppError::internal(error)),
    }
}

pub(crate) async fn post_queue_pause(
    State(state): State<AppState>,
    payload: Result<Json<QueuePauseRequest>, JsonRejection>,
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
    let paused = request.paused;
    let snapshot =
        match run_blocking_app(move || app_api::set_queue_paused(&repo_root, paused)).await {
            Ok(snapshot) => snapshot,
            Err(error) => return error_response(error),
        };

    if !paused {
        state.queue_dispatcher.wake();
    }

    Json(build_queue_status_response(snapshot)).into_response()
}

pub(crate) async fn post_queue_cancel_pending(State(state): State<AppState>) -> Response {
    let repo_root = state.repo_root.clone();
    match run_blocking_app(move || app_api::cancel_pending_queue_entries(&repo_root)).await {
        Ok(result) => Json(QueueCancelPendingResponse {
            total_cancelled: result.total_cancelled,
            ffmpeg_cancelled: result.ffmpeg_cancelled,
            stt_cancelled: result.stt_cancelled,
            summary_cancelled: result.summary_cancelled,
            embedding_cancelled: result.embedding_cancelled,
        })
        .into_response(),
        Err(error) => error_response(error),
    }
}

fn build_queue_status_response(snapshot: crate::index::TaskQueueState) -> QueueStatusResponse {
    QueueStatusResponse {
        paused: snapshot.paused,
        burst_limit: snapshot.burst_limit,
        active_batch: snapshot.active_batch.map(build_queue_batch_response),
        pending_batches: snapshot
            .pending_batches
            .into_iter()
            .map(|batch| QueueBatchResponse {
                category: batch.category,
                running: None,
                entries: batch
                    .entries
                    .into_iter()
                    .map(build_queue_entry_response)
                    .collect(),
            })
            .collect(),
    }
}

fn build_queue_batch_response(batch: crate::index::ActiveQueueBatch) -> QueueBatchResponse {
    QueueBatchResponse {
        category: batch.category,
        running: batch.running.map(build_queue_entry_response),
        entries: batch
            .entries
            .into_iter()
            .map(build_queue_entry_response)
            .collect(),
    }
}

fn build_queue_entry_response(entry: crate::index::QueueEntry) -> QueueEntryResponse {
    QueueEntryResponse {
        job_id: entry.job_id,
        task_type: entry.task_type,
        category: entry.category,
        queued_at: entry.queued_at,
    }
}
