use super::super::router_with_repo_root;
use super::super::types::{JobSubmissionResponse, QueueCancelPendingResponse, QueueStatusResponse};
use super::support::{
    get_request, post_json_request, read_json, temp_workspace, wait_for_job_completion,
    write_build_script, write_fake_ffmpeg, write_fake_ffprobe, write_test_wav,
};
use crate::index::{
    ActiveQueueBatch, IndexStore, JobRecord, QueueBatch, QueueCategory, QueueEntry, QueuePayload,
    TaskQueueState, TaskStatus, TaskType,
};
use crate::test_support::{EnvVarGuard, env_lock, mark_job_completed_with_audio, test_job};
use axum::http::StatusCode;
use std::fs;
use std::path::{Path, PathBuf};
use tower::util::ServiceExt;

#[tokio::test(flavor = "multi_thread")]
async fn get_queue_returns_active_and_pending_batches() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _env_guard = EnvVarGuard::capture(crate::server::QUEUE_START_PAUSED_ENV_VAR);
    unsafe { std::env::set_var(crate::server::QUEUE_START_PAUSED_ENV_VAR, "1") };
    let _burst_guard = EnvVarGuard::capture(crate::index::QUEUE_BURST_LIMIT_ENV_VAR);
    unsafe { std::env::set_var(crate::index::QUEUE_BURST_LIMIT_ENV_VAR, "250") };

    let repo_root = temp_workspace();
    let app = router_with_repo_root(repo_root.clone());

    IndexStore::new(&repo_root)
        .with_index_mut(|index| {
            index.task_queue = TaskQueueState {
                paused: true,
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

    assert!(body.paused);
    assert_eq!(body.burst_limit, 250);

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

#[tokio::test(flavor = "multi_thread")]
async fn get_queue_omits_empty_active_batch_without_running_or_entries() {
    let repo_root = temp_workspace();
    let app = router_with_repo_root(repo_root.clone());

    IndexStore::new(&repo_root)
        .with_index_mut(|index| {
            index.task_queue = TaskQueueState {
                paused: false,
                burst_limit: 100,
                active_batch: Some(ActiveQueueBatch {
                    category: QueueCategory::Embed,
                    running: None,
                    entries: Vec::new(),
                }),
                pending_batches: Vec::new(),
            };
            Ok(())
        })
        .expect("seed empty active batch");

    let response = app
        .oneshot(get_request("/queue"))
        .await
        .expect("queue response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: QueueStatusResponse = read_json(response).await;
    assert!(body.active_batch.is_none());
    assert!(body.pending_batches.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn post_queue_pause_resumes_dispatcher_work() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _env_guard = EnvVarGuard::capture(crate::server::QUEUE_START_PAUSED_ENV_VAR);
    unsafe { std::env::set_var(crate::server::QUEUE_START_PAUSED_ENV_VAR, "1") };

    let repo_root = temp_workspace();
    let scripts_dir = repo_root.join("scripts");
    let build_bin = repo_root
        .join(".build/ffmpeg")
        .join(crate::ffmpeg::target_dir_name())
        .join("install/bin");
    let input = repo_root.join("fixture.wav");

    fs::create_dir_all(&scripts_dir).expect("scripts dir");
    fs::create_dir_all(&build_bin).expect("toolchain dir");
    write_build_script(&crate::ffmpeg::build_script_path(&repo_root, "ffmpeg"));
    write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 1, Some("mono"));
    write_fake_ffmpeg(&fake_command_path(&build_bin, "ffmpeg"));
    write_test_wav(&input, 1);

    let app = router_with_repo_root(repo_root.clone());
    let submit_response = app
        .clone()
        .oneshot(post_json_request(
            "/jobs",
            &serde_json::json!({ "input_path": input }),
        ))
        .await
        .expect("submit response");

    assert_eq!(submit_response.status(), StatusCode::ACCEPTED);
    let submitted: JobSubmissionResponse = read_json(submit_response).await;
    assert_eq!(submitted.status, crate::index::JobStatus::Queued);

    let queue_before: QueueStatusResponse = read_json(
        app.clone()
            .oneshot(get_request("/queue"))
            .await
            .expect("queue before resume"),
    )
    .await;
    assert!(queue_before.paused);

    let response = app
        .clone()
        .oneshot(post_json_request(
            "/queue/pause",
            &serde_json::json!({ "paused": false }),
        ))
        .await
        .expect("resume response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: QueueStatusResponse = read_json(response).await;
    assert!(!body.paused);

    let completed = wait_for_job_completion(&app, &submitted.job_id).await;
    assert_eq!(completed.status, crate::index::JobStatus::Completed);
}

#[tokio::test(flavor = "multi_thread")]
async fn post_queue_cancel_pending_marks_only_waiting_tasks_failed() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _env_guard = EnvVarGuard::capture(crate::server::QUEUE_START_PAUSED_ENV_VAR);
    unsafe { std::env::set_var(crate::server::QUEUE_START_PAUSED_ENV_VAR, "1") };

    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let app = router_with_repo_root(repo_root.clone());

    let mut running_job = JobRecord::new(
        "job-running".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
        PathBuf::from("/tmp/job-running.wav"),
        store.job_dir("job-running"),
    );
    running_job
        .mark_ffmpeg_running("2026-01-01T00:00:01Z".to_string())
        .expect("mark running");
    store.insert_job(running_job).expect("insert running job");

    let queued_ffmpeg = JobRecord::new(
        "job-queued".to_string(),
        "2026-01-01T00:00:02Z".to_string(),
        PathBuf::from("/tmp/job-queued.wav"),
        store.job_dir("job-queued"),
    );
    store
        .insert_job(queued_ffmpeg)
        .expect("insert queued ffmpeg");

    let mut stt_job = test_job(
        "job-stt",
        "2026-01-01T00:00:03Z",
        "sources/job-stt/source.wav",
        "hash-job-stt",
        "job-stt.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut stt_job,
        "2026-01-01T00:00:04Z",
        &["mono_mix.wav"],
    )
    .expect("mark stt job completed");
    stt_job.enqueue_task(TaskType::Stt, "2026-01-01T00:00:05Z".to_string());
    store.insert_job(stt_job).expect("insert stt job");

    let mut summary_job = test_job(
        "job-summary",
        "2026-01-01T00:00:06Z",
        "sources/job-summary/source.wav",
        "hash-job-summary",
        "job-summary.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut summary_job,
        "2026-01-01T00:00:07Z",
        &["mono_mix.wav"],
    )
    .expect("mark summary job completed");
    summary_job.enqueue_task(TaskType::Summary, "2026-01-01T00:00:08Z".to_string());
    store.insert_job(summary_job).expect("insert summary job");

    let mut embedding_job = test_job(
        "job-embedding",
        "2026-01-01T00:00:09Z",
        "sources/job-embedding/source.wav",
        "hash-job-embedding",
        "job-embedding.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut embedding_job,
        "2026-01-01T00:00:10Z",
        &["mono_mix.wav"],
    )
    .expect("mark embedding job completed");
    embedding_job.enqueue_task(TaskType::Embedding, "2026-01-01T00:00:11Z".to_string());
    store
        .insert_job(embedding_job)
        .expect("insert embedding job");

    store
        .with_index_mut(|index| {
            index.task_queue = TaskQueueState {
                paused: true,
                active_batch: Some(ActiveQueueBatch {
                    category: QueueCategory::Ffmpeg,
                    running: Some(ffmpeg_entry(
                        "job-running",
                        Path::new("/tmp/job-running.wav"),
                        "2026-01-01T00:00:01Z".to_string(),
                    )),
                    entries: vec![ffmpeg_entry(
                        "job-queued",
                        Path::new("/tmp/job-queued.wav"),
                        "2026-01-01T00:00:02Z".to_string(),
                    )],
                }),
                pending_batches: vec![
                    QueueBatch {
                        category: QueueCategory::Stt,
                        entries: vec![stt_entry(
                            "job-stt",
                            &[PathBuf::from("mono_mix.wav")],
                            "2026-01-01T00:00:05Z".to_string(),
                        )],
                    },
                    QueueBatch {
                        category: QueueCategory::Llm,
                        entries: vec![summary_entry(
                            "job-summary",
                            false,
                            "2026-01-01T00:00:08Z".to_string(),
                        )],
                    },
                    QueueBatch {
                        category: QueueCategory::Embed,
                        entries: vec![embedding_entry(
                            "job-embedding",
                            "2026-01-01T00:00:11Z".to_string(),
                        )],
                    },
                ],
                ..TaskQueueState::default()
            };
            Ok(())
        })
        .expect("seed queue");

    let response = app
        .clone()
        .oneshot(super::support::post_empty_request("/queue/cancel-pending"))
        .await
        .expect("cancel response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: QueueCancelPendingResponse = read_json(response).await;
    assert_eq!(body.total_cancelled, 4);
    assert_eq!(body.ffmpeg_cancelled, 1);
    assert_eq!(body.stt_cancelled, 1);
    assert_eq!(body.summary_cancelled, 1);
    assert_eq!(body.embedding_cancelled, 1);

    let queue_after: QueueStatusResponse = read_json(
        app.clone()
            .oneshot(get_request("/queue"))
            .await
            .expect("queue after cancel"),
    )
    .await;
    assert!(queue_after.paused);
    assert_eq!(
        queue_after
            .active_batch
            .as_ref()
            .and_then(|batch| batch.running.as_ref())
            .map(|entry| entry.job_id.as_str()),
        Some("job-running")
    );
    assert!(
        queue_after
            .active_batch
            .as_ref()
            .is_some_and(|batch| batch.entries.is_empty())
    );
    assert!(queue_after.pending_batches.is_empty());

    let queued_job = store
        .find_job("job-queued")
        .expect("queued lookup")
        .expect("queued job");
    assert_eq!(queued_job.status, crate::index::JobStatus::Failed);
    assert_eq!(
        queued_job.error_message.as_deref(),
        Some("cancelled by user")
    );

    let stt_job = store
        .find_job("job-stt")
        .expect("stt lookup")
        .expect("stt job");
    assert_eq!(
        stt_job.task(TaskType::Stt).map(|task| task.status),
        Some(TaskStatus::Failed)
    );
    assert_eq!(
        stt_job
            .task(TaskType::Stt)
            .and_then(|task| task.last_error.as_deref()),
        Some("cancelled by user")
    );

    let summary_job = store
        .find_job("job-summary")
        .expect("summary lookup")
        .expect("summary job");
    assert_eq!(
        summary_job.task(TaskType::Summary).map(|task| task.status),
        Some(TaskStatus::Failed)
    );

    let embedding_job = store
        .find_job("job-embedding")
        .expect("embedding lookup")
        .expect("embedding job");
    assert_eq!(
        embedding_job
            .task(TaskType::Embedding)
            .map(|task| task.status),
        Some(TaskStatus::Failed)
    );

    let running_job = store
        .find_job("job-running")
        .expect("running lookup")
        .expect("running job");
    assert_eq!(running_job.status, crate::index::JobStatus::Running);
    assert_eq!(
        running_job.task(TaskType::Ffmpeg).map(|task| task.status),
        Some(TaskStatus::Running)
    );
}

fn fake_command_path(base_dir: &Path, name: &str) -> PathBuf {
    crate::ffmpeg::fake_command_path(base_dir, name)
}

fn ffmpeg_entry(job_id: &str, input_path: &Path, queued_at: String) -> QueueEntry {
    QueueEntry {
        job_id: job_id.to_string(),
        task_type: TaskType::Ffmpeg,
        category: QueueCategory::Ffmpeg,
        queued_at,
        payload: QueuePayload::Ffmpeg {
            input_path: input_path.to_string_lossy().into_owned(),
        },
    }
}

fn stt_entry(job_id: &str, audio_files: &[PathBuf], queued_at: String) -> QueueEntry {
    QueueEntry {
        job_id: job_id.to_string(),
        task_type: TaskType::Stt,
        category: QueueCategory::Stt,
        queued_at,
        payload: QueuePayload::Stt {
            audio_files: audio_files
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            language: "ko".to_string(),
            keywords: Vec::new(),
        },
    }
}

fn summary_entry(job_id: &str, force_regenerate: bool, queued_at: String) -> QueueEntry {
    QueueEntry {
        job_id: job_id.to_string(),
        task_type: TaskType::Summary,
        category: QueueCategory::Llm,
        queued_at,
        payload: QueuePayload::Summary { force_regenerate },
    }
}

fn embedding_entry(job_id: &str, queued_at: String) -> QueueEntry {
    QueueEntry {
        job_id: job_id.to_string(),
        task_type: TaskType::Embedding,
        category: QueueCategory::Embed,
        queued_at,
        payload: QueuePayload::Embedding,
    }
}
