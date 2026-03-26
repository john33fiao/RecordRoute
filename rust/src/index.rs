#[path = "index/lock.rs"]
mod lock;
#[path = "index/store.rs"]
mod store;
#[path = "index/types.rs"]
mod types;

pub use store::IndexStore;
pub use types::{
    JobOutputs, JobProbe, JobRecord, JobSplitOutput, JobStatus, ModelKind, ModelPreparationRecord,
    ModelPreparationStatus, TaskRecord, TaskStatus, TaskType,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::path::{Path, PathBuf};
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
    fn reads_legacy_index_without_model_preparations() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        store.ensure_db_dir().expect("db dir");
        fs::write(
            repo_root.join("db/index.json"),
            r#"{"version":1,"jobs":[]}"#,
        )
        .expect("legacy index");

        let index = store.read_index().expect("read legacy index");

        assert_eq!(index.version, 1);
        assert_eq!(
            index.model_preparations,
            types::ModelPreparations::default()
        );
        assert!(index.jobs.is_empty());
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
    fn finds_latest_reusable_completed_job() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let source_path = PathBuf::from("/tmp/input.wav");

        let valid_dir = store.job_dir("job-1");
        fs::create_dir_all(&valid_dir).expect("valid dir");
        let mut valid_job = JobRecord::new(
            "job-1".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            source_path.clone(),
            valid_dir.clone(),
        );
        let valid_outputs = JobOutputs {
            merged_mono_wav: Some(path_to_string(&valid_dir.join("mono_mix.wav"))),
            split_mono_wavs: vec![JobSplitOutput {
                channel_index: 1,
                path: path_to_string(&valid_dir.join("channel_01.wav")),
            }],
        };
        create_file(Path::new(
            valid_outputs
                .merged_mono_wav
                .as_deref()
                .expect("merged path"),
        ));
        create_file(Path::new(&valid_outputs.split_mono_wavs[0].path));
        valid_job
            .mark_completed("2026-01-01T00:00:01Z".to_string(), valid_outputs)
            .expect("mark completed");
        store.insert_job(valid_job).expect("insert valid job");

        let invalid_dir = store.job_dir("job-2");
        fs::create_dir_all(&invalid_dir).expect("invalid dir");
        let mut invalid_job = JobRecord::new(
            "job-2".to_string(),
            "2026-01-01T00:00:02Z".to_string(),
            source_path.clone(),
            invalid_dir.clone(),
        );
        invalid_job
            .mark_completed(
                "2026-01-01T00:00:03Z".to_string(),
                JobOutputs {
                    merged_mono_wav: Some(path_to_string(&invalid_dir.join("mono_mix.wav"))),
                    split_mono_wavs: vec![JobSplitOutput {
                        channel_index: 1,
                        path: path_to_string(&invalid_dir.join("channel_01.wav")),
                    }],
                },
            )
            .expect("mark invalid completed");
        store.insert_job(invalid_job).expect("insert invalid job");

        let mut failed_job = JobRecord::new(
            "job-3".to_string(),
            "2026-01-01T00:00:04Z".to_string(),
            source_path.clone(),
            store.job_dir("job-3"),
        );
        failed_job
            .mark_failed(
                "2026-01-01T00:00:05Z".to_string(),
                "synthetic failure".to_string(),
            )
            .expect("mark failed");
        store.insert_job(failed_job).expect("insert failed job");

        let found = store
            .find_reusable_completed_job(&source_path)
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
    fn stale_lock_is_recovered_before_writing_index() {
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
    fn fresh_lock_times_out() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        store.ensure_db_dir().expect("db dir");
        let now = lock::current_unix_timestamp_secs().expect("timestamp");
        fs::write(
            repo_root.join("db/index.lock"),
            format!("pid=123\ncreated_at_unix_secs={now}\n"),
        )
        .expect("write fresh lock");

        let error = store.list_jobs().expect_err("fresh lock should block");
        assert!(error.contains("timed out waiting for index lock"));
    }

    #[test]
    fn active_lock_handle_times_out_instead_of_access_denied() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        store.ensure_db_dir().expect("db dir");
        let lock_path = repo_root.join("db/index.lock");
        let _guard = super::lock::LockGuard::acquire(&lock_path).expect("hold lock");

        let error = store.list_jobs().expect_err("active lock should block");
        assert!(error.contains("timed out waiting for index lock"));
        assert!(!error.to_ascii_lowercase().contains("denied"));
    }

    #[test]
    fn writes_valid_json_after_atomic_replace() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let job = JobRecord::new(
            "job-1".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/input.wav"),
            store.job_dir("job-1"),
        );

        store.insert_job(job).expect("insert job");

        let content = fs::read_to_string(repo_root.join("db/index.json")).expect("index content");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid json");
        assert_eq!(parsed["jobs"].as_array().map(Vec::len), Some(1));

        let temp_files = fs::read_dir(repo_root.join("db"))
            .expect("db dir entries")
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("tmp"))
            .count();
        assert_eq!(temp_files, 0);
    }

    fn path_to_string(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    fn create_file(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir");
        }
        File::create(path).expect("create file");
    }

    fn temp_workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!("recordroute-index-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).expect("temp workspace");
        path
    }
}
