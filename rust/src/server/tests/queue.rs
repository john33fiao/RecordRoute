use super::super::router_with_repo_root;
use super::super::types::QueueStatusResponse;
use super::support::{get_request, read_json, temp_workspace};
use crate::index::{
    ActiveQueueBatch, IndexStore, QueueBatch, QueueCategory, QueueEntry, QueuePayload,
    TaskQueueState, TaskType,
};
use axum::http::StatusCode;
use tower::util::ServiceExt;

#[tokio::test(flavor = "multi_thread")]
async fn get_queue_returns_active_and_pending_batches() {
    let _guard = crate::test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let repo_root = temp_workspace();
    let app = router_with_repo_root(repo_root.clone());

    IndexStore::new(&repo_root)
        .with_index_mut(|index| {
            index.task_queue = TaskQueueState {
                burst_limit: 5,
                active_batch: Some(ActiveQueueBatch {
                    category: QueueCategory::Stt,
                    running: Some(QueueEntry {
                        job_id: "job-running".to_string(),
                        task_type: TaskType::Stt,
                        category: QueueCategory::Stt,
                        queued_at: "2026-01-01T00:00:01Z".to_string(),
                        payload: QueuePayload::Stt {
                            audio_files: vec!["mono_mix.wav".to_string()],
                            language: "ko".to_string(),
                            keywords: Vec::new(),
                        },
                    }),
                    entries: vec![QueueEntry {
                        job_id: "job-waiting".to_string(),
                        task_type: TaskType::Stt,
                        category: QueueCategory::Stt,
                        queued_at: "2026-01-01T00:00:02Z".to_string(),
                        payload: QueuePayload::Stt {
                            audio_files: vec!["channel_01.wav".to_string()],
                            language: "ko".to_string(),
                            keywords: Vec::new(),
                        },
                    }],
                }),
                pending_batches: vec![QueueBatch {
                    category: QueueCategory::Llm,
                    entries: vec![QueueEntry {
                        job_id: "job-summary".to_string(),
                        task_type: TaskType::Summary,
                        category: QueueCategory::Llm,
                        queued_at: "2026-01-01T00:00:03Z".to_string(),
                        payload: QueuePayload::Summary {
                            force_regenerate: false,
                        },
                    }],
                }],
            };
            Ok(())
        })
        .expect("seed queue state");

    let response = app
        .oneshot(get_request("/queue"))
        .await
        .expect("queue response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: QueueStatusResponse = read_json(response).await;

    assert_eq!(body.burst_limit, 5);

    let active = body.active_batch.expect("active batch");
    assert_eq!(active.category, QueueCategory::Stt);
    assert_eq!(
        active.running.as_ref().map(|entry| entry.job_id.as_str()),
        Some("job-running")
    );
    assert_eq!(active.entries.len(), 1);
    assert_eq!(active.entries[0].job_id, "job-waiting");

    assert_eq!(body.pending_batches.len(), 1);
    assert_eq!(body.pending_batches[0].category, QueueCategory::Llm);
    assert_eq!(body.pending_batches[0].entries.len(), 1);
    assert_eq!(body.pending_batches[0].entries[0].job_id, "job-summary");
}
