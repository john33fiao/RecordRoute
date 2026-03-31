use super::backend::MetadataBackend;
use super::postgres::PostgresMetadataStore;
use super::sqlite::SqliteMetadataStore;
use super::types::{
    AudioArtifactRecord, DictionaryKeywordSource, DictionaryKeywords, IndexFile, JobProbe,
    JobRecord, JobResetSelection, JobStatus, ModelKind, ModelPreparationRecord, ModelPreparations,
    SummaryEmbeddingVectorRecord, SummaryRecord, TaskQueueState, TaskType, TranscriptRecord,
};
use crate::audio_store::AudioStore;
use crate::storage::{MetadataDriver, StorageConfig};
use std::path::{Path, PathBuf};

pub struct IndexStore {
    repo_root: PathBuf,
    backend: Option<Box<dyn MetadataBackend>>,
    audio_store: Option<AudioStore>,
    init_error: Option<String>,
}

impl IndexStore {
    pub fn new(repo_root: &Path) -> Self {
        match StorageConfig::load(repo_root) {
            Ok(config) => {
                let audio_store = AudioStore::new(repo_root).ok();
                let backend: Box<dyn MetadataBackend> = match config.metadata.driver {
                    MetadataDriver::Sqlite => {
                        Box::new(SqliteMetadataStore::new(config.metadata.sqlite_path))
                    }
                    MetadataDriver::Postgres => Box::new(PostgresMetadataStore::new(
                        config.metadata.postgres_url.unwrap_or_default(),
                    )),
                };
                Self {
                    repo_root: repo_root.to_path_buf(),
                    backend: Some(backend),
                    audio_store,
                    init_error: None,
                }
            }
            Err(error) => Self {
                repo_root: repo_root.to_path_buf(),
                backend: None,
                audio_store: None,
                init_error: Some(error),
            },
        }
    }

    pub fn ensure_db_dir(&self) -> Result<(), String> {
        self.backend()?.ensure_initialized()?;
        self.audio_store()?.ensure_dirs()
    }

    pub fn job_dir(&self, job_id: &str) -> PathBuf {
        self.audio_store
            .as_ref()
            .map(|store| store.job_dir(job_id))
            .unwrap_or_else(|| self.repo_root.join("db/audio/jobs").join(job_id))
    }

    pub fn source_path(&self, source_ref: &str) -> PathBuf {
        self.audio_store
            .as_ref()
            .map(|store| store.resolve_storage_path(source_ref))
            .unwrap_or_else(|| self.repo_root.join("db/audio").join(source_ref))
    }

    #[cfg(test)]
    pub fn insert_job(&self, record: JobRecord) -> Result<(), String> {
        self.with_index_mut(|index| {
            index.jobs.push(record);
            Ok(())
        })
    }

    pub fn update_job(
        &self,
        job_id: &str,
        update: impl FnOnce(&mut JobRecord) -> JobRecord,
    ) -> Result<(), String> {
        self.with_index_mut(|index| {
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

    pub fn find_reusable_completed_job_by_source_hash(
        &self,
        source_content_sha256: &str,
    ) -> Result<Option<JobRecord>, String> {
        let index = self.read_index()?;
        for job in index.jobs.iter().rev() {
            if job.status != JobStatus::Completed
                || job.source_content_sha256 != source_content_sha256
                || !self.job_has_reusable_audio(job)?
            {
                continue;
            }
            return Ok(Some(job.clone()));
        }
        Ok(None)
    }

    pub fn find_inflight_job_by_source_hash(
        &self,
        source_content_sha256: &str,
    ) -> Result<Option<JobRecord>, String> {
        let index = self.read_index()?;
        Ok(index
            .jobs
            .iter()
            .rev()
            .find(|job| {
                matches!(job.status, JobStatus::Queued | JobStatus::Running)
                    && job.source_content_sha256 == source_content_sha256
            })
            .cloned())
    }

    #[cfg(test)]
    pub fn find_running_job_by_source(
        &self,
        source_path: &Path,
    ) -> Result<Option<JobRecord>, String> {
        let source = source_path.to_string_lossy().into_owned();
        let index = self.read_index()?;
        Ok(index
            .jobs
            .iter()
            .rev()
            .find(|job| {
                matches!(job.status, JobStatus::Queued | JobStatus::Running)
                    && job.source_ref == source
            })
            .cloned())
    }

    pub fn find_job(&self, job_id: &str) -> Result<Option<JobRecord>, String> {
        self.with_index_read(|index| {
            Ok(index.jobs.iter().find(|job| job.job_id == job_id).cloned())
        })
    }

    pub fn list_jobs(&self) -> Result<Vec<JobRecord>, String> {
        self.with_index_read(|index| Ok(index.jobs.iter().rev().cloned().collect()))
    }

    pub fn list_completed_jobs(&self) -> Result<Vec<JobRecord>, String> {
        self.with_index_read(|index| {
            Ok(index
                .jobs
                .iter()
                .rev()
                .filter(|job| job.status == JobStatus::Completed)
                .cloned()
                .collect())
        })
    }

    pub fn list_jobs_by_source_ref(&self, source_ref: &str) -> Result<Vec<JobRecord>, String> {
        self.with_index_read(|index| {
            Ok(index
                .jobs
                .iter()
                .rev()
                .filter(|job| job.source_ref == source_ref)
                .cloned()
                .collect())
        })
    }

    pub fn model_preparations(&self) -> Result<ModelPreparations, String> {
        self.with_index_read(|index| Ok(index.model_preparations.clone()))
    }

    #[cfg(test)]
    pub fn model_preparation(&self, model: ModelKind) -> Result<ModelPreparationRecord, String> {
        self.with_index_read(|index| Ok(index.model_preparations.record(model).clone()))
    }

    pub fn task_queue(&self) -> Result<TaskQueueState, String> {
        self.with_index_read(|index| Ok(index.task_queue.clone()))
    }

    pub fn update_model_preparation(
        &self,
        model: ModelKind,
        update: impl FnOnce(&mut ModelPreparationRecord),
    ) -> Result<ModelPreparationRecord, String> {
        let mut updated = None;
        self.with_index_mut(|index| {
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
        self.with_index_mut(|index| {
            let record = index.model_preparations.llama_embedding_mut();
            update(record);
            updated = Some(record.clone());
            Ok(())
        })?;
        updated.ok_or_else(|| "failed to update llama embedding preparation".to_string())
    }

    pub fn list_stt_dictionary_keywords(&self) -> Result<DictionaryKeywords, String> {
        self.backend()?.list_stt_dictionary_keywords()
    }

    pub fn upsert_stt_dictionary_keyword(
        &self,
        keyword: &str,
        source: DictionaryKeywordSource,
    ) -> Result<(), String> {
        self.backend()?
            .upsert_stt_dictionary_keyword(keyword, source)
    }

    pub fn delete_stt_dictionary_keyword(
        &self,
        keyword: &str,
        source: DictionaryKeywordSource,
    ) -> Result<bool, String> {
        self.backend()?
            .delete_stt_dictionary_keyword(keyword, source)
    }

    pub fn promote_stt_dictionary_keyword(&self, keyword: &str) -> Result<bool, String> {
        self.backend()?.promote_stt_dictionary_keyword(keyword)
    }

    pub fn list_audio_artifacts(&self, job_id: &str) -> Result<Vec<AudioArtifactRecord>, String> {
        self.backend()?.list_audio_artifacts(job_id)
    }

    pub fn upsert_audio_artifact(&self, record: &AudioArtifactRecord) -> Result<(), String> {
        self.backend()?.upsert_audio_artifact(record)
    }

    pub fn list_transcripts(&self, job_id: &str) -> Result<Vec<TranscriptRecord>, String> {
        self.backend()?.list_transcripts(job_id)
    }

    pub fn find_transcript(
        &self,
        job_id: &str,
        transcript_id: &str,
    ) -> Result<Option<TranscriptRecord>, String> {
        self.backend()?.get_transcript(job_id, transcript_id)
    }

    pub fn upsert_transcript(&self, record: &TranscriptRecord) -> Result<(), String> {
        self.backend()?.upsert_transcript(record)
    }

    pub fn delete_transcript(&self, job_id: &str, transcript_id: &str) -> Result<bool, String> {
        self.backend()?.delete_transcript(job_id, transcript_id)
    }

    pub fn count_transcripts(&self, job_id: &str) -> Result<usize, String> {
        self.backend()?.count_transcripts(job_id)
    }

    pub fn get_summary(&self, job_id: &str) -> Result<Option<SummaryRecord>, String> {
        self.backend()?.get_summary(job_id)
    }

    #[cfg(test)]
    pub fn upsert_summary(&self, record: &SummaryRecord) -> Result<(), String> {
        self.backend()?.upsert_summary(record)
    }

    pub fn commit_summary_success(
        &self,
        job_id: &str,
        finished_at: String,
        record: &SummaryRecord,
        auto_keywords: &[String],
    ) -> Result<JobRecord, String> {
        self.ensure_db_dir()?;
        let mut index = self.read_index()?;
        let updated = {
            let job = index
                .jobs
                .iter_mut()
                .find(|job| job.job_id == job_id)
                .ok_or_else(|| format!("job not found in index: {job_id}"))?;
            job.complete_task(TaskType::Summary, finished_at)?;
            job.clone()
        };
        self.backend()?
            .write_index_with_summary_and_keywords(&index, record, auto_keywords)?;
        Ok(updated)
    }

    pub fn get_summary_embedding(
        &self,
        job_id: &str,
    ) -> Result<Option<SummaryEmbeddingVectorRecord>, String> {
        self.backend()?.get_summary_embedding(job_id)
    }

    pub fn commit_summary_embedding_success(
        &self,
        job_id: &str,
        finished_at: String,
        record: &SummaryEmbeddingVectorRecord,
    ) -> Result<JobRecord, String> {
        self.ensure_db_dir()?;
        let mut index = self.read_index()?;
        let updated = {
            let job = index
                .jobs
                .iter_mut()
                .find(|job| job.job_id == job_id)
                .ok_or_else(|| format!("job not found in index: {job_id}"))?;
            job.complete_task(TaskType::Embedding, finished_at)?;
            job.summary_embedding = Some(record.metadata.clone());
            job.clone()
        };
        self.backend()?
            .write_index_with_summary_embedding(&index, job_id, record)?;
        Ok(updated)
    }

    pub fn reset_job_stages(
        &self,
        job_id: &str,
        selection: JobResetSelection,
        finished_at: String,
        reset_message: &str,
    ) -> Result<JobRecord, String> {
        self.ensure_db_dir()?;
        let selection = selection.normalized();
        let mut index = self.read_index()?;
        let updated = {
            let job = index
                .jobs
                .iter_mut()
                .find(|job| job.job_id == job_id)
                .ok_or_else(|| format!("job not found in index: {job_id}"))?;
            apply_job_reset(job, selection, finished_at, reset_message);
            job.clone()
        };
        let mut normalized = index.clone();
        normalized.task_queue.normalize_burst_limit();
        self.backend()?
            .write_index_with_job_reset(&normalized, job_id, selection)?;
        Ok(updated)
    }

    pub fn with_index_mut<T>(
        &self,
        mutate: impl FnOnce(&mut IndexFile) -> Result<T, String>,
    ) -> Result<T, String> {
        self.ensure_db_dir()?;
        let mut index = self.read_index()?;
        let output = mutate(&mut index)?;
        self.write_index(&index)?;
        Ok(output)
    }

    pub fn with_index_read<T>(
        &self,
        read: impl FnOnce(&IndexFile) -> Result<T, String>,
    ) -> Result<T, String> {
        let index = self.read_index()?;
        read(&index)
    }

    pub(crate) fn read_index(&self) -> Result<IndexFile, String> {
        let mut index = self.backend()?.read_index()?;
        index.task_queue.normalize_burst_limit();
        Ok(index)
    }

    fn write_index(&self, index: &IndexFile) -> Result<(), String> {
        let mut normalized = index.clone();
        normalized.task_queue.normalize_burst_limit();
        self.backend()?.write_index(&normalized)
    }

    fn job_has_reusable_audio(&self, job: &JobRecord) -> Result<bool, String> {
        let Some(merged) = job.outputs.merged_mono_wav.as_deref() else {
            return Ok(false);
        };
        if job.split_strategy.expects_split_outputs() && job.outputs.split_mono_wavs.is_empty() {
            return Ok(false);
        }
        let artifacts = self.list_audio_artifacts(&job.job_id)?;
        let merged_ok = artifacts
            .iter()
            .find(|artifact| artifact.logical_name == merged)
            .is_some_and(|artifact| self.source_path(&artifact.storage_key).is_file());
        if !merged_ok {
            return Ok(false);
        }
        if !job.split_strategy.expects_split_outputs() {
            return Ok(true);
        }
        Ok(job.outputs.split_mono_wavs.iter().all(|output| {
            artifacts
                .iter()
                .find(|artifact| artifact.logical_name == output.path)
                .is_some_and(|artifact| self.source_path(&artifact.storage_key).is_file())
        }))
    }

    fn backend(&self) -> Result<&dyn MetadataBackend, String> {
        if let Some(error) = &self.init_error {
            return Err(error.clone());
        }
        self.backend
            .as_deref()
            .ok_or_else(|| "metadata backend is not initialized".to_string())
    }

    pub fn audio_store(&self) -> Result<&AudioStore, String> {
        if let Some(error) = &self.init_error {
            return Err(error.clone());
        }
        self.audio_store
            .as_ref()
            .ok_or_else(|| "audio store is not initialized".to_string())
    }
}

fn apply_job_reset(
    job: &mut JobRecord,
    selection: JobResetSelection,
    finished_at: String,
    reset_message: &str,
) {
    if selection.ffmpeg {
        let reason = reset_message.to_string();
        let _ = job.mark_failed(finished_at.clone(), reason);
        job.outputs = Default::default();
        job.probe = JobProbe::default();
    }

    if selection.stt {
        mark_task_reset(job, TaskType::Stt, &finished_at, reset_message);
        job.set_task_request_fingerprint(TaskType::Stt, None);
    }

    if selection.summary {
        mark_task_reset(job, TaskType::Summary, &finished_at, reset_message);
    }

    if selection.embedding {
        mark_task_reset(job, TaskType::Embedding, &finished_at, reset_message);
        job.summary_embedding = None;
    }
}

fn mark_task_reset(
    job: &mut JobRecord,
    task_type: TaskType,
    finished_at: &str,
    reset_message: &str,
) {
    let Some(task) = job
        .tasks
        .iter_mut()
        .find(|task| task.task_type == task_type)
    else {
        return;
    };
    task.mark_failed(finished_at.to_string(), reset_message.to_string());
    if task_type == TaskType::Stt {
        task.request_fingerprint = None;
    }
}
