use super::types::{AppState, QueueBatchResponse, QueueEntryResponse, QueueStatusResponse};
use super::{error_response, run_blocking};
use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};

pub(crate) async fn get_queue(State(state): State<AppState>) -> Response {
    let repo_root = state.repo_root.clone();
    match run_blocking(move || crate::app::queue_snapshot(&repo_root)).await {
        Ok(snapshot) => Json(QueueStatusResponse {
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
        })
        .into_response(),
        Err(error) => error_response(crate::error::AppError::internal(error)),
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
