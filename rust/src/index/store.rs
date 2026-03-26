use super::lock::LockGuard;
use super::types::{
    IndexFile, JobRecord, JobStatus, ModelKind, ModelPreparationRecord, ModelPreparations,
};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct IndexStore {
    db_dir: PathBuf,
    index_path: PathBuf,
    lock_path: PathBuf,
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

    pub fn list_completed_jobs(&self) -> Result<Vec<JobRecord>, String> {
        self.with_locked_index_read(|index| {
            Ok(index
                .jobs
                .iter()
                .rev()
                .filter(|job| job.status == JobStatus::Completed)
                .cloned()
                .collect())
        })
    }

    pub fn list_jobs_by_source_path(&self, source_path: &str) -> Result<Vec<JobRecord>, String> {
        self.with_locked_index_read(|index| {
            Ok(index
                .jobs
                .iter()
                .rev()
                .filter(|job| job.source_path == source_path)
                .cloned()
                .collect())
        })
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

    pub fn update_llama_embedding_preparation(
        &self,
        update: impl FnOnce(&mut ModelPreparationRecord),
    ) -> Result<ModelPreparationRecord, String> {
        let mut updated = None;
        self.with_locked_index(|index| {
            let record = index.model_preparations.llama_embedding_mut();
            update(record);
            updated = Some(record.clone());
            Ok(())
        })?;
        updated.ok_or_else(|| "failed to update llama embedding preparation".to_string())
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

    pub(crate) fn read_index(&self) -> Result<IndexFile, String> {
        if !self.index_path.exists() {
            return Ok(IndexFile::empty());
        }

        let content = fs::read_to_string(&self.index_path)
            .map_err(|error| format!("failed to read {}: {error}", self.index_path.display()))?;

        serde_json::from_str(&content)
            .map_err(|error| format!("failed to parse {}: {error}", self.index_path.display()))
    }

    fn write_index(&self, index: &IndexFile) -> Result<(), String> {
        let content = serde_json::to_vec_pretty(index)
            .map_err(|error| format!("failed to serialize index: {error}"))?;
        let temp_path = self.db_dir.join(format!("index.{}.tmp", Uuid::now_v7()));
        let mut file = File::create(&temp_path)
            .map_err(|error| format!("failed to create {}: {error}", temp_path.display()))?;
        file.write_all(&content)
            .map_err(|error| format!("failed to write {}: {error}", temp_path.display()))?;
        file.sync_all()
            .map_err(|error| format!("failed to sync {}: {error}", temp_path.display()))?;
        drop(file);
        replace_index_file(&temp_path, &self.index_path)
    }
}

#[cfg(not(windows))]
fn replace_index_file(temp_path: &Path, index_path: &Path) -> Result<(), String> {
    fs::rename(temp_path, index_path).map_err(|error| {
        format!(
            "failed to replace {} with {}: {error}",
            index_path.display(),
            temp_path.display()
        )
    })
}

#[cfg(windows)]
fn replace_index_file(temp_path: &Path, index_path: &Path) -> Result<(), String> {
    if index_path.exists() {
        fs::remove_file(index_path).map_err(|error| {
            format!(
                "failed to remove previous index {} before replace: {error}",
                index_path.display()
            )
        })?;
    }

    fs::rename(temp_path, index_path).map_err(|error| {
        format!(
            "failed to install {} from {}: {error}",
            index_path.display(),
            temp_path.display()
        )
    })
}
