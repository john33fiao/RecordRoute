use super::super::{
    BatchProcessTarget, RESET_BY_USER_MESSAGE, reset_job, search_summaries,
    submit_batch_pipeline_jobs,
};
use super::support::*;
use crate::index::{
    ActiveQueueBatch, IndexStore, JobResetSelection, JobStatus, QueueCategory,
    SummaryEmbeddingRecord, SummaryEmbeddingVectorRecord, TaskStatus, TaskType,
};
use crate::llama;
use crate::test_support::{
    env_lock, mark_job_completed_with_audio, seed_summary, seed_transcripts, test_job,
};
use sha2::{Digest, Sha256};
use std::fs;

#[test]
fn reset_job_ffmpeg_only_preserves_downstream_data_and_does_not_requeue_stt() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let summary_text = "회의 요약 본문";
    let job_id = "job-reset-ffmpeg";

    let mut job = test_job(
        job_id,
        "2026-01-01T00:00:00Z",
        "sources/job-reset-ffmpeg/source.wav",
        "hash-job-reset-ffmpeg",
        "reset-ffmpeg.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut job,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav", "channel_01.wav"],
    )
    .expect("mark ffmpeg completed");
    job.enqueue_task(TaskType::Stt, "2026-01-01T00:00:02Z".to_string());
    job.complete_task(TaskType::Stt, "2026-01-01T00:00:03Z".to_string())
        .expect("complete stt");
    job.enqueue_task(TaskType::Summary, "2026-01-01T00:00:04Z".to_string());
    job.complete_task(TaskType::Summary, "2026-01-01T00:00:05Z".to_string())
        .expect("complete summary");
    job.enqueue_task(TaskType::Embedding, "2026-01-01T00:00:06Z".to_string());
    store.insert_job(job).expect("insert job");
    seed_transcripts(
        &store,
        job_id,
        &[("mono_mix", "안녕하세요"), ("channel_01", "반갑습니다")],
    )
    .expect("seed transcripts");
    seed_summary(&store, job_id, summary_text).expect("seed summary");
    commit_embedding(&repo_root, &store, job_id, summary_text);

    let source_ref = store
        .find_job(job_id)
        .expect("find job")
        .expect("job")
        .source_ref;

    let result = reset_job(
        &repo_root,
        job_id,
        JobResetSelection {
            ffmpeg: true,
            ..JobResetSelection::default()
        },
    )
    .expect("reset ffmpeg");

    assert_eq!(
        result.deleted,
        JobResetSelection {
            ffmpeg: true,
            ..JobResetSelection::default()
        }
    );
    assert_eq!(result.job.status, JobStatus::Failed);
    assert_eq!(result.job.source_ref, source_ref);
    assert_eq!(
        result.job.error_message.as_deref(),
        Some(RESET_BY_USER_MESSAGE)
    );
    assert_eq!(result.job.outputs.merged_mono_wav, None);
    assert!(result.job.outputs.split_mono_wavs.is_empty());
    assert_eq!(
        result.job.task(TaskType::Ffmpeg).map(|task| task.status),
        Some(TaskStatus::Failed)
    );
    assert_eq!(
        result
            .job
            .task(TaskType::Ffmpeg)
            .and_then(|task| task.last_error.as_deref()),
        Some(RESET_BY_USER_MESSAGE)
    );
    assert!(!store.job_dir(job_id).exists());
    assert!(
        store
            .list_audio_artifacts(job_id)
            .expect("list audio artifacts")
            .is_empty()
    );
    assert_eq!(
        store.count_transcripts(job_id).expect("count transcripts"),
        2
    );
    assert!(store.get_summary(job_id).expect("read summary").is_some());
    assert!(
        store
            .get_summary_embedding(job_id)
            .expect("read embedding")
            .is_some()
    );

    let batch = submit_batch_pipeline_jobs(&repo_root, BatchProcessTarget::All)
        .expect("submit batch pipeline jobs");
    assert_eq!(batch.ffmpeg_queued, 1);
    assert_eq!(batch.stt_queued, 0);
    assert_eq!(batch.summary_queued, 0);
    assert_eq!(batch.embedding_queued, 0);
}

#[test]
fn reset_job_summary_only_preserves_embedding_but_hides_stale_search_results() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        std::env::remove_var("RECORDROUTE_LLAMA_MODEL");
        std::env::remove_var("RECORDROUTE_LLAMA_EMBEDDING_MODEL");
    }

    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let summary_text = "회의 요약 본문";
    let job_id = "job-reset-summary";

    prepare_fake_llama_toolchain(&repo_root);

    let mut job = test_job(
        job_id,
        "2026-01-01T00:00:00Z",
        "sources/job-reset-summary/source.wav",
        "hash-job-reset-summary",
        "reset-summary.wav",
    );
    mark_job_completed_with_audio(&store, &mut job, "2026-01-01T00:00:01Z", &["mono_mix.wav"])
        .expect("mark ffmpeg completed");
    job.enqueue_task(TaskType::Stt, "2026-01-01T00:00:02Z".to_string());
    job.complete_task(TaskType::Stt, "2026-01-01T00:00:03Z".to_string())
        .expect("complete stt");
    job.enqueue_task(TaskType::Summary, "2026-01-01T00:00:04Z".to_string());
    job.complete_task(TaskType::Summary, "2026-01-01T00:00:05Z".to_string())
        .expect("complete summary");
    job.enqueue_task(TaskType::Embedding, "2026-01-01T00:00:06Z".to_string());
    store.insert_job(job).expect("insert job");
    seed_transcripts(&store, job_id, &[("mono_mix", "안녕하세요")]).expect("seed transcripts");
    seed_summary(&store, job_id, summary_text).expect("seed summary");
    commit_embedding(&repo_root, &store, job_id, summary_text);

    let summary_spool_dir = repo_root.join("db/audio-spool/summary").join(job_id);
    fs::create_dir_all(&summary_spool_dir).expect("create summary spool dir");
    fs::write(summary_spool_dir.join("result.md"), summary_text).expect("write summary spool");

    let before = search_summaries(&repo_root, "검색 질의", 10, None).expect("search before reset");
    assert_eq!(before.len(), 1);

    let result = reset_job(
        &repo_root,
        job_id,
        JobResetSelection {
            summary: true,
            ..JobResetSelection::default()
        },
    )
    .expect("reset summary");

    assert_eq!(
        result.job.task(TaskType::Summary).map(|task| task.status),
        Some(TaskStatus::Failed)
    );
    assert_eq!(
        result
            .job
            .task(TaskType::Summary)
            .and_then(|task| task.last_error.as_deref()),
        Some(RESET_BY_USER_MESSAGE)
    );
    assert!(result.job.summary_embedding.is_some());
    assert!(store.get_summary(job_id).expect("read summary").is_none());
    assert!(
        store
            .get_summary_embedding(job_id)
            .expect("read embedding")
            .is_some()
    );
    assert!(!summary_spool_dir.exists());

    let after = search_summaries(&repo_root, "검색 질의", 10, None).expect("search after reset");
    assert!(after.is_empty());
}

#[test]
fn reset_job_embedding_only_removes_embedding_metadata_and_vector() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let summary_text = "임베딩만 재생성할 요약";
    let job_id = "job-reset-embedding";

    let mut job = test_job(
        job_id,
        "2026-01-01T00:00:00Z",
        "sources/job-reset-embedding/source.wav",
        "hash-job-reset-embedding",
        "reset-embedding.wav",
    );
    mark_job_completed_with_audio(&store, &mut job, "2026-01-01T00:00:01Z", &["mono_mix.wav"])
        .expect("mark ffmpeg completed");
    job.enqueue_task(TaskType::Summary, "2026-01-01T00:00:02Z".to_string());
    job.complete_task(TaskType::Summary, "2026-01-01T00:00:03Z".to_string())
        .expect("complete summary");
    job.enqueue_task(TaskType::Embedding, "2026-01-01T00:00:04Z".to_string());
    store.insert_job(job).expect("insert job");
    seed_summary(&store, job_id, summary_text).expect("seed summary");
    commit_embedding(&repo_root, &store, job_id, summary_text);

    let result = reset_job(
        &repo_root,
        job_id,
        JobResetSelection {
            embedding: true,
            ..JobResetSelection::default()
        },
    )
    .expect("reset embedding");

    assert!(store.get_summary(job_id).expect("read summary").is_some());
    assert!(
        store
            .get_summary_embedding(job_id)
            .expect("read embedding")
            .is_none()
    );
    assert!(result.job.summary_embedding.is_none());
    assert_eq!(
        result.job.task(TaskType::Embedding).map(|task| task.status),
        Some(TaskStatus::Failed)
    );
    assert_eq!(
        result
            .job
            .task(TaskType::Embedding)
            .and_then(|task| task.last_error.as_deref()),
        Some(RESET_BY_USER_MESSAGE)
    );
}

#[test]
fn reset_job_allows_empty_active_batch_without_entries_or_running() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let job_id = "job-reset-empty-active-batch";

    let mut job = test_job(
        job_id,
        "2026-01-01T00:00:00Z",
        "sources/job-reset-empty-active-batch/source.wav",
        "hash-job-reset-empty-active-batch",
        "reset-empty-active-batch.wav",
    );
    mark_job_completed_with_audio(&store, &mut job, "2026-01-01T00:00:01Z", &["mono_mix.wav"])
        .expect("mark ffmpeg completed");
    store.insert_job(job).expect("insert job");

    store
        .with_index_mut(|index| {
            index.task_queue.active_batch = Some(ActiveQueueBatch {
                category: QueueCategory::Embed,
                running: None,
                entries: Vec::new(),
            });
            Ok(())
        })
        .expect("seed empty active batch");

    let result = reset_job(
        &repo_root,
        job_id,
        JobResetSelection {
            ffmpeg: true,
            ..JobResetSelection::default()
        },
    )
    .expect("reset with empty active batch");

    assert_eq!(result.job.status, JobStatus::Failed);
    assert_eq!(
        result.job.task(TaskType::Ffmpeg).map(|task| task.status),
        Some(TaskStatus::Failed)
    );
}

fn prepare_fake_llama_toolchain(repo_root: &std::path::Path) {
    let llama_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&llama_bin).expect("llama bin");
    write_build_script(&build_script_path(repo_root, "llama"));
    write_fake_llama_cli(
        &fake_command_path(&llama_bin, "llama-cli"),
        &repo_root.join("llama.log"),
        &repo_root.join("llama-prompt.txt"),
        false,
    );
    write_fake_llama_embedding(
        &fake_command_path(&llama_bin, "llama-embedding"),
        &repo_root.join("llama-embedding.log"),
    );
}

fn commit_embedding(
    repo_root: &std::path::Path,
    store: &IndexStore,
    job_id: &str,
    summary_text: &str,
) {
    let record = SummaryEmbeddingVectorRecord {
        metadata: SummaryEmbeddingRecord {
            model_id: llama::embedding_model_id(repo_root),
            text_sha256: summary_text_sha256(summary_text),
            dimension: 2,
            normalized: true,
            created_at: "2026-01-01T00:00:07Z".to_string(),
        },
        vector: vec![0.25, 0.75],
    };
    store
        .commit_summary_embedding_success(job_id, "2026-01-01T00:00:08Z".to_string(), &record)
        .expect("commit embedding");
}

fn summary_text_sha256(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}
