use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexFile {
    pub version: u32,
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
            version: 1,
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
            started_at,
            finished_at: None,
            source_path: source_path.to_string_lossy().into_owned(),
            source_file_name,
            job_dir: job_dir.to_string_lossy().into_owned(),
            probe: JobProbe::default(),
            split_strategy: SplitStrategy::PerChannelPlusMergedMono,
            outputs: JobOutputs::default(),
            error_message: None,
        }
    }

    pub fn mark_completed(&mut self, finished_at: String, outputs: JobOutputs) {
        self.status = JobStatus::Completed;
        self.finished_at = Some(finished_at);
        self.outputs = outputs;
        self.error_message = None;
    }

    pub fn mark_failed(&mut self, finished_at: String, error_message: String) {
        self.status = JobStatus::Failed;
        self.finished_at = Some(finished_at);
        self.error_message = Some(error_message);
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

    fn temp_workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!("recordroute-index-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).expect("temp workspace");
        path
    }
}
