use super::backend::{
    DICTIONARY_AUTO_DEMO_KEYWORDS, DICTIONARY_AUTO_DEMO_SEED_FLAG, MetadataBackend,
    SerializedJobRecord, deserialize_job_record, from_json, serialize_job_record, to_json,
};
use super::types::{
    AudioArtifactRecord, DictionaryKeywordSource, DictionaryKeywords, IndexFile, ModelKind,
    SummaryEmbeddingVectorRecord, SummaryRecord, TaskRecord, TranscriptRecord,
};
use rusqlite::{Connection, OptionalExtension, params};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) struct SqliteMetadataStore {
    db_path: PathBuf,
}

impl SqliteMetadataStore {
    pub(crate) fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    fn open(&self) -> Result<Connection, String> {
        ensure_parent_dir(&self.db_path)?;
        let mut connection = Connection::open(&self.db_path).map_err(|error| {
            format!("failed to open sqlite {}: {error}", self.db_path.display())
        })?;
        initialize_schema(&mut connection)?;
        Ok(connection)
    }
}

impl MetadataBackend for SqliteMetadataStore {
    fn ensure_initialized(&self) -> Result<(), String> {
        let _ = self.open()?;
        Ok(())
    }

    fn read_index(&self) -> Result<IndexFile, String> {
        let connection = self.open()?;
        let mut jobs_stmt = connection
            .prepare(
                "SELECT job_id, status_json, started_at, finished_at, source_ref, source_kind_json,
                        source_content_sha256, source_file_name, probe_json, split_strategy_json,
                        outputs_json, error_message, summary_embedding_json
                 FROM jobs
                 ORDER BY started_at ASC, job_id ASC",
            )
            .map_err(|error| format!("failed to prepare sqlite jobs read: {error}"))?;
        let job_rows = jobs_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, Option<String>>(12)?,
                ))
            })
            .map_err(|error| format!("failed to map sqlite jobs: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to read sqlite jobs: {error}"))?;
        let mut jobs = Vec::with_capacity(job_rows.len());
        for (
            job_id,
            status_json,
            started_at,
            finished_at,
            source_ref,
            source_kind_json,
            source_content_sha256,
            source_file_name,
            probe_json,
            split_strategy_json,
            outputs_json,
            error_message,
            summary_embedding_json,
        ) in job_rows
        {
            jobs.push(deserialize_job_record(SerializedJobRecord {
                job_id,
                status_json,
                started_at,
                finished_at,
                source_ref,
                source_kind_json,
                source_content_sha256,
                source_file_name,
                probe_json,
                split_strategy_json,
                outputs_json,
                error_message,
                summary_embedding_json,
            })?);
        }

        let mut tasks_stmt = connection
            .prepare("SELECT job_id, data_json FROM tasks ORDER BY rowid ASC")
            .map_err(|error| format!("failed to prepare sqlite tasks read: {error}"))?;
        let task_rows = tasks_stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| format!("failed to query sqlite tasks: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to collect sqlite tasks: {error}"))?;

        let mut jobs = jobs;
        for (job_id, task_json) in task_rows {
            let task: TaskRecord = from_json(&task_json)?;
            if let Some(job) = jobs.iter_mut().find(|job| job.job_id == job_id) {
                job.tasks.push(task);
            }
        }

        let mut model_stmt = connection
            .prepare("SELECT model, data_json FROM model_preparations")
            .map_err(|error| format!("failed to prepare sqlite model read: {error}"))?;
        let mut preparations = super::types::ModelPreparations::default();
        let model_rows = model_stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| format!("failed to query sqlite model preparations: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to collect sqlite model preparations: {error}"))?;
        for (model_name, raw) in model_rows {
            let record = from_json(&raw)?;
            match model_name.as_str() {
                "whisper" => preparations.whisper = record,
                "llama" => preparations.llama = record,
                "llama_embedding" => preparations.llama_embedding = record,
                _ => {}
            }
        }

        let queue_json = connection
            .query_row(
                "SELECT data_json FROM queue_state WHERE singleton_id = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("failed to read sqlite queue state: {error}"))?;
        let task_queue = queue_json
            .map(|raw| from_json(&raw))
            .transpose()?
            .unwrap_or_default();

        Ok(IndexFile {
            version: 4,
            model_preparations: preparations,
            task_queue,
            jobs,
        })
    }

    fn write_index(&self, index: &IndexFile) -> Result<(), String> {
        let mut connection = self.open()?;
        let tx = connection
            .transaction()
            .map_err(|error| format!("failed to start sqlite transaction: {error}"))?;
        write_index_transaction(&tx, index)?;
        tx.commit()
            .map_err(|error| format!("failed to commit sqlite index transaction: {error}"))
    }

    fn list_stt_dictionary_keywords(&self) -> Result<DictionaryKeywords, String> {
        let connection = self.open()?;
        let mut stmt = connection
            .prepare(
                "SELECT keyword, source
                 FROM stt_dictionary_keywords
                 ORDER BY keyword COLLATE NOCASE ASC, keyword ASC",
            )
            .map_err(|error| format!("failed to prepare sqlite dictionary read: {error}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| format!("failed to query sqlite dictionary keywords: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to collect sqlite dictionary keywords: {error}"))?;

        let mut keywords = DictionaryKeywords::default();
        for (keyword, source) in rows {
            match source.parse::<DictionaryKeywordSource>()? {
                DictionaryKeywordSource::User => keywords.user_keywords.push(keyword),
                DictionaryKeywordSource::Auto => keywords.auto_keywords.push(keyword),
            }
        }

        Ok(keywords)
    }

    fn upsert_stt_dictionary_keyword(
        &self,
        keyword: &str,
        source: DictionaryKeywordSource,
    ) -> Result<(), String> {
        let connection = self.open()?;
        match source {
            DictionaryKeywordSource::User => {
                connection
                    .execute(
                        "INSERT INTO stt_dictionary_keywords (keyword, source)
                         VALUES (?, ?)
                         ON CONFLICT(keyword)
                         DO UPDATE SET source = excluded.source
                         WHERE stt_dictionary_keywords.source = ?",
                        params![
                            keyword,
                            DictionaryKeywordSource::User.as_str(),
                            DictionaryKeywordSource::Auto.as_str()
                        ],
                    )
                    .map_err(|error| {
                        format!("failed to upsert sqlite dictionary keyword {keyword}: {error}")
                    })?;
            }
            DictionaryKeywordSource::Auto => {
                connection
                    .execute(
                        "INSERT INTO stt_dictionary_keywords (keyword, source)
                         VALUES (?, ?)
                         ON CONFLICT(keyword) DO NOTHING",
                        params![keyword, DictionaryKeywordSource::Auto.as_str()],
                    )
                    .map_err(|error| {
                        format!("failed to upsert sqlite dictionary keyword {keyword}: {error}")
                    })?;
            }
        }
        Ok(())
    }

    fn delete_stt_dictionary_keyword(
        &self,
        keyword: &str,
        source: DictionaryKeywordSource,
    ) -> Result<bool, String> {
        let connection = self.open()?;
        let deleted = connection
            .execute(
                "DELETE FROM stt_dictionary_keywords WHERE keyword = ? AND source = ?",
                params![keyword, source.as_str()],
            )
            .map_err(|error| {
                format!("failed to delete sqlite dictionary keyword {keyword}: {error}")
            })?;
        Ok(deleted > 0)
    }

    fn promote_stt_dictionary_keyword(&self, keyword: &str) -> Result<bool, String> {
        let connection = self.open()?;
        let updated = connection
            .execute(
                "UPDATE stt_dictionary_keywords
                 SET source = ?
                 WHERE keyword = ? AND source = ?",
                params![
                    DictionaryKeywordSource::User.as_str(),
                    keyword,
                    DictionaryKeywordSource::Auto.as_str()
                ],
            )
            .map_err(|error| {
                format!("failed to promote sqlite dictionary keyword {keyword}: {error}")
            })?;
        Ok(updated > 0)
    }

    fn list_audio_artifacts(&self, job_id: &str) -> Result<Vec<AudioArtifactRecord>, String> {
        let connection = self.open()?;
        let mut stmt = connection
            .prepare(
                "SELECT job_id, logical_name, storage_key
                 FROM audio_artifacts
                 WHERE job_id = ?
                 ORDER BY logical_name ASC",
            )
            .map_err(|error| format!("failed to prepare sqlite audio artifacts read: {error}"))?;
        stmt.query_map([job_id], |row| {
            Ok(AudioArtifactRecord {
                job_id: row.get(0)?,
                logical_name: row.get(1)?,
                storage_key: row.get(2)?,
            })
        })
        .map_err(|error| format!("failed to query sqlite audio artifacts: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect sqlite audio artifacts: {error}"))
    }

    fn upsert_audio_artifact(&self, record: &AudioArtifactRecord) -> Result<(), String> {
        let connection = self.open()?;
        connection
            .execute(
                "INSERT INTO audio_artifacts (job_id, logical_name, storage_key)
                 VALUES (?, ?, ?)
                 ON CONFLICT(job_id, logical_name)
                 DO UPDATE SET storage_key = excluded.storage_key",
                params![record.job_id, record.logical_name, record.storage_key],
            )
            .map_err(|error| {
                format!(
                    "failed to upsert sqlite audio artifact {}: {error}",
                    record.logical_name
                )
            })?;
        Ok(())
    }

    fn list_transcripts(&self, job_id: &str) -> Result<Vec<TranscriptRecord>, String> {
        let connection = self.open()?;
        let mut stmt = connection
            .prepare(
                "SELECT job_id, transcript_id, file_name, text
                 FROM transcripts
                 WHERE job_id = ?
                 ORDER BY file_name ASC",
            )
            .map_err(|error| format!("failed to prepare sqlite transcript read: {error}"))?;
        stmt.query_map([job_id], |row| {
            Ok(TranscriptRecord {
                job_id: row.get(0)?,
                transcript_id: row.get(1)?,
                file_name: row.get(2)?,
                text: row.get(3)?,
            })
        })
        .map_err(|error| format!("failed to query sqlite transcripts: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect sqlite transcripts: {error}"))
    }

    fn get_transcript(
        &self,
        job_id: &str,
        transcript_id: &str,
    ) -> Result<Option<TranscriptRecord>, String> {
        let connection = self.open()?;
        connection
            .query_row(
                "SELECT job_id, transcript_id, file_name, text
                 FROM transcripts
                 WHERE job_id = ? AND transcript_id = ?",
                params![job_id, transcript_id],
                |row| {
                    Ok(TranscriptRecord {
                        job_id: row.get(0)?,
                        transcript_id: row.get(1)?,
                        file_name: row.get(2)?,
                        text: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(|error| format!("failed to read sqlite transcript: {error}"))
    }

    fn upsert_transcript(&self, record: &TranscriptRecord) -> Result<(), String> {
        let connection = self.open()?;
        connection
            .execute(
                "INSERT INTO transcripts (job_id, transcript_id, file_name, text)
                 VALUES (?, ?, ?, ?)
                 ON CONFLICT(job_id, transcript_id)
                 DO UPDATE SET file_name = excluded.file_name, text = excluded.text",
                params![
                    record.job_id,
                    record.transcript_id,
                    record.file_name,
                    record.text
                ],
            )
            .map_err(|error| {
                format!(
                    "failed to upsert sqlite transcript {}: {error}",
                    record.transcript_id
                )
            })?;
        Ok(())
    }

    fn count_transcripts(&self, job_id: &str) -> Result<usize, String> {
        let connection = self.open()?;
        let count = connection
            .query_row(
                "SELECT COUNT(*) FROM transcripts WHERE job_id = ?",
                [job_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| format!("failed to count sqlite transcripts: {error}"))?;
        Ok(count.max(0) as usize)
    }

    fn get_summary(&self, job_id: &str) -> Result<Option<SummaryRecord>, String> {
        let connection = self.open()?;
        connection
            .query_row(
                "SELECT job_id, file_name, text, one_line_summary FROM summaries WHERE job_id = ?",
                [job_id],
                |row| {
                    Ok(SummaryRecord {
                        job_id: row.get(0)?,
                        file_name: row.get(1)?,
                        text: row.get(2)?,
                        one_line_summary: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(|error| format!("failed to read sqlite summary: {error}"))
    }

    fn upsert_summary(&self, record: &SummaryRecord) -> Result<(), String> {
        let connection = self.open()?;
        connection
            .execute(
                "INSERT INTO summaries (job_id, file_name, text, one_line_summary)
                 VALUES (?, ?, ?, ?)
                 ON CONFLICT(job_id)
                 DO UPDATE SET
                    file_name = excluded.file_name,
                    text = excluded.text,
                    one_line_summary = excluded.one_line_summary",
                params![
                    record.job_id,
                    record.file_name,
                    record.text,
                    record.one_line_summary
                ],
            )
            .map_err(|error| {
                format!(
                    "failed to upsert sqlite summary for job {}: {error}",
                    record.job_id
                )
            })?;
        Ok(())
    }

    fn get_summary_embedding(
        &self,
        job_id: &str,
    ) -> Result<Option<SummaryEmbeddingVectorRecord>, String> {
        let connection = self.open()?;
        let row = connection
            .query_row(
                "SELECT metadata_json, vector_json FROM summary_embeddings WHERE job_id = ?",
                [job_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| format!("failed to read sqlite summary embedding: {error}"))?;
        row.map(|(metadata_json, vector_json)| {
            Ok(SummaryEmbeddingVectorRecord {
                metadata: from_json(&metadata_json)?,
                vector: from_json(&vector_json)?,
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
        let mut connection = self.open()?;
        let tx = connection
            .transaction()
            .map_err(|error| format!("failed to start sqlite transaction: {error}"))?;
        write_index_transaction(&tx, index)?;
        upsert_summary_embedding_transaction(&tx, job_id, record)?;
        tx.commit()
            .map_err(|error| format!("failed to commit sqlite embedding transaction: {error}"))
    }
}

fn write_index_transaction(
    tx: &rusqlite::Transaction<'_>,
    index: &IndexFile,
) -> Result<(), String> {
    tx.execute("DELETE FROM tasks", [])
        .map_err(|error| format!("failed to clear sqlite tasks: {error}"))?;
    tx.execute("DELETE FROM jobs", [])
        .map_err(|error| format!("failed to clear sqlite jobs: {error}"))?;
    tx.execute("DELETE FROM model_preparations", [])
        .map_err(|error| format!("failed to clear sqlite model preparations: {error}"))?;
    tx.execute("DELETE FROM queue_state", [])
        .map_err(|error| format!("failed to clear sqlite queue state: {error}"))?;

    for job in &index.jobs {
        let raw = serialize_job_record(job)?;
        tx.execute(
            "INSERT INTO jobs (
                job_id, status_json, started_at, finished_at, source_ref, source_kind_json,
                source_content_sha256, source_file_name, probe_json, split_strategy_json,
                outputs_json, error_message, summary_embedding_json
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                raw.job_id,
                raw.status_json,
                raw.started_at,
                raw.finished_at,
                raw.source_ref,
                raw.source_kind_json,
                raw.source_content_sha256,
                raw.source_file_name,
                raw.probe_json,
                raw.split_strategy_json,
                raw.outputs_json,
                raw.error_message,
                raw.summary_embedding_json,
            ],
        )
        .map_err(|error| format!("failed to insert sqlite job {}: {error}", job.job_id))?;

        for task in &job.tasks {
            tx.execute(
                "INSERT INTO tasks (task_id, job_id, task_type, data_json) VALUES (?, ?, ?, ?)",
                params![
                    task.task_id,
                    job.job_id,
                    to_json(&task.task_type)?,
                    to_json(task)?
                ],
            )
            .map_err(|error| {
                format!(
                    "failed to insert sqlite task {} for job {}: {error}",
                    task.task_id, job.job_id
                )
            })?;
        }
    }

    for (model_name, model_kind) in [
        ("whisper", ModelKind::Whisper),
        ("llama", ModelKind::Llama),
        ("llama_embedding", ModelKind::Llama),
    ] {
        let record = match model_name {
            "whisper" => &index.model_preparations.whisper,
            "llama" => &index.model_preparations.llama,
            _ => &index.model_preparations.llama_embedding,
        };
        let _ = model_kind;
        tx.execute(
            "INSERT INTO model_preparations (model, data_json) VALUES (?, ?)",
            params![model_name, to_json(record)?],
        )
        .map_err(|error| {
            format!("failed to insert sqlite model preparation {model_name}: {error}")
        })?;
    }

    tx.execute(
        "INSERT INTO queue_state (singleton_id, data_json) VALUES (1, ?)",
        params![to_json(&index.task_queue)?],
    )
    .map_err(|error| format!("failed to insert sqlite queue state: {error}"))?;
    Ok(())
}

fn upsert_summary_embedding_transaction(
    tx: &rusqlite::Transaction<'_>,
    job_id: &str,
    record: &SummaryEmbeddingVectorRecord,
) -> Result<(), String> {
    tx.execute(
        "INSERT INTO summary_embeddings (job_id, metadata_json, vector_json)
         VALUES (?, ?, ?)
         ON CONFLICT(job_id)
         DO UPDATE SET metadata_json = excluded.metadata_json, vector_json = excluded.vector_json",
        params![job_id, to_json(&record.metadata)?, to_json(&record.vector)?],
    )
    .map_err(|error| {
        format!("failed to upsert sqlite summary embedding for job {job_id}: {error}")
    })?;
    Ok(())
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!(
            "sqlite path has no parent directory: {}",
            path.display()
        ));
    };
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create sqlite directory {}: {error}",
            parent.display()
        )
    })
}

fn initialize_schema(connection: &mut Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS jobs (
                job_id TEXT PRIMARY KEY,
                status_json TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                source_ref TEXT NOT NULL,
                source_kind_json TEXT NOT NULL,
                source_content_sha256 TEXT NOT NULL,
                source_file_name TEXT NOT NULL,
                probe_json TEXT NOT NULL,
                split_strategy_json TEXT NOT NULL,
                outputs_json TEXT NOT NULL,
                error_message TEXT,
                summary_embedding_json TEXT
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
                singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
                data_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS stt_dictionary_keywords (
                keyword TEXT PRIMARY KEY,
                source TEXT NOT NULL DEFAULT 'user'
            );
            CREATE TABLE IF NOT EXISTS metadata_bootstrap_flags (
                flag TEXT PRIMARY KEY
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
                text TEXT NOT NULL,
                one_line_summary TEXT NULL
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
        .map_err(|error| format!("failed to initialize sqlite schema: {error}"))?;
    ensure_summary_one_line_column(connection)?;
    ensure_dictionary_keyword_source_column(connection)?;
    ensure_dictionary_auto_keywords_seeded(connection)
}

fn ensure_summary_one_line_column(connection: &Connection) -> Result<(), String> {
    let mut stmt = connection
        .prepare("PRAGMA table_info(summaries)")
        .map_err(|error| format!("failed to inspect sqlite summaries schema: {error}"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("failed to query sqlite summaries schema: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect sqlite summaries schema: {error}"))?;
    if columns.iter().any(|column| column == "one_line_summary") {
        return Ok(());
    }

    connection
        .execute(
            "ALTER TABLE summaries ADD COLUMN one_line_summary TEXT NULL",
            [],
        )
        .map_err(|error| format!("failed to migrate sqlite summaries schema: {error}"))?;
    Ok(())
}

fn ensure_dictionary_keyword_source_column(connection: &Connection) -> Result<(), String> {
    let mut stmt = connection
        .prepare("PRAGMA table_info(stt_dictionary_keywords)")
        .map_err(|error| format!("failed to inspect sqlite dictionary schema: {error}"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("failed to query sqlite dictionary schema: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect sqlite dictionary schema: {error}"))?;
    if !columns.iter().any(|column| column == "source") {
        connection
            .execute(
                "ALTER TABLE stt_dictionary_keywords
                 ADD COLUMN source TEXT NOT NULL DEFAULT 'user'",
                [],
            )
            .map_err(|error| format!("failed to migrate sqlite dictionary schema: {error}"))?;
    }

    connection
        .execute(
            "UPDATE stt_dictionary_keywords
             SET source = 'user'
             WHERE source IS NULL OR source = ''",
            [],
        )
        .map_err(|error| format!("failed to backfill sqlite dictionary sources: {error}"))?;
    Ok(())
}

fn ensure_dictionary_auto_keywords_seeded(connection: &mut Connection) -> Result<(), String> {
    let seeded = connection
        .query_row(
            "SELECT flag
             FROM metadata_bootstrap_flags
             WHERE flag = ?",
            [DICTIONARY_AUTO_DEMO_SEED_FLAG],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("failed to read sqlite dictionary bootstrap flag: {error}"))?
        .is_some();
    if seeded {
        return Ok(());
    }

    let tx = connection
        .transaction()
        .map_err(|error| format!("failed to start sqlite dictionary seed transaction: {error}"))?;
    for keyword in DICTIONARY_AUTO_DEMO_KEYWORDS {
        tx.execute(
            "INSERT INTO stt_dictionary_keywords (keyword, source)
             VALUES (?, ?)
             ON CONFLICT(keyword) DO NOTHING",
            params![keyword, DictionaryKeywordSource::Auto.as_str()],
        )
        .map_err(|error| {
            format!("failed to seed sqlite auto dictionary keyword {keyword}: {error}")
        })?;
    }
    tx.execute(
        "INSERT INTO metadata_bootstrap_flags (flag)
         VALUES (?)
         ON CONFLICT(flag) DO NOTHING",
        [DICTIONARY_AUTO_DEMO_SEED_FLAG],
    )
    .map_err(|error| format!("failed to persist sqlite dictionary bootstrap flag: {error}"))?;
    tx.commit()
        .map_err(|error| format!("failed to commit sqlite dictionary seed transaction: {error}"))
}
