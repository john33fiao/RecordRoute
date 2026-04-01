use super::super::router_with_repo_root;
use super::support::{get_request, read_json, temp_workspace};
use crate::index::{
    IndexStore, JobRecord, SourceKind, SummaryEmbeddingRecord, SummaryEmbeddingVectorRecord,
    TaskType,
};
use crate::server::types::StatsOverviewResponse;
use crate::test_support::{mark_job_completed_with_audio, seed_summary, seed_transcripts};
use axum::http::StatusCode;
use tower::util::ServiceExt;

fn upload_job(job_id: &str, source_file_name: &str) -> JobRecord {
    JobRecord::new_with_source(
        job_id.to_string(),
        "2026-01-01T00:00:00Z".to_string(),
        format!("sources/{job_id}/{source_file_name}"),
        SourceKind::Upload,
        format!("hash-{job_id}"),
        source_file_name.to_string(),
    )
}

fn local_job(job_id: &str, source_file_name: &str) -> JobRecord {
    JobRecord::new_with_source(
        job_id.to_string(),
        "2026-01-01T00:00:00Z".to_string(),
        format!("sources/{job_id}/{source_file_name}"),
        SourceKind::LocalFile,
        format!("hash-{job_id}"),
        source_file_name.to_string(),
    )
}

fn queue_task(job: &mut JobRecord, task_type: TaskType, queued_at: &str) {
    job.enqueue_task(task_type, queued_at.to_string());
}

fn complete_task(job: &mut JobRecord, task_type: TaskType, queued_at: &str, finished_at: &str) {
    job.enqueue_task(task_type, queued_at.to_string());
    job.complete_task(task_type, finished_at.to_string())
        .expect("complete task");
}

fn running_task(job: &mut JobRecord, task_type: TaskType, started_at: &str) {
    job.upsert_running_task(task_type, started_at.to_string());
}

fn fail_task(
    job: &mut JobRecord,
    task_type: TaskType,
    queued_at: &str,
    finished_at: &str,
    error: &str,
) {
    job.enqueue_task(task_type, queued_at.to_string());
    job.fail_task(task_type, finished_at.to_string(), error.to_string())
        .expect("fail task");
}

fn seed_embedding(store: &IndexStore, job_id: &str, created_at: &str) -> Result<(), String> {
    store.commit_summary_embedding_success(
        job_id,
        created_at.to_string(),
        &SummaryEmbeddingVectorRecord {
            metadata: SummaryEmbeddingRecord {
                model_id: "embedding-model".to_string(),
                text_sha256: format!("summary-sha-{job_id}"),
                dimension: 3,
                normalized: true,
                created_at: created_at.to_string(),
            },
            vector: vec![0.1, 0.2, 0.3],
        },
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn get_stats_overview_returns_zero_counts_for_empty_workspace() {
    let repo_root = temp_workspace();
    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/stats/overview"))
        .await
        .expect("stats overview response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: StatsOverviewResponse = read_json(response).await;
    assert_eq!(body.upload_job_count, 0);
    assert_eq!(body.all_job_count, 0);
    assert_eq!(body.stages.len(), 4);

    for stage in body.stages {
        assert_eq!(stage.completed_count, 0);
        assert_eq!(stage.in_progress_count, 0);
        assert_eq!(stage.unprocessed_count, 0);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn get_stats_overview_separates_upload_totals_and_stage_buckets() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);

    let mut complete_all = upload_job("job-upload-complete-all", "complete.wav");
    mark_job_completed_with_audio(
        &store,
        &mut complete_all,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark complete-all ffmpeg completed");
    complete_task(
        &mut complete_all,
        TaskType::Stt,
        "2026-01-01T00:00:02Z",
        "2026-01-01T00:00:03Z",
    );
    complete_task(
        &mut complete_all,
        TaskType::Summary,
        "2026-01-01T00:00:04Z",
        "2026-01-01T00:00:05Z",
    );
    queue_task(
        &mut complete_all,
        TaskType::Embedding,
        "2026-01-01T00:00:06Z",
    );
    store
        .insert_job(complete_all)
        .expect("insert complete-all job");
    seed_transcripts(
        &store,
        "job-upload-complete-all",
        &[("mono_mix", "transcript text")],
    )
    .expect("seed complete-all transcripts");
    seed_summary(&store, "job-upload-complete-all", "summary text")
        .expect("seed complete-all summary");
    seed_embedding(&store, "job-upload-complete-all", "2026-01-01T00:00:07Z")
        .expect("seed complete-all embedding");

    let mut stt_queued = upload_job("job-upload-stt-queued", "stt-queued.wav");
    mark_job_completed_with_audio(
        &store,
        &mut stt_queued,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark stt queued ffmpeg completed");
    queue_task(&mut stt_queued, TaskType::Stt, "2026-01-01T00:00:02Z");
    store.insert_job(stt_queued).expect("insert stt queued job");

    let mut summary_running = upload_job("job-upload-summary-running", "summary-running.wav");
    mark_job_completed_with_audio(
        &store,
        &mut summary_running,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark summary running ffmpeg completed");
    complete_task(
        &mut summary_running,
        TaskType::Stt,
        "2026-01-01T00:00:02Z",
        "2026-01-01T00:00:03Z",
    );
    running_task(
        &mut summary_running,
        TaskType::Summary,
        "2026-01-01T00:00:04Z",
    );
    store
        .insert_job(summary_running)
        .expect("insert summary running job");
    seed_transcripts(
        &store,
        "job-upload-summary-running",
        &[("mono_mix", "transcript text")],
    )
    .expect("seed summary running transcripts");

    let mut summary_unprocessed =
        upload_job("job-upload-summary-unprocessed", "summary-unprocessed.wav");
    mark_job_completed_with_audio(
        &store,
        &mut summary_unprocessed,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark summary unprocessed ffmpeg completed");
    complete_task(
        &mut summary_unprocessed,
        TaskType::Stt,
        "2026-01-01T00:00:02Z",
        "2026-01-01T00:00:03Z",
    );
    store
        .insert_job(summary_unprocessed)
        .expect("insert summary unprocessed job");
    seed_transcripts(
        &store,
        "job-upload-summary-unprocessed",
        &[("mono_mix", "transcript text")],
    )
    .expect("seed summary unprocessed transcripts");

    let mut summary_completed_without_output = upload_job(
        "job-upload-summary-completed-no-output",
        "summary-completed-no-output.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut summary_completed_without_output,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark summary completed-no-output ffmpeg completed");
    complete_task(
        &mut summary_completed_without_output,
        TaskType::Stt,
        "2026-01-01T00:00:02Z",
        "2026-01-01T00:00:03Z",
    );
    complete_task(
        &mut summary_completed_without_output,
        TaskType::Summary,
        "2026-01-01T00:00:04Z",
        "2026-01-01T00:00:05Z",
    );
    store
        .insert_job(summary_completed_without_output)
        .expect("insert summary completed-no-output job");
    seed_transcripts(
        &store,
        "job-upload-summary-completed-no-output",
        &[("mono_mix", "transcript text")],
    )
    .expect("seed summary completed-no-output transcripts");

    let mut embedding_queued = upload_job("job-upload-embedding-queued", "embedding-queued.wav");
    mark_job_completed_with_audio(
        &store,
        &mut embedding_queued,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark embedding queued ffmpeg completed");
    complete_task(
        &mut embedding_queued,
        TaskType::Stt,
        "2026-01-01T00:00:02Z",
        "2026-01-01T00:00:03Z",
    );
    complete_task(
        &mut embedding_queued,
        TaskType::Summary,
        "2026-01-01T00:00:04Z",
        "2026-01-01T00:00:05Z",
    );
    queue_task(
        &mut embedding_queued,
        TaskType::Embedding,
        "2026-01-01T00:00:06Z",
    );
    store
        .insert_job(embedding_queued)
        .expect("insert embedding queued job");
    seed_transcripts(
        &store,
        "job-upload-embedding-queued",
        &[("mono_mix", "transcript text")],
    )
    .expect("seed embedding queued transcripts");
    seed_summary(&store, "job-upload-embedding-queued", "summary text")
        .expect("seed embedding queued summary");

    let mut embedding_failed = upload_job("job-upload-embedding-failed", "embedding-failed.wav");
    mark_job_completed_with_audio(
        &store,
        &mut embedding_failed,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark embedding failed ffmpeg completed");
    complete_task(
        &mut embedding_failed,
        TaskType::Stt,
        "2026-01-01T00:00:02Z",
        "2026-01-01T00:00:03Z",
    );
    complete_task(
        &mut embedding_failed,
        TaskType::Summary,
        "2026-01-01T00:00:04Z",
        "2026-01-01T00:00:05Z",
    );
    fail_task(
        &mut embedding_failed,
        TaskType::Embedding,
        "2026-01-01T00:00:06Z",
        "2026-01-01T00:00:07Z",
        "synthetic embedding failure",
    );
    store
        .insert_job(embedding_failed)
        .expect("insert embedding failed job");
    seed_transcripts(
        &store,
        "job-upload-embedding-failed",
        &[("mono_mix", "transcript text")],
    )
    .expect("seed embedding failed transcripts");
    seed_summary(&store, "job-upload-embedding-failed", "summary text")
        .expect("seed embedding failed summary");

    let mut summary_failed = upload_job("job-upload-summary-failed", "summary-failed.wav");
    mark_job_completed_with_audio(
        &store,
        &mut summary_failed,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark summary failed ffmpeg completed");
    complete_task(
        &mut summary_failed,
        TaskType::Stt,
        "2026-01-01T00:00:02Z",
        "2026-01-01T00:00:03Z",
    );
    fail_task(
        &mut summary_failed,
        TaskType::Summary,
        "2026-01-01T00:00:04Z",
        "2026-01-01T00:00:05Z",
        "synthetic summary failure",
    );
    store
        .insert_job(summary_failed)
        .expect("insert summary failed job");
    seed_transcripts(
        &store,
        "job-upload-summary-failed",
        &[("mono_mix", "transcript text")],
    )
    .expect("seed summary failed transcripts");

    let mut complete_local = local_job("job-local-complete-all", "local-complete.wav");
    mark_job_completed_with_audio(
        &store,
        &mut complete_local,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark local ffmpeg completed");
    complete_task(
        &mut complete_local,
        TaskType::Stt,
        "2026-01-01T00:00:02Z",
        "2026-01-01T00:00:03Z",
    );
    complete_task(
        &mut complete_local,
        TaskType::Summary,
        "2026-01-01T00:00:04Z",
        "2026-01-01T00:00:05Z",
    );
    queue_task(
        &mut complete_local,
        TaskType::Embedding,
        "2026-01-01T00:00:06Z",
    );
    store
        .insert_job(complete_local)
        .expect("insert local complete job");
    seed_transcripts(
        &store,
        "job-local-complete-all",
        &[("mono_mix", "transcript text")],
    )
    .expect("seed local transcripts");
    seed_summary(&store, "job-local-complete-all", "summary text").expect("seed local summary");
    seed_embedding(&store, "job-local-complete-all", "2026-01-01T00:00:07Z")
        .expect("seed local embedding");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/stats/overview"))
        .await
        .expect("stats overview response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: StatsOverviewResponse = read_json(response).await;

    assert_eq!(body.upload_job_count, 8);
    assert_eq!(body.all_job_count, 9);

    let ffmpeg = body
        .stages
        .iter()
        .find(|stage| stage.stage == TaskType::Ffmpeg)
        .expect("ffmpeg stage");
    assert_eq!(ffmpeg.completed_count, 8);
    assert_eq!(ffmpeg.in_progress_count, 0);
    assert_eq!(ffmpeg.unprocessed_count, 0);

    let stt = body
        .stages
        .iter()
        .find(|stage| stage.stage == TaskType::Stt)
        .expect("stt stage");
    assert_eq!(stt.completed_count, 7);
    assert_eq!(stt.in_progress_count, 1);
    assert_eq!(stt.unprocessed_count, 0);

    let summary = body
        .stages
        .iter()
        .find(|stage| stage.stage == TaskType::Summary)
        .expect("summary stage");
    assert_eq!(summary.completed_count, 3);
    assert_eq!(summary.in_progress_count, 1);
    assert_eq!(summary.unprocessed_count, 3);

    let embedding = body
        .stages
        .iter()
        .find(|stage| stage.stage == TaskType::Embedding)
        .expect("embedding stage");
    assert_eq!(embedding.completed_count, 1);
    assert_eq!(embedding.in_progress_count, 1);
    assert_eq!(embedding.unprocessed_count, 5);
}
