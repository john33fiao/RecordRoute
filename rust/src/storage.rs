use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use axum::extract::multipart::Field;
use chrono::{DateTime, Utc};
use pgvector::Vector;
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{FromRow, PgPool};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::models::{
    Job, NewRecording, ProcessingStatus, ProcessingStep, Recording, RecordingBundle,
    RecordingListItem, SearchResult, StructuredSummary,
};

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

pub struct PostgresRepository {
    pool: PgPool,
}

impl PostgresRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn get_job_by_recording_id(&self, recording_id: Uuid) -> Result<Option<Job>> {
        let row = sqlx::query_as::<_, JobRow>(
            r#"
            SELECT *
            FROM jobs
            WHERE recording_id = $1
            "#,
        )
        .bind(recording_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(TryInto::try_into).transpose()
    }

    async fn update_steps(
        &self,
        recording_id: Uuid,
        job_id: Uuid,
        step: ProcessingStep,
        last_error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE recordings
            SET current_step = $2,
                updated_at = NOW(),
                last_error = $3
            WHERE id = $1
            "#,
        )
        .bind(recording_id)
        .bind(step.as_str())
        .bind(last_error)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            UPDATE jobs
            SET step = $2,
                updated_at = NOW(),
                last_error = $3
            WHERE id = $1
            "#,
        )
        .bind(job_id)
        .bind(step.as_str())
        .bind(last_error)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn embedding_table_exists(&self) -> Result<bool> {
        Ok(sqlx::query_scalar::<_, bool>("SELECT to_regclass('public.recording_embeddings') IS NOT NULL")
            .fetch_one(&self.pool)
            .await?)
    }

    async fn ensure_embedding_table_for_dimension(&self, dimension: usize) -> Result<()> {
        if self.embedding_table_exists().await? {
            let type_name = sqlx::query_scalar::<_, String>(
                r#"
                SELECT format_type(a.atttypid, a.atttypmod)
                FROM pg_attribute a
                JOIN pg_class c ON c.oid = a.attrelid
                JOIN pg_namespace n ON n.oid = c.relnamespace
                WHERE n.nspname = 'public'
                  AND c.relname = 'recording_embeddings'
                  AND a.attname = 'embedding'
                  AND NOT a.attisdropped
                "#,
            )
            .fetch_one(&self.pool)
            .await?;

            let existing_dimension = type_name
                .strip_prefix("vector(")
                .and_then(|value| value.strip_suffix(')'))
                .ok_or_else(|| anyhow!("unexpected pgvector type `{type_name}`"))?
                .parse::<usize>()?;

            if existing_dimension != dimension {
                bail!(
                    "embedding dimension mismatch: existing table uses {existing_dimension}, request uses {dimension}"
                );
            }
            return Ok(());
        }

        let create_table = format!(
            r#"
            CREATE TABLE IF NOT EXISTS recording_embeddings (
                recording_id UUID PRIMARY KEY REFERENCES recordings(id) ON DELETE CASCADE,
                embedding vector({dimension}) NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        );
        sqlx::query(&create_table).execute(&self.pool).await?;
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_recording_embeddings_cosine
            ON recording_embeddings USING ivfflat (embedding vector_cosine_ops)
            WITH (lists = 100)
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[async_trait]
impl RecordingRepository for PostgresRepository {
    async fn insert_recording_with_job(&self, new_recording: NewRecording) -> Result<(Recording, Job)> {
        let mut tx = self.pool.begin().await?;

        let recording = sqlx::query_as::<_, RecordingRow>(
            r#"
            INSERT INTO recordings (
                id,
                original_filename,
                original_content_type,
                file_size_bytes,
                original_rel_path,
                status,
                current_step
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(new_recording.id)
        .bind(&new_recording.original_filename)
        .bind(&new_recording.original_content_type)
        .bind(new_recording.file_size_bytes)
        .bind(&new_recording.original_rel_path)
        .bind(ProcessingStatus::Queued.as_str())
        .bind(ProcessingStep::UploadSaved.as_str())
        .fetch_one(&mut *tx)
        .await?;

        let job = sqlx::query_as::<_, JobRow>(
            r#"
            INSERT INTO jobs (
                id,
                recording_id,
                status,
                step
            )
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(new_recording.id)
        .bind(ProcessingStatus::Queued.as_str())
        .bind(ProcessingStep::UploadSaved.as_str())
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok((recording.try_into()?, job.try_into()?))
    }

    async fn get_job(&self, job_id: Uuid) -> Result<Option<Job>> {
        let row = sqlx::query_as::<_, JobRow>("SELECT * FROM jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(TryInto::try_into).transpose()
    }

    async fn get_recording(&self, recording_id: Uuid) -> Result<Option<Recording>> {
        let row = sqlx::query_as::<_, RecordingRow>("SELECT * FROM recordings WHERE id = $1")
            .bind(recording_id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(TryInto::try_into).transpose()
    }

    async fn get_recording_bundle(&self, recording_id: Uuid) -> Result<Option<RecordingBundle>> {
        let Some(recording) = self.get_recording(recording_id).await? else {
            return Ok(None);
        };
        let job = self.get_job_by_recording_id(recording_id).await?;
        Ok(Some(RecordingBundle { recording, job }))
    }

    async fn list_recordings(&self, status: Option<ProcessingStatus>) -> Result<Vec<RecordingListItem>> {
        let rows = if let Some(status) = status {
            sqlx::query_as::<_, RecordingListRow>(
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
                WHERE status = $1
                ORDER BY created_at DESC
                "#,
            )
            .bind(status.as_str())
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, RecordingListRow>(
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
                "#,
            )
            .fetch_all(&self.pool)
            .await?
        };

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn claim_next_job(&self) -> Result<Option<Job>> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_as::<_, JobRow>(
            r#"
            WITH next_job AS (
                SELECT id
                FROM jobs
                WHERE status = 'queued'
                  AND next_attempt_at <= NOW()
                ORDER BY created_at ASC
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            UPDATE jobs AS j
            SET status = 'processing',
                started_at = COALESCE(j.started_at, NOW()),
                updated_at = NOW(),
                attempt_count = j.attempt_count + 1,
                last_error = NULL
            FROM next_job
            WHERE j.id = next_job.id
            RETURNING j.*
            "#,
        )
        .fetch_optional(&mut *tx)
        .await?;

        let Some(job_row) = row else {
            tx.commit().await?;
            return Ok(None);
        };

        sqlx::query(
            r#"
            UPDATE recordings
            SET status = $2,
                updated_at = NOW(),
                last_error = NULL
            WHERE id = $1
            "#,
        )
        .bind(job_row.recording_id)
        .bind(ProcessingStatus::Processing.as_str())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(Some(job_row.try_into()?))
    }

    async fn mark_wav_ready(&self, recording_id: Uuid, job_id: Uuid, wav_rel_path: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE recordings
            SET wav_rel_path = $2,
                current_step = $3,
                updated_at = NOW(),
                last_error = NULL
            WHERE id = $1
            "#,
        )
        .bind(recording_id)
        .bind(wav_rel_path)
        .bind(ProcessingStep::WavReady.as_str())
        .execute(&self.pool)
        .await?;

        self.update_steps(recording_id, job_id, ProcessingStep::WavReady, None)
            .await
    }

    async fn save_transcription(
        &self,
        recording_id: Uuid,
        job_id: Uuid,
        language: Option<&str>,
        transcript: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE recordings
            SET language = $2,
                transcript = $3,
                current_step = $4,
                updated_at = NOW(),
                last_error = NULL
            WHERE id = $1
            "#,
        )
        .bind(recording_id)
        .bind(language)
        .bind(transcript)
        .bind(ProcessingStep::Transcribed.as_str())
        .execute(&self.pool)
        .await?;

        self.update_steps(recording_id, job_id, ProcessingStep::Transcribed, None)
            .await
    }

    async fn save_summary(
        &self,
        recording_id: Uuid,
        job_id: Uuid,
        summary: &StructuredSummary,
        canonical_text: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE recordings
            SET summary = $2,
                summary_canonical_text = $3,
                current_step = $4,
                updated_at = NOW(),
                last_error = NULL
            WHERE id = $1
            "#,
        )
        .bind(recording_id)
        .bind(Json(summary))
        .bind(canonical_text)
        .bind(ProcessingStep::Summarized.as_str())
        .execute(&self.pool)
        .await?;

        self.update_steps(recording_id, job_id, ProcessingStep::Summarized, None)
            .await
    }

    async fn save_embedding(&self, recording_id: Uuid, embedding: &[f32]) -> Result<()> {
        self.ensure_embedding_table_for_dimension(embedding.len()).await?;

        sqlx::query(
            r#"
            INSERT INTO recording_embeddings (recording_id, embedding)
            VALUES ($1, $2)
            ON CONFLICT (recording_id)
            DO UPDATE SET embedding = EXCLUDED.embedding, updated_at = NOW()
            "#,
        )
        .bind(recording_id)
        .bind(Vector::from(embedding.to_vec()))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn complete_job(&self, recording_id: Uuid, job_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            UPDATE recordings
            SET status = $2,
                current_step = $3,
                updated_at = NOW(),
                last_error = NULL
            WHERE id = $1
            "#,
        )
        .bind(recording_id)
        .bind(ProcessingStatus::Completed.as_str())
        .bind(ProcessingStep::Embedded.as_str())
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            UPDATE jobs
            SET status = $2,
                step = $3,
                updated_at = NOW(),
                completed_at = NOW(),
                last_error = NULL
            WHERE id = $1
            "#,
        )
        .bind(job_id)
        .bind(ProcessingStatus::Completed.as_str())
        .bind(ProcessingStep::Embedded.as_str())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn reschedule_or_fail_job(
        &self,
        job: &Job,
        step: ProcessingStep,
        error: &str,
        max_attempts: i32,
        retry_backoff: Duration,
    ) -> Result<()> {
        let final_status = if job.attempt_count >= max_attempts {
            ProcessingStatus::Failed
        } else {
            ProcessingStatus::Queued
        };

        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"
            UPDATE recordings
            SET status = $2,
                current_step = $3,
                updated_at = NOW(),
                last_error = $4
            WHERE id = $1
            "#,
        )
        .bind(job.recording_id)
        .bind(final_status.as_str())
        .bind(step.as_str())
        .bind(error)
        .execute(&mut *tx)
        .await?;

        if final_status == ProcessingStatus::Failed {
            sqlx::query(
                r#"
                UPDATE jobs
                SET status = $2,
                    step = $3,
                    updated_at = NOW(),
                    completed_at = NOW(),
                    last_error = $4
                WHERE id = $1
                "#,
            )
            .bind(job.id)
            .bind(final_status.as_str())
            .bind(step.as_str())
            .bind(error)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                r#"
                UPDATE jobs
                SET status = $2,
                    step = $3,
                    updated_at = NOW(),
                    next_attempt_at = NOW() + ($4 * interval '1 second'),
                    last_error = $5
                WHERE id = $1
                "#,
            )
            .bind(job.id)
            .bind(final_status.as_str())
            .bind(step.as_str())
            .bind(retry_backoff.as_secs() as i64)
            .bind(error)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn search_recordings(
        &self,
        query: &str,
        query_embedding: Option<&[f32]>,
        limit: i64,
    ) -> Result<Vec<SearchResult>> {
        let embedding_available = self.embedding_table_exists().await?;
        let use_embedding = embedding_available && query_embedding.is_some();

        let rows = if use_embedding {
            let vector = Vector::from(query_embedding.unwrap().to_vec());
            sqlx::query_as::<_, SearchRow>(
                r#"
                SELECT
                    r.id,
                    r.original_filename,
                    r.status,
                    r.current_step,
                    NULLIF(LEFT(COALESCE(r.summary_canonical_text, ''), 280), '') AS summary_excerpt,
                    NULLIF(LEFT(COALESCE(r.transcript, ''), 280), '') AS transcript_excerpt,
                    (r.search_document @@ plainto_tsquery('simple', $1) OR r.original_filename ILIKE '%' || $1 || '%') AS keyword_hit,
                    COALESCE(ts_rank(r.search_document, plainto_tsquery('simple', $1)), 0)::REAL AS keyword_score,
                    CASE
                        WHEN e.embedding IS NULL THEN NULL
                        ELSE (1 - (e.embedding <=> $2))::REAL
                    END AS similarity_score,
                    r.created_at
                FROM recordings r
                LEFT JOIN recording_embeddings e ON e.recording_id = r.id
                WHERE (r.search_document @@ plainto_tsquery('simple', $1) OR r.original_filename ILIKE '%' || $1 || '%')
                   OR e.embedding IS NOT NULL
                ORDER BY keyword_hit DESC, keyword_score DESC, similarity_score DESC NULLS LAST, r.created_at DESC
                LIMIT $3
                "#,
            )
            .bind(query)
            .bind(vector)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, SearchRow>(
                r#"
                SELECT
                    r.id,
                    r.original_filename,
                    r.status,
                    r.current_step,
                    NULLIF(LEFT(COALESCE(r.summary_canonical_text, ''), 280), '') AS summary_excerpt,
                    NULLIF(LEFT(COALESCE(r.transcript, ''), 280), '') AS transcript_excerpt,
                    (r.search_document @@ plainto_tsquery('simple', $1) OR r.original_filename ILIKE '%' || $1 || '%') AS keyword_hit,
                    COALESCE(ts_rank(r.search_document, plainto_tsquery('simple', $1)), 0)::REAL AS keyword_score,
                    NULL::REAL AS similarity_score,
                    r.created_at
                FROM recordings r
                WHERE (r.search_document @@ plainto_tsquery('simple', $1) OR r.original_filename ILIKE '%' || $1 || '%')
                ORDER BY keyword_hit DESC, keyword_score DESC, r.created_at DESC
                LIMIT $2
                "#,
            )
            .bind(query)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        rows.into_iter().map(TryInto::try_into).collect()
    }
}

#[derive(Debug, FromRow)]
struct RecordingRow {
    id: Uuid,
    original_filename: String,
    original_content_type: Option<String>,
    file_size_bytes: i64,
    original_rel_path: String,
    wav_rel_path: Option<String>,
    language: Option<String>,
    transcript: Option<String>,
    summary: Option<Json<StructuredSummary>>,
    summary_canonical_text: Option<String>,
    status: String,
    current_step: String,
    last_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<RecordingRow> for Recording {
    type Error = anyhow::Error;

    fn try_from(value: RecordingRow) -> Result<Self> {
        Ok(Self {
            id: value.id,
            original_filename: value.original_filename,
            original_content_type: value.original_content_type,
            file_size_bytes: value.file_size_bytes,
            original_rel_path: value.original_rel_path,
            wav_rel_path: value.wav_rel_path,
            language: value.language,
            transcript: value.transcript,
            summary: value.summary.map(|value| value.0),
            summary_canonical_text: value.summary_canonical_text,
            status: value.status.parse()?,
            current_step: value.current_step.parse()?,
            last_error: value.last_error,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

#[derive(Debug, FromRow)]
struct JobRow {
    id: Uuid,
    recording_id: Uuid,
    status: String,
    step: String,
    attempt_count: i32,
    last_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    next_attempt_at: DateTime<Utc>,
}

impl TryFrom<JobRow> for Job {
    type Error = anyhow::Error;

    fn try_from(value: JobRow) -> Result<Self> {
        Ok(Self {
            id: value.id,
            recording_id: value.recording_id,
            status: value.status.parse()?,
            step: value.step.parse()?,
            attempt_count: value.attempt_count,
            last_error: value.last_error,
            created_at: value.created_at,
            updated_at: value.updated_at,
            started_at: value.started_at,
            completed_at: value.completed_at,
            next_attempt_at: value.next_attempt_at,
        })
    }
}

#[derive(Debug, FromRow)]
struct RecordingListRow {
    id: Uuid,
    original_filename: String,
    status: String,
    current_step: String,
    language: Option<String>,
    has_transcript: bool,
    has_summary: bool,
    last_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<RecordingListRow> for RecordingListItem {
    type Error = anyhow::Error;

    fn try_from(value: RecordingListRow) -> Result<Self> {
        Ok(Self {
            id: value.id,
            original_filename: value.original_filename,
            status: value.status.parse()?,
            current_step: value.current_step.parse()?,
            language: value.language,
            has_transcript: value.has_transcript,
            has_summary: value.has_summary,
            last_error: value.last_error,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

#[derive(Debug, FromRow)]
struct SearchRow {
    id: Uuid,
    original_filename: String,
    status: String,
    current_step: String,
    summary_excerpt: Option<String>,
    transcript_excerpt: Option<String>,
    keyword_hit: bool,
    keyword_score: f32,
    similarity_score: Option<f32>,
    created_at: DateTime<Utc>,
}

impl TryFrom<SearchRow> for SearchResult {
    type Error = anyhow::Error;

    fn try_from(value: SearchRow) -> Result<Self> {
        Ok(Self {
            id: value.id,
            original_filename: value.original_filename,
            status: value.status.parse()?,
            current_step: value.current_step.parse()?,
            summary_excerpt: value.summary_excerpt,
            transcript_excerpt: value.transcript_excerpt,
            keyword_hit: value.keyword_hit,
            keyword_score: value.keyword_score,
            similarity_score: value.similarity_score,
            created_at: value.created_at,
        })
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

