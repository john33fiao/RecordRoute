use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use axum::extract::multipart::Field;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Deserialize;
use serde_json::Value;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::task;
use uuid::Uuid;

use crate::models::{
    Job, NewRecording, ProcessingStatus, ProcessingStep, Recording, RecordingBundle,
    RecordingListItem, SearchResult, StructuredSummary,
};

const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const SQLITE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS recordings (
    id TEXT PRIMARY KEY,
    original_filename TEXT NOT NULL,
    original_content_type TEXT,
    file_size_bytes INTEGER NOT NULL,
    original_rel_path TEXT NOT NULL,
    wav_rel_path TEXT,
    language TEXT,
    transcript TEXT,
    summary TEXT,
    summary_canonical_text TEXT,
    has_embedding INTEGER NOT NULL DEFAULT 0,
    embedding_dim INTEGER,
    status TEXT NOT NULL,
    current_step TEXT NOT NULL,
    last_error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_recordings_status_created_at
    ON recordings (status, created_at DESC);

CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    recording_id TEXT NOT NULL UNIQUE REFERENCES recordings(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    step TEXT NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER,
    next_attempt_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jobs_queue
    ON jobs (status, next_attempt_at, created_at);

CREATE VIRTUAL TABLE IF NOT EXISTS recordings_fts USING fts5(
    recording_id UNINDEXED,
    original_filename,
    summary_canonical_text,
    transcript,
    tokenize = 'unicode61'
);
"#;

#[async_trait]
pub trait RecordingRepository: Send + Sync {
    async fn insert_recording_with_job(&self, new_recording: NewRecording) -> Result<(Recording, Job)>;
    async fn get_job(&self, job_id: Uuid) -> Result<Option<Job>>;
    async fn get_recording(&self, recording_id: Uuid) -> Result<Option<Recording>>;
    async fn get_recording_bundle(&self, recording_id: Uuid) -> Result<Option<RecordingBundle>>;
    async fn list_recordings(&self, status: Option<ProcessingStatus>) -> Result<Vec<RecordingListItem>>;
    async fn claim_next_job(&self) -> Result<Option<Job>>;
    async fn mark_wav_ready(&self, recording_id: Uuid, job_id: Uuid, wav_rel_path: &str) -> Result<()>;
    async fn save_transcription(
        &self,
        recording_id: Uuid,
        job_id: Uuid,
        language: Option<&str>,
        transcript: &str,
    ) -> Result<()>;
    async fn save_summary(
        &self,
        recording_id: Uuid,
        job_id: Uuid,
        summary: &StructuredSummary,
        canonical_text: &str,
    ) -> Result<()>;
    async fn save_embedding(&self, recording_id: Uuid, embedding: &[f32]) -> Result<()>;
    async fn complete_job(&self, recording_id: Uuid, job_id: Uuid) -> Result<()>;
    async fn reschedule_or_fail_job(
        &self,
        job: &Job,
        step: ProcessingStep,
        error: &str,
        max_attempts: i32,
        retry_backoff: Duration,
    ) -> Result<()>;
    async fn search_recordings(
        &self,
        query: &str,
        query_embedding: Option<&[f32]>,
        limit: i64,
    ) -> Result<Vec<SearchResult>>;
}

#[derive(Debug, Clone)]
pub struct SavedUpload {
    pub absolute_path: PathBuf,
    pub relative_path: String,
    pub sanitized_filename: String,
    pub file_size_bytes: i64,
}

#[derive(Debug, Clone)]
pub struct LocalFileStore {
    root: PathBuf,
}

impl LocalFileStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub async fn ensure_root(&self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .await
            .with_context(|| format!("failed to create storage root {}", self.root.display()))
    }

    pub async fn save_upload_field(
        &self,
        recording_id: Uuid,
        original_filename: &str,
        content_type: Option<&str>,
        field: &mut Field<'_>,
        max_size: usize,
    ) -> Result<SavedUpload> {
        let extension = file_extension(original_filename)
            .ok_or_else(|| anyhow!("uploaded file must have an extension"))?;
        if !is_supported_audio_extension(&extension) {
            bail!("unsupported file extension `{extension}`")
        }

        if let Some(content_type) = content_type {
            if !is_supported_content_type(content_type) {
                bail!("unsupported content type `{content_type}`");
            }
        }

        let sanitized_filename = sanitize_filename(original_filename);
        let absolute_path = self.original_abs_path(recording_id, &sanitized_filename);
        let relative_path = self.relative_path(&absolute_path)?;

        if let Some(parent) = absolute_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file = fs::File::create(&absolute_path)
            .await
            .with_context(|| format!("failed to create upload target {}", absolute_path.display()))?;
        let mut size = 0usize;

        while let Some(chunk) = field.chunk().await? {
            size += chunk.len();
            if size > max_size {
                bail!("file exceeds max upload size of {max_size} bytes");
            }
            file.write_all(&chunk).await?;
        }
        file.flush().await?;

        Ok(SavedUpload {
            absolute_path,
            relative_path,
            sanitized_filename,
            file_size_bytes: size as i64,
        })
    }

    pub fn wav_abs_path(&self, recording_id: Uuid) -> PathBuf {
        self.root
            .join(recording_id.to_string())
            .join("wav")
            .join("standard.wav")
    }

    pub fn artifact_abs_path(&self, recording_id: Uuid, artifact_name: &str) -> PathBuf {
        self.root
            .join(recording_id.to_string())
            .join("artifacts")
            .join(artifact_name)
    }

    pub fn absolute_path_from_rel(&self, relative_path: &str) -> PathBuf {
        let relative = relative_path.replace('/', std::path::MAIN_SEPARATOR_STR);
        self.root.join(relative)
    }

    pub async fn write_json_artifact(
        &self,
        recording_id: Uuid,
        artifact_name: &str,
        value: &Value,
    ) -> Result<String> {
        let absolute_path = self.artifact_abs_path(recording_id, artifact_name);
        if let Some(parent) = absolute_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&absolute_path, serde_json::to_vec_pretty(value)?)
            .await
            .with_context(|| format!("failed to write artifact {}", absolute_path.display()))?;
        self.relative_path(&absolute_path)
    }

    pub async fn remove_recording_dir(&self, recording_id: Uuid) -> Result<()> {
        let target = self.root.join(recording_id.to_string());
        if fs::try_exists(&target).await? {
            fs::remove_dir_all(&target).await?;
        }
        Ok(())
    }

    pub fn relative_path(&self, absolute_path: &Path) -> Result<String> {
        let relative = absolute_path
            .strip_prefix(&self.root)
            .with_context(|| format!("path {} is outside storage root", absolute_path.display()))?;
        Ok(relative.to_string_lossy().replace('\\', "/"))
    }

    fn original_abs_path(&self, recording_id: Uuid, filename: &str) -> PathBuf {
        self.root
            .join(recording_id.to_string())
            .join("original")
            .join(filename)
    }
}

#[derive(Debug, Clone)]
struct ArtifactSimilarityEngine {
    storage_root: PathBuf,
}

impl ArtifactSimilarityEngine {
    fn new(storage_root: PathBuf) -> Self {
        Self { storage_root }
    }

    async fn similarity_for_recording(
        &self,
        recording_id: Uuid,
        expected_dim: Option<usize>,
        query_embedding: &[f32],
    ) -> Option<f32> {
        let path = self
            .storage_root
            .join(recording_id.to_string())
            .join("artifacts")
            .join("embedding.json");
        let bytes = match fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!("skipping similarity for {}: failed to read {}: {}", recording_id, path.display(), error);
                return None;
            }
        };

        let artifact = match serde_json::from_slice::<EmbeddingArtifact>(&bytes) {
            Ok(artifact) => artifact,
            Err(error) => {
                tracing::warn!("skipping similarity for {}: invalid embedding artifact {}: {}", recording_id, path.display(), error);
                return None;
            }
        };

        let dimension = artifact.dimensions.unwrap_or(artifact.embedding.len());
        if expected_dim.is_some_and(|value| value != dimension) {
            tracing::warn!(
                "skipping similarity for {}: db dimension {} does not match artifact dimension {}",
                recording_id,
                expected_dim.unwrap_or_default(),
                dimension
            );
            return None;
        }
        if dimension != artifact.embedding.len() {
            tracing::warn!(
                "skipping similarity for {}: artifact dimension {} does not match vector length {}",
                recording_id,
                dimension,
                artifact.embedding.len()
            );
            return None;
        }
        if dimension != query_embedding.len() {
            tracing::warn!(
                "skipping similarity for {}: query dimension {} does not match artifact dimension {}",
                recording_id,
                query_embedding.len(),
                dimension
            );
            return None;
        }

        Some(cosine_similarity(query_embedding, &artifact.embedding))
    }
}

#[derive(Debug, Deserialize)]
struct EmbeddingArtifact {
    dimensions: Option<usize>,
    embedding: Vec<f32>,
}

pub struct SqliteRepository {
    db_path: PathBuf,
    similarity_engine: ArtifactSimilarityEngine,
}

impl SqliteRepository {
    pub async fn new(db_path: PathBuf, storage_root: PathBuf) -> Result<Self> {
        let repository = Self {
            db_path,
            similarity_engine: ArtifactSimilarityEngine::new(storage_root),
        };
        repository.initialize().await?;
        Ok(repository)
    }

    async fn initialize(&self) -> Result<()> {
        self.with_connection(|connection| {
            connection.execute_batch(SQLITE_SCHEMA)?;
            Ok(())
        })
        .await
    }

    async fn with_connection<T, F>(&self, func: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    {
        let db_path = self.db_path.clone();
        task::spawn_blocking(move || {
            let mut connection = open_sqlite_connection(&db_path)?;
            func(&mut connection)
        })
        .await
        .context("sqlite task join failed")?
    }
}
#[async_trait]
impl RecordingRepository for SqliteRepository {
    async fn insert_recording_with_job(&self, new_recording: NewRecording) -> Result<(Recording, Job)> {
        let now = now_timestamp();
        let new_recording_clone = new_recording.clone();
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let job_id = Uuid::new_v4();

            transaction.execute(
                r#"
                INSERT INTO recordings (
                    id,
                    original_filename,
                    original_content_type,
                    file_size_bytes,
                    original_rel_path,
                    status,
                    current_step,
                    created_at,
                    updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
                params![
                    new_recording_clone.id.to_string(),
                    new_recording_clone.original_filename.clone(),
                    new_recording_clone.original_content_type.clone(),
                    new_recording_clone.file_size_bytes,
                    new_recording_clone.original_rel_path.clone(),
                    ProcessingStatus::Queued.as_str(),
                    ProcessingStep::UploadSaved.as_str(),
                    now,
                    now,
                ],
            )?;

            transaction.execute(
                r#"
                INSERT INTO jobs (
                    id,
                    recording_id,
                    status,
                    step,
                    attempt_count,
                    created_at,
                    updated_at,
                    next_attempt_at
                )
                VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7)
                "#,
                params![
                    job_id.to_string(),
                    new_recording_clone.id.to_string(),
                    ProcessingStatus::Queued.as_str(),
                    ProcessingStep::UploadSaved.as_str(),
                    now,
                    now,
                    now,
                ],
            )?;

            sync_recording_fts(&transaction, new_recording_clone.id)?;
            transaction.commit()?;

            Ok((
                Recording {
                    id: new_recording_clone.id,
                    original_filename: new_recording_clone.original_filename,
                    original_content_type: new_recording_clone.original_content_type,
                    file_size_bytes: new_recording_clone.file_size_bytes,
                    original_rel_path: new_recording_clone.original_rel_path,
                    wav_rel_path: None,
                    language: None,
                    transcript: None,
                    summary: None,
                    summary_canonical_text: None,
                    status: ProcessingStatus::Queued,
                    current_step: ProcessingStep::UploadSaved,
                    last_error: None,
                    created_at: datetime_from_timestamp(now)?,
                    updated_at: datetime_from_timestamp(now)?,
                },
                Job {
                    id: job_id,
                    recording_id: new_recording_clone.id,
                    status: ProcessingStatus::Queued,
                    step: ProcessingStep::UploadSaved,
                    attempt_count: 0,
                    last_error: None,
                    created_at: datetime_from_timestamp(now)?,
                    updated_at: datetime_from_timestamp(now)?,
                    started_at: None,
                    completed_at: None,
                    next_attempt_at: datetime_from_timestamp(now)?,
                },
            ))
        })
        .await
    }

    async fn get_job(&self, job_id: Uuid) -> Result<Option<Job>> {
        self.with_connection(move |connection| {
            let row = connection
                .query_row(
                    "SELECT * FROM jobs WHERE id = ?1",
                    params![job_id.to_string()],
                    job_db_row_from_row,
                )
                .optional()?;
            row.map(TryInto::try_into).transpose()
        })
        .await
    }

    async fn get_recording(&self, recording_id: Uuid) -> Result<Option<Recording>> {
        self.with_connection(move |connection| {
            let row = connection
                .query_row(
                    "SELECT * FROM recordings WHERE id = ?1",
                    params![recording_id.to_string()],
                    recording_db_row_from_row,
                )
                .optional()?;
            row.map(TryInto::try_into).transpose()
        })
        .await
    }

    async fn get_recording_bundle(&self, recording_id: Uuid) -> Result<Option<RecordingBundle>> {
        let Some(recording) = self.get_recording(recording_id).await? else {
            return Ok(None);
        };
        let job = self
            .with_connection(move |connection| {
                let row = connection
                    .query_row(
                        "SELECT * FROM jobs WHERE recording_id = ?1",
                        params![recording_id.to_string()],
                        job_db_row_from_row,
                    )
                    .optional()?;
                row.map(TryInto::try_into).transpose()
            })
            .await?;
        Ok(Some(RecordingBundle { recording, job }))
    }

    async fn list_recordings(&self, status: Option<ProcessingStatus>) -> Result<Vec<RecordingListItem>> {
        self.with_connection(move |connection| {
            let sql = if status.is_some() {
                r#"
                SELECT
                    id,
                    original_filename,
                    status,
                    current_step,
                    language,
                    transcript IS NOT NULL AS has_transcript,
                    summary IS NOT NULL AS has_summary,
                    last_error,
                    created_at,
                    updated_at
                FROM recordings
                WHERE status = ?1
                ORDER BY created_at DESC
                "#
            } else {
                r#"
                SELECT
                    id,
                    original_filename,
                    status,
                    current_step,
                    language,
                    transcript IS NOT NULL AS has_transcript,
                    summary IS NOT NULL AS has_summary,
                    last_error,
                    created_at,
                    updated_at
                FROM recordings
                ORDER BY created_at DESC
                "#
            };

            let mut statement = connection.prepare(sql)?;
            if let Some(status) = status {
                statement
                    .query_map(params![status.as_str()], recording_list_db_row_from_row)?
                    .collect::<std::result::Result<Vec<_>, _>>()?
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect()
            } else {
                statement
                    .query_map([], recording_list_db_row_from_row)?
                    .collect::<std::result::Result<Vec<_>, _>>()?
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect()
            }
        })
        .await
    }

    async fn claim_next_job(&self) -> Result<Option<Job>> {
        let now = now_timestamp();
        self.with_connection(move |connection| {
            let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let row = transaction
                .query_row(
                    r#"
                    SELECT *
                    FROM jobs
                    WHERE status = ?1
                      AND next_attempt_at <= ?2
                    ORDER BY created_at ASC
                    LIMIT 1
                    "#,
                    params![ProcessingStatus::Queued.as_str(), now],
                    job_db_row_from_row,
                )
                .optional()?;

            let Some(mut job_row) = row else {
                transaction.commit()?;
                return Ok(None);
            };

            let started_at = job_row.started_at.unwrap_or(now);
            job_row.status = ProcessingStatus::Processing.as_str().to_string();
            job_row.updated_at = now;
            job_row.started_at = Some(started_at);
            job_row.attempt_count += 1;
            job_row.last_error = None;

            transaction.execute(
                r#"
                UPDATE jobs
                SET status = ?2,
                    started_at = COALESCE(started_at, ?3),
                    updated_at = ?4,
                    attempt_count = ?5,
                    last_error = NULL
                WHERE id = ?1
                "#,
                params![
                    job_row.id,
                    ProcessingStatus::Processing.as_str(),
                    started_at,
                    now,
                    job_row.attempt_count,
                ],
            )?;
            transaction.execute(
                r#"
                UPDATE recordings
                SET status = ?2,
                    updated_at = ?3,
                    last_error = NULL
                WHERE id = ?1
                "#,
                params![job_row.recording_id, ProcessingStatus::Processing.as_str(), now],
            )?;

            transaction.commit()?;
            Ok(Some(job_row.try_into()?))
        })
        .await
    }
    async fn mark_wav_ready(&self, recording_id: Uuid, job_id: Uuid, wav_rel_path: &str) -> Result<()> {
        let wav_rel_path = wav_rel_path.to_string();
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let now = now_timestamp();
            transaction.execute(
                r#"
                UPDATE recordings
                SET wav_rel_path = ?2,
                    current_step = ?3,
                    updated_at = ?4,
                    last_error = NULL
                WHERE id = ?1
                "#,
                params![recording_id.to_string(), wav_rel_path, ProcessingStep::WavReady.as_str(), now],
            )?;
            update_steps(&transaction, recording_id, job_id, ProcessingStep::WavReady, None, now)?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    async fn save_transcription(
        &self,
        recording_id: Uuid,
        job_id: Uuid,
        language: Option<&str>,
        transcript: &str,
    ) -> Result<()> {
        let language = language.map(str::to_string);
        let transcript = transcript.to_string();
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let now = now_timestamp();
            transaction.execute(
                r#"
                UPDATE recordings
                SET language = ?2,
                    transcript = ?3,
                    current_step = ?4,
                    updated_at = ?5,
                    last_error = NULL
                WHERE id = ?1
                "#,
                params![
                    recording_id.to_string(),
                    language,
                    transcript,
                    ProcessingStep::Transcribed.as_str(),
                    now,
                ],
            )?;
            update_steps(&transaction, recording_id, job_id, ProcessingStep::Transcribed, None, now)?;
            sync_recording_fts(&transaction, recording_id)?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    async fn save_summary(
        &self,
        recording_id: Uuid,
        job_id: Uuid,
        summary: &StructuredSummary,
        canonical_text: &str,
    ) -> Result<()> {
        let summary_json = serde_json::to_string(summary)?;
        let canonical_text = canonical_text.to_string();
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let now = now_timestamp();
            transaction.execute(
                r#"
                UPDATE recordings
                SET summary = ?2,
                    summary_canonical_text = ?3,
                    current_step = ?4,
                    updated_at = ?5,
                    last_error = NULL
                WHERE id = ?1
                "#,
                params![
                    recording_id.to_string(),
                    summary_json,
                    canonical_text,
                    ProcessingStep::Summarized.as_str(),
                    now,
                ],
            )?;
            update_steps(&transaction, recording_id, job_id, ProcessingStep::Summarized, None, now)?;
            sync_recording_fts(&transaction, recording_id)?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    async fn save_embedding(&self, recording_id: Uuid, embedding: &[f32]) -> Result<()> {
        let dimension = embedding.len() as i64;
        self.with_connection(move |connection| {
            connection.execute(
                r#"
                UPDATE recordings
                SET has_embedding = 1,
                    embedding_dim = ?2,
                    updated_at = ?3
                WHERE id = ?1
                "#,
                params![recording_id.to_string(), dimension, now_timestamp()],
            )?;
            Ok(())
        })
        .await
    }

    async fn complete_job(&self, recording_id: Uuid, job_id: Uuid) -> Result<()> {
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let now = now_timestamp();
            transaction.execute(
                r#"
                UPDATE recordings
                SET status = ?2,
                    current_step = ?3,
                    updated_at = ?4,
                    last_error = NULL
                WHERE id = ?1
                "#,
                params![
                    recording_id.to_string(),
                    ProcessingStatus::Completed.as_str(),
                    ProcessingStep::Embedded.as_str(),
                    now,
                ],
            )?;
            transaction.execute(
                r#"
                UPDATE jobs
                SET status = ?2,
                    step = ?3,
                    updated_at = ?4,
                    completed_at = ?5,
                    last_error = NULL
                WHERE id = ?1
                "#,
                params![
                    job_id.to_string(),
                    ProcessingStatus::Completed.as_str(),
                    ProcessingStep::Embedded.as_str(),
                    now,
                    now,
                ],
            )?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    async fn reschedule_or_fail_job(
        &self,
        job: &Job,
        step: ProcessingStep,
        error: &str,
        max_attempts: i32,
        retry_backoff: Duration,
    ) -> Result<()> {
        let job = job.clone();
        let error = error.to_string();
        self.with_connection(move |connection| {
            let final_status = if job.attempt_count >= max_attempts {
                ProcessingStatus::Failed
            } else {
                ProcessingStatus::Queued
            };
            let transaction = connection.transaction()?;
            let now = now_timestamp();

            transaction.execute(
                r#"
                UPDATE recordings
                SET status = ?2,
                    current_step = ?3,
                    updated_at = ?4,
                    last_error = ?5
                WHERE id = ?1
                "#,
                params![
                    job.recording_id.to_string(),
                    final_status.as_str(),
                    step.as_str(),
                    now,
                    error.clone(),
                ],
            )?;

            if final_status == ProcessingStatus::Failed {
                transaction.execute(
                    r#"
                    UPDATE jobs
                    SET status = ?2,
                        step = ?3,
                        updated_at = ?4,
                        completed_at = ?5,
                        last_error = ?6
                    WHERE id = ?1
                    "#,
                    params![
                        job.id.to_string(),
                        final_status.as_str(),
                        step.as_str(),
                        now,
                        now,
                        error,
                    ],
                )?;
            } else {
                transaction.execute(
                    r#"
                    UPDATE jobs
                    SET status = ?2,
                        step = ?3,
                        updated_at = ?4,
                        next_attempt_at = ?5,
                        completed_at = NULL,
                        last_error = ?6
                    WHERE id = ?1
                    "#,
                    params![
                        job.id.to_string(),
                        final_status.as_str(),
                        step.as_str(),
                        now,
                        now + retry_backoff.as_secs() as i64,
                        error,
                    ],
                )?;
            }

            transaction.commit()?;
            Ok(())
        })
        .await
    }

    async fn search_recordings(
        &self,
        query: &str,
        query_embedding: Option<&[f32]>,
        limit: i64,
    ) -> Result<Vec<SearchResult>> {
        let query = query.to_string();
        let keyword_scores = self
            .with_connection(move |connection| fetch_keyword_scores(connection, &query))
            .await?;
        let candidates = self.with_connection(fetch_search_candidates).await?;

        let mut results = Vec::new();
        for candidate in candidates {
            let keyword_score = keyword_scores.get(&candidate.id).copied().unwrap_or(0.0);
            let keyword_hit = keyword_score > 0.0;
            let similarity_score = match query_embedding {
                Some(query_embedding) if candidate.has_embedding => {
                    self.similarity_engine
                        .similarity_for_recording(
                            candidate.recording_id()?,
                            candidate.embedding_dim.map(|value| value as usize),
                            query_embedding,
                        )
                        .await
                }
                _ => None,
            };

            if !keyword_hit && similarity_score.is_none() {
                continue;
            }

            results.push(SearchResult {
                id: candidate.recording_id()?,
                original_filename: candidate.original_filename,
                status: candidate.status.parse()?,
                current_step: candidate.current_step.parse()?,
                summary_excerpt: candidate.summary_canonical_text,
                transcript_excerpt: candidate.transcript,
                keyword_hit,
                keyword_score,
                similarity_score,
                created_at: datetime_from_timestamp(candidate.created_at)?,
            });
        }

        results.sort_by(compare_search_results);
        results.truncate(limit.max(0) as usize);
        Ok(results)
    }
}
#[derive(Debug)]
struct RecordingDbRow {
    id: String,
    original_filename: String,
    original_content_type: Option<String>,
    file_size_bytes: i64,
    original_rel_path: String,
    wav_rel_path: Option<String>,
    language: Option<String>,
    transcript: Option<String>,
    summary: Option<String>,
    summary_canonical_text: Option<String>,
    status: String,
    current_step: String,
    last_error: Option<String>,
    created_at: i64,
    updated_at: i64,
}

impl TryFrom<RecordingDbRow> for Recording {
    type Error = anyhow::Error;

    fn try_from(value: RecordingDbRow) -> Result<Self> {
        Ok(Self {
            id: parse_uuid(&value.id)?,
            original_filename: value.original_filename,
            original_content_type: value.original_content_type,
            file_size_bytes: value.file_size_bytes,
            original_rel_path: value.original_rel_path,
            wav_rel_path: value.wav_rel_path,
            language: value.language,
            transcript: value.transcript,
            summary: parse_summary(value.summary)?,
            summary_canonical_text: value.summary_canonical_text,
            status: value.status.parse()?,
            current_step: value.current_step.parse()?,
            last_error: value.last_error,
            created_at: datetime_from_timestamp(value.created_at)?,
            updated_at: datetime_from_timestamp(value.updated_at)?,
        })
    }
}

#[derive(Debug)]
struct JobDbRow {
    id: String,
    recording_id: String,
    status: String,
    step: String,
    attempt_count: i32,
    last_error: Option<String>,
    created_at: i64,
    updated_at: i64,
    started_at: Option<i64>,
    completed_at: Option<i64>,
    next_attempt_at: i64,
}

impl TryFrom<JobDbRow> for Job {
    type Error = anyhow::Error;

    fn try_from(value: JobDbRow) -> Result<Self> {
        Ok(Self {
            id: parse_uuid(&value.id)?,
            recording_id: parse_uuid(&value.recording_id)?,
            status: value.status.parse()?,
            step: value.step.parse()?,
            attempt_count: value.attempt_count,
            last_error: value.last_error,
            created_at: datetime_from_timestamp(value.created_at)?,
            updated_at: datetime_from_timestamp(value.updated_at)?,
            started_at: value.started_at.map(datetime_from_timestamp).transpose()?,
            completed_at: value.completed_at.map(datetime_from_timestamp).transpose()?,
            next_attempt_at: datetime_from_timestamp(value.next_attempt_at)?,
        })
    }
}

#[derive(Debug)]
struct RecordingListDbRow {
    id: String,
    original_filename: String,
    status: String,
    current_step: String,
    language: Option<String>,
    has_transcript: i64,
    has_summary: i64,
    last_error: Option<String>,
    created_at: i64,
    updated_at: i64,
}

impl TryFrom<RecordingListDbRow> for RecordingListItem {
    type Error = anyhow::Error;

    fn try_from(value: RecordingListDbRow) -> Result<Self> {
        Ok(Self {
            id: parse_uuid(&value.id)?,
            original_filename: value.original_filename,
            status: value.status.parse()?,
            current_step: value.current_step.parse()?,
            language: value.language,
            has_transcript: value.has_transcript != 0,
            has_summary: value.has_summary != 0,
            last_error: value.last_error,
            created_at: datetime_from_timestamp(value.created_at)?,
            updated_at: datetime_from_timestamp(value.updated_at)?,
        })
    }
}

#[derive(Debug)]
struct SearchCandidateRow {
    id: String,
    original_filename: String,
    status: String,
    current_step: String,
    summary_canonical_text: Option<String>,
    transcript: Option<String>,
    has_embedding: bool,
    embedding_dim: Option<i64>,
    created_at: i64,
}

impl SearchCandidateRow {
    fn recording_id(&self) -> Result<Uuid> {
        parse_uuid(&self.id)
    }
}

fn open_sqlite_connection(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create sqlite parent directory {}", parent.display()))?;
    }

    let connection = Connection::open(path)
        .with_context(|| format!("failed to open sqlite database {}", path.display()))?;
    connection.busy_timeout(SQLITE_BUSY_TIMEOUT)?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;",
    )?;
    Ok(connection)
}

fn update_steps(
    connection: &Connection,
    recording_id: Uuid,
    job_id: Uuid,
    step: ProcessingStep,
    last_error: Option<&str>,
    now: i64,
) -> Result<()> {
    connection.execute(
        r#"
        UPDATE recordings
        SET current_step = ?2,
            updated_at = ?3,
            last_error = ?4
        WHERE id = ?1
        "#,
        params![recording_id.to_string(), step.as_str(), now, last_error],
    )?;
    connection.execute(
        r#"
        UPDATE jobs
        SET step = ?2,
            updated_at = ?3,
            last_error = ?4
        WHERE id = ?1
        "#,
        params![job_id.to_string(), step.as_str(), now, last_error],
    )?;
    Ok(())
}

fn sync_recording_fts(connection: &Connection, recording_id: Uuid) -> Result<()> {
    let payload = connection
        .query_row(
            r#"
            SELECT original_filename, summary_canonical_text, transcript
            FROM recordings
            WHERE id = ?1
            "#,
            params![recording_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()?;

    connection.execute(
        "DELETE FROM recordings_fts WHERE recording_id = ?1",
        params![recording_id.to_string()],
    )?;

    if let Some((original_filename, summary_canonical_text, transcript)) = payload {
        connection.execute(
            r#"
            INSERT INTO recordings_fts (
                recording_id,
                original_filename,
                summary_canonical_text,
                transcript
            )
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![
                recording_id.to_string(),
                original_filename,
                summary_canonical_text.unwrap_or_default(),
                transcript.unwrap_or_default(),
            ],
        )?;
    }

    Ok(())
}

fn fetch_keyword_scores(connection: &Connection, query: &str) -> Result<HashMap<String, f32>> {
    let mut scores: HashMap<String, f32> = HashMap::new();

    if let Some(fts_query) = build_fts_query(query) {
        let mut statement = connection.prepare(
            r#"
            SELECT recording_id, CAST(-bm25(recordings_fts) AS REAL) AS keyword_score
            FROM recordings_fts
            WHERE recordings_fts MATCH ?1
            "#,
        )?;
        let rows = statement.query_map(params![fts_query], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
        })?;

        for row in rows {
            let (recording_id, keyword_score) = row?;
            scores
                .entry(recording_id)
                .and_modify(|score| *score = score.max(keyword_score))
                .or_insert(keyword_score);
        }
    }

    let mut like_statement = connection.prepare(
        r#"
        SELECT id
        FROM recordings
        WHERE lower(original_filename) LIKE '%' || lower(?1) || '%'
        "#,
    )?;
    let like_rows = like_statement.query_map(params![query], |row| row.get::<_, String>(0))?;
    for row in like_rows {
        let recording_id = row?;
        scores
            .entry(recording_id)
            .and_modify(|score| *score = score.max(0.25))
            .or_insert(0.25);
    }

    Ok(scores)
}

fn fetch_search_candidates(connection: &mut Connection) -> Result<Vec<SearchCandidateRow>> {
    let mut statement = connection.prepare(
        r#"
        SELECT
            id,
            original_filename,
            status,
            current_step,
            summary_canonical_text,
            transcript,
            has_embedding,
            embedding_dim,
            created_at
        FROM recordings
        ORDER BY created_at DESC
        "#,
    )?;
    let rows = statement.query_map([], |row| {
        Ok(SearchCandidateRow {
            id: row.get(0)?,
            original_filename: row.get(1)?,
            status: row.get(2)?,
            current_step: row.get(3)?,
            summary_canonical_text: row.get(4)?,
            transcript: row.get(5)?,
            has_embedding: row.get::<_, i64>(6)? != 0,
            embedding_dim: row.get(7)?,
            created_at: row.get(8)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn build_fts_query(query: &str) -> Option<String> {
    let tokens = query
        .split_whitespace()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" AND "))
    }
}
fn recording_db_row_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RecordingDbRow> {
    Ok(RecordingDbRow {
        id: row.get("id")?,
        original_filename: row.get("original_filename")?,
        original_content_type: row.get("original_content_type")?,
        file_size_bytes: row.get("file_size_bytes")?,
        original_rel_path: row.get("original_rel_path")?,
        wav_rel_path: row.get("wav_rel_path")?,
        language: row.get("language")?,
        transcript: row.get("transcript")?,
        summary: row.get("summary")?,
        summary_canonical_text: row.get("summary_canonical_text")?,
        status: row.get("status")?,
        current_step: row.get("current_step")?,
        last_error: row.get("last_error")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn job_db_row_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JobDbRow> {
    Ok(JobDbRow {
        id: row.get("id")?,
        recording_id: row.get("recording_id")?,
        status: row.get("status")?,
        step: row.get("step")?,
        attempt_count: row.get("attempt_count")?,
        last_error: row.get("last_error")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        started_at: row.get("started_at")?,
        completed_at: row.get("completed_at")?,
        next_attempt_at: row.get("next_attempt_at")?,
    })
}

fn recording_list_db_row_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RecordingListDbRow> {
    Ok(RecordingListDbRow {
        id: row.get("id")?,
        original_filename: row.get("original_filename")?,
        status: row.get("status")?,
        current_step: row.get("current_step")?,
        language: row.get("language")?,
        has_transcript: row.get("has_transcript")?,
        has_summary: row.get("has_summary")?,
        last_error: row.get("last_error")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn parse_summary(raw: Option<String>) -> Result<Option<StructuredSummary>> {
    raw.map(|value| serde_json::from_str(&value).with_context(|| format!("invalid summary json: {value}")))
        .transpose()
}

fn parse_uuid(value: &str) -> Result<Uuid> {
    Uuid::parse_str(value).with_context(|| format!("invalid uuid `{value}` in sqlite row"))
}

fn now_timestamp() -> i64 {
    Utc::now().timestamp()
}

fn datetime_from_timestamp(value: i64) -> Result<DateTime<Utc>> {
    DateTime::from_timestamp(value, 0)
        .ok_or_else(|| anyhow!("invalid sqlite timestamp `{value}`"))
}

fn compare_search_results(left: &SearchResult, right: &SearchResult) -> Ordering {
    right
        .keyword_hit
        .cmp(&left.keyword_hit)
        .then_with(|| compare_f32_desc(left.keyword_score, right.keyword_score))
        .then_with(|| compare_option_f32_desc(left.similarity_score, right.similarity_score))
        .then_with(|| right.created_at.cmp(&left.created_at))
}

fn compare_option_f32_desc(left: Option<f32>, right: Option<f32>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => compare_f32_desc(left, right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_f32_desc(left: f32, right: f32) -> Ordering {
    right.partial_cmp(&left).unwrap_or(Ordering::Equal)
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let dot = left.iter().zip(right.iter()).map(|(a, b)| a * b).sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm * right_norm)
    }
}

fn file_extension(filename: &str) -> Option<String> {
    Path::new(filename)
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
}

fn is_supported_audio_extension(extension: &str) -> bool {
    matches!(extension, "mp3" | "m4a" | "wav")
}

fn is_supported_content_type(content_type: &str) -> bool {
    matches!(
        content_type,
        "audio/mpeg"
            | "audio/mp3"
            | "audio/mp4"
            | "audio/x-m4a"
            | "audio/m4a"
            | "audio/wav"
            | "audio/x-wav"
    )
}

fn sanitize_filename(filename: &str) -> String {
    let sanitized = filename
        .chars()
        .map(|char| match char {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => char,
            _ => '_',
        })
        .collect::<String>();

    if sanitized.is_empty() {
        "upload.wav".to_string()
    } else {
        sanitized
    }
}

