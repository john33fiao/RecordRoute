#[path = "index/backend.rs"]
mod backend;
#[path = "index/postgres.rs"]
mod postgres;
#[path = "index/sqlite.rs"]
mod sqlite;
#[path = "index/store.rs"]
mod store;
#[path = "index/types.rs"]
mod types;

pub use store::IndexStore;
pub use types::{
    ActiveQueueBatch, AudioArtifactRecord, IndexFile, JobOutputs, JobProbe, JobRecord,
    JobSplitOutput, JobStatus, ModelKind, ModelPreparationRecord, ModelPreparationStatus,
    QueueBatch, QueueCategory, QueueEntry, QueuePayload, SourceKind, SummaryEmbeddingRecord,
    SummaryEmbeddingVectorRecord, SummaryRecord, TaskQueueState, TaskRecord, TaskStatus, TaskType,
    TranscriptRecord,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{mark_job_completed_with_audio, test_job};
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn initializes_empty_index_when_missing() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);

        store.ensure_db_dir().expect("db dir");
        let index = store.read_index().expect("read index");

        assert_eq!(index, types::IndexFile::empty());
    }

    #[test]
    fn ignores_legacy_json_when_sqlite_backend_is_used() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        store.ensure_db_dir().expect("db dir");
        fs::write(
            repo_root.join("db/index.json"),
            r#"{"version":1,"jobs":[]}"#,
        )
        .expect("legacy index");

        let index = store.read_index().expect("read index");

        assert_eq!(index.version, 4);
        assert_eq!(
            index.model_preparations,
            types::ModelPreparations::default()
        );
        assert_eq!(index.task_queue, TaskQueueState::default());
        assert!(index.jobs.is_empty());
    }

    #[test]
    fn persists_default_version_and_queue_to_sqlite_backend() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        store.ensure_db_dir().expect("db dir");
        let index = store.read_index().expect("read empty index");
        assert_eq!(index.version, 4);
        assert_eq!(index.task_queue, TaskQueueState::default());

        store
            .insert_job(JobRecord::new(
                "job-1".to_string(),
                "2026-01-01T00:00:00Z".to_string(),
                PathBuf::from("/tmp/input.wav"),
                store.job_dir("job-1"),
            ))
            .expect("insert migrated job");

        let persisted = store.read_index().expect("persisted index");
        assert_eq!(persisted.version, 4);
        assert_eq!(persisted.task_queue.burst_limit, 3);
        assert!(repo_root.join("db/index.sqlite3").is_file());
    }

    #[test]
    fn inserts_and_updates_job_status() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let mut job = JobRecord::new(
            "job-1".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/input.mp3"),
            store.job_dir("job-1"),
        );

        store.insert_job(job.clone()).expect("insert");

        job.probe.channels = Some(2);
        job.mark_failed(
            "2026-01-01T00:00:01Z".to_string(),
            "synthetic failure".to_string(),
        )
        .expect("mark failed");
        store
            .update_job("job-1", |_| job.clone())
            .expect("update failed job");

        let index = store.read_index().expect("read index");
        assert_eq!(index.jobs.len(), 1);
        assert_eq!(index.jobs[0].status, JobStatus::Failed);
        assert_eq!(index.jobs[0].probe.channels, Some(2));
        assert_eq!(
            index.jobs[0].error_message.as_deref(),
            Some("synthetic failure")
        );
    }

    #[test]
    fn finds_latest_reusable_completed_job_by_source_hash() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let mut valid_job = test_job(
            "job-1",
            "2026-01-01T00:00:00Z",
            "sources/shared/source.wav",
            "shared-hash",
            "input.wav",
        );
        mark_job_completed_with_audio(
            &store,
            &mut valid_job,
            "2026-01-01T00:00:01Z",
            &["channel_01.wav", "mono_mix.wav"],
        )
        .expect("mark completed");
        store.insert_job(valid_job).expect("insert valid job");

        let mut invalid_job = test_job(
            "job-2",
            "2026-01-01T00:00:02Z",
            "sources/shared/source.wav",
            "shared-hash",
            "input.wav",
        );
        mark_job_completed_with_audio(
            &store,
            &mut invalid_job,
            "2026-01-01T00:00:03Z",
            &["channel_01.wav", "mono_mix.wav"],
        )
        .expect("mark invalid completed");
        invalid_job
            .job_dir = store.job_dir("job-2").to_string_lossy().into_owned();
        store.insert_job(invalid_job).expect("insert invalid job");
        fs::remove_file(store.job_dir("job-2").join("mono_mix.wav")).expect("remove mono mix");

        let mut failed_job = test_job(
            "job-3",
            "2026-01-01T00:00:04Z",
            "sources/shared/source.wav",
            "shared-hash",
            "input.wav",
        );
        failed_job
            .mark_failed(
                "2026-01-01T00:00:05Z".to_string(),
                "synthetic failure".to_string(),
            )
            .expect("mark failed");
        store.insert_job(failed_job).expect("insert failed job");

        let found = store
            .find_reusable_completed_job_by_source_hash("shared-hash")
            .expect("lookup should succeed")
            .expect("valid job should be found");

        assert_eq!(found.job_id, "job-1");
    }

    #[test]
    fn finds_latest_running_job_by_source() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let source_path = PathBuf::from("/tmp/input.wav");

        let older = JobRecord::new(
            "job-1".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            source_path.clone(),
            store.job_dir("job-1"),
        );
        store.insert_job(older).expect("insert older job");

        let mut failed = JobRecord::new(
            "job-2".to_string(),
            "2026-01-01T00:00:01Z".to_string(),
            source_path.clone(),
            store.job_dir("job-2"),
        );
        failed
            .mark_failed(
                "2026-01-01T00:00:02Z".to_string(),
                "synthetic failure".to_string(),
            )
            .expect("mark failed");
        store.insert_job(failed).expect("insert failed job");

        let newer = JobRecord::new(
            "job-3".to_string(),
            "2026-01-01T00:00:03Z".to_string(),
            source_path.clone(),
            store.job_dir("job-3"),
        );
        store.insert_job(newer).expect("insert newer job");

        let found = store
            .find_running_job_by_source(&source_path)
            .expect("lookup should succeed")
            .expect("running job should be found");

        assert_eq!(found.job_id, "job-3");
    }

    #[test]
    fn lists_jobs_with_latest_first() {
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

        let jobs = store.list_jobs().expect("list jobs");

        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].job_id, "job-2");
        assert_eq!(jobs[1].job_id, "job-1");
    }

    #[test]
    fn updates_model_preparation_records() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let updated = store
            .update_model_preparation(ModelKind::Whisper, |record| {
                record.mark_running("2026-01-01T00:00:00Z".to_string());
            })
            .expect("update model preparation");

        assert_eq!(updated.status, ModelPreparationStatus::Running);
        assert_eq!(
            updated.heartbeat_at.as_deref(),
            Some("2026-01-01T00:00:00Z")
        );

        let persisted = store
            .model_preparation(ModelKind::Whisper)
            .expect("read model preparation");
        assert_eq!(persisted, updated);
        assert_eq!(
            store
                .model_preparation(ModelKind::Llama)
                .expect("llama model preparation"),
            ModelPreparationRecord::default()
        );
    }

    #[test]
    fn complete_task_requires_existing_task() {
        let mut job = JobRecord::new(
            "job-1".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/input.wav"),
            PathBuf::from("/tmp/job-1"),
        );

        let error = job
            .complete_task(TaskType::Summary, "2026-01-01T00:00:02Z".to_string())
            .expect_err("summary task should be missing");

        assert!(error.contains("task not found"));
        assert!(error.contains("summary"));
    }

    #[test]
    fn fail_task_requires_existing_task() {
        let mut job = JobRecord::new(
            "job-1".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/input.wav"),
            PathBuf::from("/tmp/job-1"),
        );

        let error = job
            .fail_task(
                TaskType::Stt,
                "2026-01-01T00:00:03Z".to_string(),
                "synthetic failure".to_string(),
            )
            .expect_err("stt task should be missing");

        assert!(error.contains("task not found"));
        assert!(error.contains("stt"));
    }

    #[test]
    fn ignores_legacy_lock_files_when_writing_sqlite_backend() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        store.ensure_db_dir().expect("db dir");
        fs::write(
            repo_root.join("db/index.lock"),
            "pid=1\ncreated_at_unix_secs=1\n",
        )
        .expect("write stale lock");

        store
            .insert_job(JobRecord::new(
                "job-1".to_string(),
                "2026-01-01T00:00:00Z".to_string(),
                PathBuf::from("/tmp/input.wav"),
                store.job_dir("job-1"),
            ))
            .expect("insert with recovered lock");

        let index = store.read_index().expect("read index");
        assert_eq!(index.jobs.len(), 1);
    }

    #[test]
    fn creates_sqlite_file_after_first_write() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let job = JobRecord::new(
            "job-1".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/input.wav"),
            store.job_dir("job-1"),
        );

        store.insert_job(job).expect("insert job");

        let index = store.read_index().expect("read index");
        assert_eq!(index.jobs.len(), 1);
        assert!(repo_root.join("db/index.sqlite3").is_file());
    }

    fn temp_workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!("recordroute-index-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).expect("temp workspace");
        path
    }
}
