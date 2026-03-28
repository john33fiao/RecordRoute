use super::backend::{
    MetadataBackend, SerializedJobRecord, deserialize_job_record, from_json, serialize_job_record,
    to_json,
};
use super::types::{
    AudioArtifactRecord, IndexFile, SummaryEmbeddingVectorRecord, SummaryRecord, TaskRecord,
    TranscriptRecord,
};
use postgres::{Client, NoTls};

pub(crate) struct PostgresMetadataStore {
    url: String,
}

impl PostgresMetadataStore {
    pub(crate) fn new(url: String) -> Self {
        Self { url }
    }

    fn connect(&self) -> Result<Client, String> {
        let mut client = Client::connect(&self.url, NoTls)
            .map_err(|error| format!("failed to connect postgres metadata store: {error}"))?;
        initialize_schema(&mut client)?;
        Ok(client)
    }
}

impl MetadataBackend for PostgresMetadataStore {
    fn ensure_initialized(&self) -> Result<(), String> {
        let _ = self.connect()?;
        Ok(())
    }

    fn read_index(&self) -> Result<IndexFile, String> {
        let mut client = self.connect()?;
        let rows = client
            .query(
                "SELECT job_id, status_json, started_at, finished_at, source_ref, source_kind_json,
                        source_content_sha256, source_file_name, probe_json, split_strategy_json,
                        outputs_json, error_message, summary_embedding_json
                 FROM jobs
                 ORDER BY started_at ASC, job_id ASC",
                &[],
            )
            .map_err(|error| format!("failed to read postgres jobs: {error}"))?;
        let mut jobs = rows
            .into_iter()
            .map(|row| {
                deserialize_job_record(SerializedJobRecord {
                    job_id: row.get(0),
                    status_json: row.get(1),
                    started_at: row.get(2),
                    finished_at: row.get(3),
                    source_ref: row.get(4),
                    source_kind_json: row.get(5),
                    source_content_sha256: row.get(6),
                    source_file_name: row.get(7),
                    probe_json: row.get(8),
                    split_strategy_json: row.get(9),
                    outputs_json: row.get(10),
                    error_message: row.get(11),
                    summary_embedding_json: row.get(12),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        for row in client
            .query(
                "SELECT job_id, data_json FROM tasks ORDER BY task_id ASC",
                &[],
            )
            .map_err(|error| format!("failed to read postgres tasks: {error}"))?
        {
            let job_id: String = row.get(0);
            let task: TaskRecord = from_json(&row.get::<_, String>(1))?;
            if let Some(job) = jobs.iter_mut().find(|job| job.job_id == job_id) {
                job.tasks.push(task);
            }
        }

        let mut preparations = super::types::ModelPreparations::default();
        for row in client
            .query("SELECT model, data_json FROM model_preparations", &[])
            .map_err(|error| format!("failed to read postgres model preparations: {error}"))?
        {
            let model_name: String = row.get(0);
            let record = from_json(&row.get::<_, String>(1))?;
            match model_name.as_str() {
                "whisper" => preparations.whisper = record,
                "llama" => preparations.llama = record,
                "llama_embedding" => preparations.llama_embedding = record,
                _ => {}
            }
        }

        let queue_state = client
            .query_opt(
                "SELECT data_json FROM queue_state WHERE singleton_id = 1",
                &[],
            )
            .map_err(|error| format!("failed to read postgres queue state: {error}"))?
            .map(|row| from_json(&row.get::<_, String>(0)))
            .transpose()?
            .unwrap_or_default();

        Ok(IndexFile {
            version: 4,
            model_preparations: preparations,
            task_queue: queue_state,
            jobs,
        })
    }

    fn write_index(&self, index: &IndexFile) -> Result<(), String> {
        let mut client = self.connect()?;
        let mut tx = client
            .transaction()
            .map_err(|error| format!("failed to start postgres transaction: {error}"))?;
        write_index_transaction(&mut tx, index)?;
        tx.commit()
            .map_err(|error| format!("failed to commit postgres transaction: {error}"))
    }

    fn list_stt_dictionary_keywords(&self) -> Result<Vec<String>, String> {
        let mut client = self.connect()?;
        client
            .query(
                "SELECT keyword
                 FROM stt_dictionary_keywords
                 ORDER BY LOWER(keyword) ASC, keyword ASC",
                &[],
            )
            .map_err(|error| format!("failed to read postgres dictionary keywords: {error}"))?
            .into_iter()
            .map(|row| Ok(row.get(0)))
            .collect()
    }

    fn upsert_stt_dictionary_keyword(&self, keyword: &str) -> Result<(), String> {
        let mut client = self.connect()?;
        client
            .execute(
                "INSERT INTO stt_dictionary_keywords (keyword)
                 VALUES ($1)
                 ON CONFLICT (keyword) DO NOTHING",
                &[&keyword],
            )
            .map_err(|error| {
                format!("failed to upsert postgres dictionary keyword {keyword}: {error}")
            })?;
        Ok(())
    }

    fn delete_stt_dictionary_keyword(&self, keyword: &str) -> Result<bool, String> {
        let mut client = self.connect()?;
        let deleted = client
            .execute(
                "DELETE FROM stt_dictionary_keywords WHERE keyword = $1",
                &[&keyword],
            )
            .map_err(|error| {
                format!("failed to delete postgres dictionary keyword {keyword}: {error}")
            })?;
        Ok(deleted > 0)
    }

    fn list_audio_artifacts(&self, job_id: &str) -> Result<Vec<AudioArtifactRecord>, String> {
        let mut client = self.connect()?;
        client
            .query(
                "SELECT job_id, logical_name, storage_key
                 FROM audio_artifacts
                 WHERE job_id = $1
                 ORDER BY logical_name ASC",
                &[&job_id],
            )
            .map_err(|error| format!("failed to read postgres audio artifacts: {error}"))?
            .into_iter()
            .map(|row| {
                Ok(AudioArtifactRecord {
                    job_id: row.get(0),
                    logical_name: row.get(1),
                    storage_key: row.get(2),
                })
            })
            .collect()
    }

    fn upsert_audio_artifact(&self, record: &AudioArtifactRecord) -> Result<(), String> {
        let mut client = self.connect()?;
        client
            .execute(
                "INSERT INTO audio_artifacts (job_id, logical_name, storage_key)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (job_id, logical_name)
                 DO UPDATE SET storage_key = EXCLUDED.storage_key",
                &[&record.job_id, &record.logical_name, &record.storage_key],
            )
            .map_err(|error| {
                format!(
                    "failed to upsert postgres audio artifact {}: {error}",
                    record.logical_name
                )
            })?;
        Ok(())
    }

    fn list_transcripts(&self, job_id: &str) -> Result<Vec<TranscriptRecord>, String> {
        let mut client = self.connect()?;
        client
            .query(
                "SELECT job_id, transcript_id, file_name, text
                 FROM transcripts
                 WHERE job_id = $1
                 ORDER BY file_name ASC",
                &[&job_id],
            )
            .map_err(|error| format!("failed to read postgres transcripts: {error}"))?
            .into_iter()
            .map(|row| {
                Ok(TranscriptRecord {
                    job_id: row.get(0),
                    transcript_id: row.get(1),
                    file_name: row.get(2),
                    text: row.get(3),
                })
            })
            .collect()
    }

    fn get_transcript(
        &self,
        job_id: &str,
        transcript_id: &str,
    ) -> Result<Option<TranscriptRecord>, String> {
        let mut client = self.connect()?;
        client
            .query_opt(
                "SELECT job_id, transcript_id, file_name, text
                 FROM transcripts
                 WHERE job_id = $1 AND transcript_id = $2",
                &[&job_id, &transcript_id],
            )
            .map_err(|error| format!("failed to read postgres transcript: {error}"))?
            .map(|row| {
                Ok(TranscriptRecord {
                    job_id: row.get(0),
                    transcript_id: row.get(1),
                    file_name: row.get(2),
                    text: row.get(3),
                })
            })
            .transpose()
    }

    fn upsert_transcript(&self, record: &TranscriptRecord) -> Result<(), String> {
        let mut client = self.connect()?;
        client
            .execute(
                "INSERT INTO transcripts (job_id, transcript_id, file_name, text)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (job_id, transcript_id)
                 DO UPDATE SET file_name = EXCLUDED.file_name, text = EXCLUDED.text",
                &[
                    &record.job_id,
                    &record.transcript_id,
                    &record.file_name,
                    &record.text,
                ],
            )
            .map_err(|error| {
                format!(
                    "failed to upsert postgres transcript {}: {error}",
                    record.transcript_id
                )
            })?;
        Ok(())
    }

    fn count_transcripts(&self, job_id: &str) -> Result<usize, String> {
        let mut client = self.connect()?;
        let count: i64 = client
            .query_one(
                "SELECT COUNT(*) FROM transcripts WHERE job_id = $1",
                &[&job_id],
            )
            .map_err(|error| format!("failed to count postgres transcripts: {error}"))?
            .get(0);
        Ok(count.max(0) as usize)
    }

    fn get_summary(&self, job_id: &str) -> Result<Option<SummaryRecord>, String> {
        let mut client = self.connect()?;
        client
            .query_opt(
                "SELECT job_id, file_name, text FROM summaries WHERE job_id = $1",
                &[&job_id],
            )
            .map_err(|error| format!("failed to read postgres summary: {error}"))?
            .map(|row| {
                Ok(SummaryRecord {
                    job_id: row.get(0),
                    file_name: row.get(1),
                    text: row.get(2),
                })
            })
            .transpose()
    }

    fn upsert_summary(&self, record: &SummaryRecord) -> Result<(), String> {
        let mut client = self.connect()?;
        client
            .execute(
                "INSERT INTO summaries (job_id, file_name, text)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (job_id)
                 DO UPDATE SET file_name = EXCLUDED.file_name, text = EXCLUDED.text",
                &[&record.job_id, &record.file_name, &record.text],
            )
            .map_err(|error| {
                format!(
                    "failed to upsert postgres summary for job {}: {error}",
                    record.job_id
                )
            })?;
        Ok(())
    }

    fn get_summary_embedding(
        &self,
        job_id: &str,
    ) -> Result<Option<SummaryEmbeddingVectorRecord>, String> {
        let mut client = self.connect()?;
        client
            .query_opt(
                "SELECT metadata_json, vector_json FROM summary_embeddings WHERE job_id = $1",
                &[&job_id],
            )
            .map_err(|error| format!("failed to read postgres summary embedding: {error}"))?
            .map(|row| {
                Ok(SummaryEmbeddingVectorRecord {
                    metadata: from_json(&row.get::<_, String>(0))?,
                    vector: from_json(&row.get::<_, String>(1))?,
                })
            })
            .transpose()
    }

    fn write_index_with_summary_embedding(
        &self,
        index: &IndexFile,
        job_id: &str,
        record: &SummaryEmbeddingVectorRecord,
    ) -> Result<(), String> {
        let mut client = self.connect()?;
        let mut tx = client
            .transaction()
            .map_err(|error| format!("failed to start postgres transaction: {error}"))?;
        write_index_transaction(&mut tx, index)?;
        upsert_summary_embedding_transaction(&mut tx, job_id, record)?;
        tx.commit()
            .map_err(|error| format!("failed to commit postgres embedding transaction: {error}"))
    }
}

fn write_index_transaction(
    tx: &mut postgres::Transaction<'_>,
    index: &IndexFile,
) -> Result<(), String> {
    for statement in [
        "DELETE FROM tasks",
        "DELETE FROM jobs",
        "DELETE FROM model_preparations",
        "DELETE FROM queue_state",
    ] {
        tx.execute(statement, &[])
            .map_err(|error| format!("failed to execute postgres reset '{statement}': {error}"))?;
    }

    for job in &index.jobs {
        let raw = serialize_job_record(job)?;
        tx.execute(
            "INSERT INTO jobs (
                job_id, status_json, started_at, finished_at, source_ref, source_kind_json,
                source_content_sha256, source_file_name, probe_json, split_strategy_json,
                outputs_json, error_message, summary_embedding_json
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
            &[
                &raw.job_id,
                &raw.status_json,
                &raw.started_at,
                &raw.finished_at,
                &raw.source_ref,
                &raw.source_kind_json,
                &raw.source_content_sha256,
                &raw.source_file_name,
                &raw.probe_json,
                &raw.split_strategy_json,
                &raw.outputs_json,
                &raw.error_message,
                &raw.summary_embedding_json,
            ],
        )
        .map_err(|error| format!("failed to insert postgres job {}: {error}", job.job_id))?;

        for task in &job.tasks {
            tx.execute(
                "INSERT INTO tasks (task_id, job_id, task_type, data_json)
                 VALUES ($1, $2, $3, $4)",
                &[
                    &task.task_id,
                    &job.job_id,
                    &to_json(&task.task_type)?,
                    &to_json(task)?,
                ],
            )
            .map_err(|error| {
                format!(
                    "failed to insert postgres task {} for job {}: {error}",
                    task.task_id, job.job_id
                )
            })?;
        }
    }

    for (model_name, record) in [
        ("whisper", &index.model_preparations.whisper),
        ("llama", &index.model_preparations.llama),
        ("llama_embedding", &index.model_preparations.llama_embedding),
    ] {
        tx.execute(
            "INSERT INTO model_preparations (model, data_json) VALUES ($1, $2)",
            &[&model_name, &to_json(record)?],
        )
        .map_err(|error| {
            format!("failed to insert postgres model preparation {model_name}: {error}")
        })?;
    }

    tx.execute(
        "INSERT INTO queue_state (singleton_id, data_json) VALUES (1, $1)",
        &[&to_json(&index.task_queue)?],
    )
    .map_err(|error| format!("failed to insert postgres queue state: {error}"))?;
    Ok(())
}

fn upsert_summary_embedding_transaction(
    tx: &mut postgres::Transaction<'_>,
    job_id: &str,
    record: &SummaryEmbeddingVectorRecord,
) -> Result<(), String> {
    tx.execute(
        "INSERT INTO summary_embeddings (job_id, metadata_json, vector_json)
         VALUES ($1, $2, $3)
         ON CONFLICT (job_id)
         DO UPDATE SET metadata_json = EXCLUDED.metadata_json, vector_json = EXCLUDED.vector_json",
        &[
            &job_id,
            &to_json(&record.metadata)?,
            &to_json(&record.vector)?,
        ],
    )
    .map_err(|error| {
        format!("failed to upsert postgres summary embedding for job {job_id}: {error}")
    })?;
    Ok(())
}

fn initialize_schema(client: &mut Client) -> Result<(), String> {
    client
        .batch_execute(
            "
            CREATE TABLE IF NOT EXISTS jobs (
                job_id TEXT PRIMARY KEY,
                status_json TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT NULL,
                source_ref TEXT NOT NULL,
                source_kind_json TEXT NOT NULL,
                source_content_sha256 TEXT NOT NULL,
                source_file_name TEXT NOT NULL,
                probe_json TEXT NOT NULL,
                split_strategy_json TEXT NOT NULL,
                outputs_json TEXT NOT NULL,
                error_message TEXT NULL,
                summary_embedding_json TEXT NULL
            );
            CREATE TABLE IF NOT EXISTS tasks (
                task_id TEXT PRIMARY KEY,
                job_id TEXT NOT NULL,
                task_type TEXT NOT NULL,
                data_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS model_preparations (
                model TEXT PRIMARY KEY,
                data_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS queue_state (
                singleton_id INTEGER PRIMARY KEY,
                data_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS stt_dictionary_keywords (
                keyword TEXT PRIMARY KEY
            );
            CREATE TABLE IF NOT EXISTS transcripts (
                job_id TEXT NOT NULL,
                transcript_id TEXT NOT NULL,
                file_name TEXT NOT NULL,
                text TEXT NOT NULL,
                PRIMARY KEY (job_id, transcript_id)
            );
            CREATE TABLE IF NOT EXISTS summaries (
                job_id TEXT PRIMARY KEY,
                file_name TEXT NOT NULL,
                text TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS summary_embeddings (
                job_id TEXT PRIMARY KEY,
                metadata_json TEXT NOT NULL,
                vector_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS audio_artifacts (
                job_id TEXT NOT NULL,
                logical_name TEXT NOT NULL,
                storage_key TEXT NOT NULL,
                PRIMARY KEY (job_id, logical_name)
            );
            ",
        )
        .map_err(|error| format!("failed to initialize postgres metadata schema: {error}"))
}
