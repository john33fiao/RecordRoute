use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexFile {
    pub version: u32,
    #[serde(default)]
    pub model_preparations: ModelPreparations,
    pub jobs: Vec<JobRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobRecord {
    pub job_id: String,
    pub status: JobStatus,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub source_path: String,
    pub source_file_name: String,
    pub job_dir: String,
    pub probe: JobProbe,
    pub split_strategy: SplitStrategy,
    pub outputs: JobOutputs,
    pub error_message: Option<String>,
    #[serde(default)]
    pub tasks: Vec<TaskRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Ffmpeg,
    Stt,
    Summary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Whisper,
    Llama,
}

impl ModelKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Whisper => "whisper",
            Self::Llama => "llama",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelPreparationStatus {
    Idle,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelPreparationRecord {
    pub status: ModelPreparationStatus,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub heartbeat_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl Default for ModelPreparationRecord {
    fn default() -> Self {
        Self {
            status: ModelPreparationStatus::Idle,
            started_at: None,
            finished_at: None,
            heartbeat_at: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ModelPreparations {
    #[serde(default)]
    pub whisper: ModelPreparationRecord,
    #[serde(default)]
    pub llama: ModelPreparationRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskRecord {
    #[serde(default = "default_task_id")]
    pub task_id: String,
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub last_error: Option<String>,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SplitStrategy {
    PerChannelPlusMergedMono,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct JobProbe {
    pub channels: Option<u32>,
    pub channel_layout: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct JobOutputs {
    pub merged_mono_wav: Option<String>,
    #[serde(default)]
    pub split_mono_wavs: Vec<JobSplitOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobSplitOutput {
    pub channel_index: u32,
    pub path: String,
}

pub struct IndexStore {
    db_dir: PathBuf,
    index_path: PathBuf,
    lock_path: PathBuf,
}

impl IndexFile {
    fn empty() -> Self {
        Self {
            version: 2,
            model_preparations: ModelPreparations::default(),
            jobs: Vec::new(),
        }
    }
}

impl JobRecord {
    pub fn new(job_id: String, started_at: String, source_path: PathBuf, job_dir: PathBuf) -> Self {
        let source_file_name = source_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| source_path.to_string_lossy().into_owned());

        Self {
            job_id,
            status: JobStatus::Running,
            started_at: started_at.clone(),
            finished_at: None,
            source_path: source_path.to_string_lossy().into_owned(),
            source_file_name,
            job_dir: job_dir.to_string_lossy().into_owned(),
            probe: JobProbe::default(),
            split_strategy: SplitStrategy::PerChannelPlusMergedMono,
            outputs: JobOutputs::default(),
            error_message: None,
            tasks: vec![TaskRecord::new(
                TaskType::Ffmpeg,
                TaskStatus::Running,
                started_at,
            )],
        }
    }

    pub fn mark_completed(&mut self, finished_at: String, outputs: JobOutputs) {
        self.status = JobStatus::Completed;
        self.finished_at = Some(finished_at.clone());
        self.outputs = outputs;
        self.error_message = None;
        self.update_task(
            TaskType::Ffmpeg,
            TaskStatus::Completed,
            Some(finished_at),
            None,
        );
    }

    pub fn mark_failed(&mut self, finished_at: String, error_message: String) {
        self.status = JobStatus::Failed;
        self.finished_at = Some(finished_at.clone());
        self.error_message = Some(error_message);
        self.update_task(
            TaskType::Ffmpeg,
            TaskStatus::Failed,
            Some(finished_at),
            self.error_message.clone(),
        );
    }

    pub fn upsert_running_task(&mut self, task_type: TaskType, started_at: String) {
        let retry_count = self
            .task(task_type.clone())
            .map(|task| task.retry_count.saturating_add(1))
            .unwrap_or(0);
        self.set_task(TaskRecord {
            task_id: build_task_id(),
            task_type,
            status: TaskStatus::Running,
            started_at,
            finished_at: None,
            last_error: None,
            retry_count,
        });
    }

    pub fn complete_task(&mut self, task_type: TaskType, finished_at: String) {
        self.update_task(task_type, TaskStatus::Completed, Some(finished_at), None);
    }

    pub fn fail_task(&mut self, task_type: TaskType, finished_at: String, error: String) {
        self.update_task(
            task_type,
            TaskStatus::Failed,
            Some(finished_at),
            Some(error),
        );
    }

    pub fn task(&self, task_type: TaskType) -> Option<&TaskRecord> {
        self.tasks.iter().find(|task| task.task_type == task_type)
    }

    fn set_task(&mut self, record: TaskRecord) {
        if let Some(existing) = self
            .tasks
            .iter_mut()
            .find(|task| task.task_type == record.task_type)
        {
            *existing = record;
        } else {
            self.tasks.push(record);
        }
    }

    fn update_task(
        &mut self,
        task_type: TaskType,
        status: TaskStatus,
        finished_at: Option<String>,
        last_error: Option<String>,
    ) {
        if let Some(task) = self
            .tasks
            .iter_mut()
            .find(|task| task.task_type == task_type)
        {
            task.status = status;
            task.finished_at = finished_at;
            task.last_error = last_error;
            return;
        }

        self.tasks.push(TaskRecord {
            task_id: build_task_id(),
            task_type,
            status,
            started_at: finished_at.clone().unwrap_or_default(),
            finished_at,
            last_error,
            retry_count: 0,
        });
    }
}

impl TaskRecord {
    pub fn new(task_type: TaskType, status: TaskStatus, started_at: String) -> Self {
        Self {
            task_id: build_task_id(),
            task_type,
            status,
            started_at,
            finished_at: None,
            last_error: None,
            retry_count: 0,
        }
    }
}

impl ModelPreparations {
    #[cfg(test)]
    pub fn record(&self, model: ModelKind) -> &ModelPreparationRecord {
        match model {
            ModelKind::Whisper => &self.whisper,
            ModelKind::Llama => &self.llama,
        }
    }

    fn record_mut(&mut self, model: ModelKind) -> &mut ModelPreparationRecord {
        match model {
            ModelKind::Whisper => &mut self.whisper,
            ModelKind::Llama => &mut self.llama,
        }
    }
}

impl ModelPreparationRecord {
    pub fn mark_running(&mut self, started_at: String) {
        self.status = ModelPreparationStatus::Running;
        self.started_at = Some(started_at.clone());
        self.finished_at = None;
        self.heartbeat_at = Some(started_at);
        self.last_error = None;
    }

    pub fn mark_completed(&mut self, finished_at: String) {
        self.status = ModelPreparationStatus::Completed;
        if self.started_at.is_none() {
            self.started_at = Some(finished_at.clone());
        }
        self.finished_at = Some(finished_at.clone());
        self.heartbeat_at = Some(finished_at);
        self.last_error = None;
    }

    pub fn mark_failed(&mut self, finished_at: String, error: String) {
        self.status = ModelPreparationStatus::Failed;
        if self.started_at.is_none() {
            self.started_at = Some(finished_at.clone());
        }
        self.finished_at = Some(finished_at.clone());
        self.heartbeat_at = Some(finished_at);
        self.last_error = Some(error);
    }

    pub fn touch(&mut self, heartbeat_at: String) {
        if self.started_at.is_none() {
            self.started_at = Some(heartbeat_at.clone());
        }
        self.heartbeat_at = Some(heartbeat_at);
    }
}

fn build_task_id() -> String {
    Uuid::now_v7().to_string()
}

fn default_task_id() -> String {
    String::new()
}

impl JobOutputs {
    fn has_reusable_files(&self) -> bool {
        let Some(merged_mono_wav) = self.merged_mono_wav.as_deref() else {
            return false;
        };
        if self.split_mono_wavs.is_empty() {
            return false;
        }

        Path::new(merged_mono_wav).is_file()
            && self
                .split_mono_wavs
                .iter()
                .all(|output| Path::new(&output.path).is_file())
    }
}

impl IndexStore {
    pub fn new(repo_root: &Path) -> Self {
        let db_dir = repo_root.join("db");
        let index_path = db_dir.join("index.json");
        let lock_path = db_dir.join("index.lock");
        Self {
            db_dir,
            index_path,
            lock_path,
        }
    }

    pub fn ensure_db_dir(&self) -> Result<(), String> {
        fs::create_dir_all(&self.db_dir).map_err(|error| {
            format!(
                "failed to create db directory {}: {error}",
                self.db_dir.display()
            )
        })
    }

    pub fn job_dir(&self, job_id: &str) -> PathBuf {
        self.db_dir.join(job_id)
    }

    pub fn insert_job(&self, record: JobRecord) -> Result<(), String> {
        self.with_locked_index(|index| {
            index.jobs.push(record);
            Ok(())
        })
    }

    pub fn update_job(
        &self,
        job_id: &str,
        update: impl FnOnce(&mut JobRecord) -> JobRecord,
    ) -> Result<(), String> {
        self.with_locked_index(|index| {
            let job = index
                .jobs
                .iter_mut()
                .find(|job| job.job_id == job_id)
                .ok_or_else(|| format!("job not found in index: {job_id}"))?;
            let replacement = update(job);
            *job = replacement;
            Ok(())
        })
    }

    pub fn find_reusable_completed_job(
        &self,
        source_path: &Path,
    ) -> Result<Option<JobRecord>, String> {
        let source_path = source_path.to_string_lossy().into_owned();
        self.with_locked_index_read(|index| {
            Ok(index
                .jobs
                .iter()
                .rev()
                .find(|job| {
                    job.status == JobStatus::Completed
                        && job.source_path == source_path
                        && job.outputs.has_reusable_files()
                })
                .cloned())
        })
    }

    pub fn find_running_job_by_source(
        &self,
        source_path: &Path,
    ) -> Result<Option<JobRecord>, String> {
        let source_path = source_path.to_string_lossy().into_owned();
        self.with_locked_index_read(|index| {
            Ok(index
                .jobs
                .iter()
                .rev()
                .find(|job| job.status == JobStatus::Running && job.source_path == source_path)
                .cloned())
        })
    }

    pub fn find_job(&self, job_id: &str) -> Result<Option<JobRecord>, String> {
        self.with_locked_index_read(|index| {
            Ok(index.jobs.iter().find(|job| job.job_id == job_id).cloned())
        })
    }

    pub fn list_jobs(&self) -> Result<Vec<JobRecord>, String> {
        self.with_locked_index_read(|index| Ok(index.jobs.iter().rev().cloned().collect()))
    }

    pub fn model_preparations(&self) -> Result<ModelPreparations, String> {
        self.with_locked_index_read(|index| Ok(index.model_preparations.clone()))
    }

    #[cfg(test)]
    pub fn model_preparation(&self, model: ModelKind) -> Result<ModelPreparationRecord, String> {
        self.with_locked_index_read(|index| Ok(index.model_preparations.record(model).clone()))
    }

    pub fn update_model_preparation(
        &self,
        model: ModelKind,
        update: impl FnOnce(&mut ModelPreparationRecord),
    ) -> Result<ModelPreparationRecord, String> {
        let mut updated = None;
        self.with_locked_index(|index| {
            let record = index.model_preparations.record_mut(model);
            update(record);
            updated = Some(record.clone());
            Ok(())
        })?;
        updated.ok_or_else(|| format!("failed to update {} model preparation", model.as_str()))
    }

    fn with_locked_index(
        &self,
        mutate: impl FnOnce(&mut IndexFile) -> Result<(), String>,
    ) -> Result<(), String> {
        self.ensure_db_dir()?;
        let _guard = LockGuard::acquire(&self.lock_path)?;
        let mut index = self.read_index()?;
        mutate(&mut index)?;
        self.write_index(&index)
    }

    fn with_locked_index_read<T>(
        &self,
        read: impl FnOnce(&IndexFile) -> Result<T, String>,
    ) -> Result<T, String> {
        if !self.db_dir.exists() {
            return read(&IndexFile::empty());
        }

        let _guard = LockGuard::acquire(&self.lock_path)?;
        let index = self.read_index()?;
        read(&index)
    }

    fn read_index(&self) -> Result<IndexFile, String> {
        if !self.index_path.exists() {
            return Ok(IndexFile::empty());
        }

        let content = fs::read_to_string(&self.index_path)
            .map_err(|error| format!("failed to read {}: {error}", self.index_path.display()))?;

        serde_json::from_str(&content)
            .map_err(|error| format!("failed to parse {}: {error}", self.index_path.display()))
    }

    fn write_index(&self, index: &IndexFile) -> Result<(), String> {
        let content = serde_json::to_string_pretty(index)
            .map_err(|error| format!("failed to serialize index: {error}"))?;
        fs::write(&self.index_path, content)
            .map_err(|error| format!("failed to write {}: {error}", self.index_path.display()))
    }
}

struct LockGuard {
    lock_path: PathBuf,
}

impl LockGuard {
    fn acquire(lock_path: &Path) -> Result<Self, String> {
        let timeout = Duration::from_secs(5);
        let poll_interval = Duration::from_millis(25);
        let started = std::time::Instant::now();

        loop {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(lock_path)
            {
                Ok(_) => {
                    return Ok(Self {
                        lock_path: lock_path.to_path_buf(),
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if started.elapsed() >= timeout {
                        return Err(format!(
                            "timed out waiting for index lock {}",
                            lock_path.display()
                        ));
                    }
                    thread::sleep(poll_interval);
                }
                Err(error) => {
                    return Err(format!(
                        "failed to create index lock {}: {error}",
                        lock_path.display()
                    ));
                }
            }
        }
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn initializes_empty_index_when_missing() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);

        store.ensure_db_dir().expect("db dir");
        let index = store.read_index().expect("read index");

        assert_eq!(index, IndexFile::empty());
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
        assert_eq!(index.model_preparations, ModelPreparations::default());
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
        );
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
        valid_job.mark_completed("2026-01-01T00:00:01Z".to_string(), valid_outputs);
        store.insert_job(valid_job).expect("insert valid job");

        let invalid_dir = store.job_dir("job-2");
        fs::create_dir_all(&invalid_dir).expect("invalid dir");
        let mut invalid_job = JobRecord::new(
            "job-2".to_string(),
            "2026-01-01T00:00:02Z".to_string(),
            source_path.clone(),
            invalid_dir.clone(),
        );
        invalid_job.mark_completed(
            "2026-01-01T00:00:03Z".to_string(),
            JobOutputs {
                merged_mono_wav: Some(path_to_string(&invalid_dir.join("mono_mix.wav"))),
                split_mono_wavs: vec![JobSplitOutput {
                    channel_index: 1,
                    path: path_to_string(&invalid_dir.join("channel_01.wav")),
                }],
            },
        );
        store.insert_job(invalid_job).expect("insert invalid job");

        let mut failed_job = JobRecord::new(
            "job-3".to_string(),
            "2026-01-01T00:00:04Z".to_string(),
            source_path.clone(),
            store.job_dir("job-3"),
        );
        failed_job.mark_failed(
            "2026-01-01T00:00:05Z".to_string(),
            "synthetic failure".to_string(),
        );
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
        failed.mark_failed(
            "2026-01-01T00:00:02Z".to_string(),
            "synthetic failure".to_string(),
        );
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
