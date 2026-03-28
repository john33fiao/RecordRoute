use super::*;
use ::postgres::{Client, NoTls};
use crate::storage::{
    AUDIO_CACHE_ROOT_ENV_VAR, AUDIO_ROOT_ENV_VAR, AUDIO_SPOOL_ROOT_ENV_VAR,
    METADATA_DRIVER_ENV_VAR, METADATA_POSTGRES_URL_ENV_VAR, METADATA_SQLITE_PATH_ENV_VAR,
};
use crate::test_support::{
    env_lock, mark_job_completed_with_audio, seed_summary_with_one_line, seed_transcripts,
    test_job,
};
use rusqlite::Connection;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const STORAGE_ENV_KEYS: [&str; 6] = [
    METADATA_DRIVER_ENV_VAR,
    METADATA_SQLITE_PATH_ENV_VAR,
    METADATA_POSTGRES_URL_ENV_VAR,
    AUDIO_ROOT_ENV_VAR,
    AUDIO_CACHE_ROOT_ENV_VAR,
    AUDIO_SPOOL_ROOT_ENV_VAR,
];

#[derive(Debug, Clone)]
enum BackendCase {
    Sqlite,
    Postgres { url: String },
}

impl BackendCase {
    fn name(&self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
            Self::Postgres { .. } => "postgres",
        }
    }
}

struct StorageEnvGuard {
    snapshot: Vec<(&'static str, Option<OsString>)>,
}

impl StorageEnvGuard {
    fn capture() -> Self {
        Self {
            snapshot: STORAGE_ENV_KEYS
                .into_iter()
                .map(|key| (key, env::var_os(key)))
                .collect(),
        }
    }
}

impl Drop for StorageEnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.snapshot {
            match value {
                Some(value) => unsafe { env::set_var(key, value) },
                None => unsafe { env::remove_var(key) },
            }
        }
    }
}

#[test]
fn backend_parity_initializes_default_index_state() {
    run_backend_matrix("default-state", |case, _repo_root, store| {
        store
            .ensure_db_dir()
            .unwrap_or_else(|error| panic!("{} ensure db dir: {error}", case.name()));
        let index = store
            .read_index()
            .unwrap_or_else(|error| panic!("{} read index: {error}", case.name()));

        assert_eq!(
            index,
            IndexFile::empty(),
            "{} default index should match empty state",
            case.name()
        );
    });
}

#[test]
fn backend_parity_round_trips_index_metadata() {
    run_backend_matrix("index-metadata", |case, _repo_root, store| {
        let mut job = test_job(
            "job-metadata",
            "2026-01-01T00:00:00Z",
            "sources/job-metadata/source.wav",
            "hash-metadata",
            "input.wav",
        );
        job.mark_ffmpeg_running("2026-01-01T00:00:01Z".to_string())
            .unwrap_or_else(|error| panic!("{} mark ffmpeg running: {error}", case.name()));
        job.probe.channels = Some(2);
        job.probe.channel_layout = Some("stereo".to_string());
        job.enqueue_task(TaskType::Stt, "2026-01-01T00:00:02Z".to_string());
        job.upsert_running_task(TaskType::Summary, "2026-01-01T00:00:03Z".to_string());
        store
            .insert_job(job)
            .unwrap_or_else(|error| panic!("{} insert job: {error}", case.name()));

        let whisper = store
            .update_model_preparation(ModelKind::Whisper, |record| {
                record.mark_running("2026-01-01T00:00:04Z".to_string());
            })
            .unwrap_or_else(|error| panic!("{} update whisper preparation: {error}", case.name()));
        let llama = store
            .update_model_preparation(ModelKind::Llama, |record| {
                record.mark_completed("2026-01-01T00:00:05Z".to_string());
            })
            .unwrap_or_else(|error| panic!("{} update llama preparation: {error}", case.name()));
        let llama_embedding = store
            .update_llama_embedding_preparation(|record| {
                record.mark_failed(
                    "2026-01-01T00:00:06Z".to_string(),
                    "embedding bootstrap failed".to_string(),
                );
            })
            .unwrap_or_else(|error| {
                panic!("{} update llama embedding preparation: {error}", case.name())
            });

        let expected_queue = TaskQueueState {
            active_batch: Some(ActiveQueueBatch {
                category: QueueCategory::Stt,
                running: Some(QueueEntry {
                    job_id: "job-metadata".to_string(),
                    task_type: TaskType::Stt,
                    category: QueueCategory::Stt,
                    queued_at: "2026-01-01T00:00:02Z".to_string(),
                    payload: QueuePayload::Stt {
                        audio_files: vec!["mono_mix.wav".to_string()],
                        language: "ko".to_string(),
                        keywords: Vec::new(),
                    },
                }),
                entries: Vec::new(),
            }),
            pending_batches: vec![QueueBatch {
                category: QueueCategory::Llm,
                entries: vec![QueueEntry {
                    job_id: "job-metadata".to_string(),
                    task_type: TaskType::Summary,
                    category: QueueCategory::Llm,
                    queued_at: "2026-01-01T00:00:03Z".to_string(),
                    payload: QueuePayload::Summary {
                        force_regenerate: false,
                    },
                }],
            }],
            burst_limit: 5,
        };
        store
            .with_index_mut(|index| {
                index.task_queue = expected_queue.clone();
                Ok(())
            })
            .unwrap_or_else(|error| panic!("{} persist queue state: {error}", case.name()));

        let persisted = store
            .read_index()
            .unwrap_or_else(|error| panic!("{} read persisted index: {error}", case.name()));
        let persisted_job = persisted
            .jobs
            .iter()
            .find(|job| job.job_id == "job-metadata")
            .unwrap_or_else(|| panic!("{} persisted job missing", case.name()));

        assert_eq!(
            persisted_job.status,
            JobStatus::Running,
            "{} job status should round-trip",
            case.name()
        );
        assert_eq!(
            persisted_job.probe.channels,
            Some(2),
            "{} job probe should round-trip",
            case.name()
        );
        assert_eq!(
            persisted_job
                .task(TaskType::Ffmpeg)
                .map(|task| task.status),
            Some(TaskStatus::Running),
            "{} ffmpeg task should round-trip",
            case.name()
        );
        assert_eq!(
            persisted_job
                .task(TaskType::Summary)
                .map(|task| task.status),
            Some(TaskStatus::Running),
            "{} summary task should round-trip",
            case.name()
        );
        assert_eq!(
            persisted.model_preparations.whisper,
            whisper,
            "{} whisper preparation should round-trip",
            case.name()
        );
        assert_eq!(
            persisted.model_preparations.llama,
            llama,
            "{} llama preparation should round-trip",
            case.name()
        );
        assert_eq!(
            persisted.model_preparations.llama_embedding,
            llama_embedding,
            "{} llama embedding preparation should round-trip",
            case.name()
        );
        assert_eq!(
            persisted.task_queue,
            expected_queue,
            "{} queue state should round-trip",
            case.name()
        );
    });
}

#[test]
fn backend_parity_round_trips_side_tables() {
    run_backend_matrix("side-tables", |case, _repo_root, store| {
        store
            .insert_job(test_job(
                "job-sidecar",
                "2026-01-01T00:00:00Z",
                "sources/job-sidecar/source.wav",
                "hash-sidecar",
                "input.wav",
            ))
            .unwrap_or_else(|error| panic!("{} insert job: {error}", case.name()));

        let expected_artifacts = vec![
            AudioArtifactRecord {
                job_id: "job-sidecar".to_string(),
                logical_name: "channel_01.wav".to_string(),
                storage_key: "jobs/job-sidecar/channel_01.wav".to_string(),
            },
            AudioArtifactRecord {
                job_id: "job-sidecar".to_string(),
                logical_name: "mono_mix.wav".to_string(),
                storage_key: "jobs/job-sidecar/mono_mix.wav".to_string(),
            },
        ];
        for artifact in &expected_artifacts {
            store.upsert_audio_artifact(artifact).unwrap_or_else(|error| {
                panic!("{} upsert audio artifact {}: {error}", case.name(), artifact.logical_name)
            });
        }

        seed_transcripts(
            store,
            "job-sidecar",
            &[
                ("channel_01", "speaker A"),
                ("mono_mix", "speaker A and speaker B"),
            ],
        )
        .unwrap_or_else(|error| panic!("{} seed transcripts: {error}", case.name()));
        seed_summary_with_one_line(
            store,
            "job-sidecar",
            "## 요약\n- 합의 완료",
            Some("합의 완료"),
        )
            .unwrap_or_else(|error| panic!("{} seed summary: {error}", case.name()));

        assert_eq!(
            store
                .list_audio_artifacts("job-sidecar")
                .unwrap_or_else(|error| panic!("{} list audio artifacts: {error}", case.name())),
            expected_artifacts,
            "{} audio artifacts should round-trip",
            case.name()
        );

        let transcripts = store
            .list_transcripts("job-sidecar")
            .unwrap_or_else(|error| panic!("{} list transcripts: {error}", case.name()));
        assert_eq!(
            transcripts,
            vec![
                TranscriptRecord {
                    job_id: "job-sidecar".to_string(),
                    transcript_id: "channel_01".to_string(),
                    file_name: "channel_01.txt".to_string(),
                    text: "speaker A".to_string(),
                },
                TranscriptRecord {
                    job_id: "job-sidecar".to_string(),
                    transcript_id: "mono_mix".to_string(),
                    file_name: "mono_mix.txt".to_string(),
                    text: "speaker A and speaker B".to_string(),
                },
            ],
            "{} transcripts should round-trip",
            case.name()
        );
        assert_eq!(
            store
                .count_transcripts("job-sidecar")
                .unwrap_or_else(|error| panic!("{} count transcripts: {error}", case.name())),
            2,
            "{} transcript count should round-trip",
            case.name()
        );
        assert_eq!(
            store
                .find_transcript("job-sidecar", "mono_mix")
                .unwrap_or_else(|error| panic!("{} find transcript: {error}", case.name())),
            Some(TranscriptRecord {
                job_id: "job-sidecar".to_string(),
                transcript_id: "mono_mix".to_string(),
                file_name: "mono_mix.txt".to_string(),
                text: "speaker A and speaker B".to_string(),
            }),
            "{} transcript lookup should round-trip",
            case.name()
        );
        assert_eq!(
            store
                .get_summary("job-sidecar")
                .unwrap_or_else(|error| panic!("{} get summary: {error}", case.name())),
            Some(SummaryRecord {
                job_id: "job-sidecar".to_string(),
                file_name: "result.md".to_string(),
                text: "## 요약\n- 합의 완료".to_string(),
                one_line_summary: Some("합의 완료".to_string()),
            }),
            "{} summary should round-trip",
            case.name()
        );
    });
}

#[test]
fn backend_parity_migrates_existing_summary_schema() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _env_guard = StorageEnvGuard::capture();

    for case in backend_cases() {
        let repo_root = temp_workspace("summary-migration", case.name());
        configure_backend_env(&case);

        match &case {
            BackendCase::Sqlite => {
                let sqlite_path = repo_root.join("db/index.sqlite3");
                fs::create_dir_all(
                    sqlite_path
                        .parent()
                        .expect("sqlite path should have a parent directory"),
                )
                .expect("sqlite dir");
                let connection = Connection::open(&sqlite_path).expect("sqlite open");
                connection
                    .execute_batch(
                        "
                        CREATE TABLE summaries (
                            job_id TEXT PRIMARY KEY,
                            file_name TEXT NOT NULL,
                            text TEXT NOT NULL
                        );
                        ",
                    )
                    .expect("legacy sqlite summaries schema");
            }
            BackendCase::Postgres { url } => {
                let mut client = Client::connect(url, NoTls).expect("postgres connect");
                client
                    .batch_execute(
                        "
                        DROP TABLE IF EXISTS summary_embeddings, summaries, transcripts,
                            stt_dictionary_keywords, audio_artifacts, tasks, jobs,
                            model_preparations, queue_state CASCADE;
                        CREATE TABLE summaries (
                            job_id TEXT PRIMARY KEY,
                            file_name TEXT NOT NULL,
                            text TEXT NOT NULL
                        );
                        ",
                    )
                    .expect("legacy postgres summaries schema");
            }
        }

        let store = IndexStore::new(&repo_root);
        store
            .ensure_db_dir()
            .unwrap_or_else(|error| panic!("{} migrate legacy summary schema: {error}", case.name()));
        store
            .upsert_summary(&SummaryRecord {
                job_id: "job-migrated".to_string(),
                file_name: "result.md".to_string(),
                text: "migrated summary".to_string(),
                one_line_summary: Some("migrated alias".to_string()),
            })
            .unwrap_or_else(|error| panic!("{} upsert migrated summary: {error}", case.name()));

        assert_eq!(
            store
                .get_summary("job-migrated")
                .unwrap_or_else(|error| panic!("{} read migrated summary: {error}", case.name())),
            Some(SummaryRecord {
                job_id: "job-migrated".to_string(),
                file_name: "result.md".to_string(),
                text: "migrated summary".to_string(),
                one_line_summary: Some("migrated alias".to_string()),
            }),
            "{} migrated summary schema should support one_line_summary",
            case.name()
        );
    }
}

#[test]
fn backend_parity_commit_summary_embedding_success_updates_metadata_and_vector() {
    run_backend_matrix("embedding-commit", |case, _repo_root, store| {
        let mut job = test_job(
            "job-embedding",
            "2026-01-01T00:00:00Z",
            "sources/job-embedding/source.wav",
            "hash-embedding",
            "input.wav",
        );
        mark_job_completed_with_audio(
            store,
            &mut job,
            "2026-01-01T00:00:01Z",
            &["mono_mix.wav"],
        )
        .unwrap_or_else(|error| panic!("{} seed completed job: {error}", case.name()));
        job.enqueue_task(TaskType::Embedding, "2026-01-01T00:00:02Z".to_string());
        store
            .insert_job(job)
            .unwrap_or_else(|error| panic!("{} insert embedding job: {error}", case.name()));

        let record = SummaryEmbeddingVectorRecord {
            metadata: SummaryEmbeddingRecord {
                model_id: "test-embedding-model".to_string(),
                text_sha256: "embedding-sha".to_string(),
                dimension: 3,
                normalized: true,
                created_at: "2026-01-01T00:00:03Z".to_string(),
            },
            vector: vec![0.1, 0.2, 0.3],
        };

        let updated = store
            .commit_summary_embedding_success(
                "job-embedding",
                "2026-01-01T00:00:04Z".to_string(),
                &record,
            )
            .unwrap_or_else(|error| panic!("{} commit embedding success: {error}", case.name()));

        assert_eq!(
            updated.summary_embedding,
            Some(record.metadata.clone()),
            "{} job metadata should include embedding",
            case.name()
        );
        assert_eq!(
            updated.task(TaskType::Embedding).map(|task| task.status),
            Some(TaskStatus::Completed),
            "{} embedding task should complete",
            case.name()
        );
        assert_eq!(
            store
                .get_summary_embedding("job-embedding")
                .unwrap_or_else(|error| panic!("{} read embedding row: {error}", case.name())),
            Some(record.clone()),
            "{} embedding vector row should persist",
            case.name()
        );
        assert_eq!(
            store
                .find_job("job-embedding")
                .unwrap_or_else(|error| panic!("{} reload job: {error}", case.name()))
                .and_then(|job| job.summary_embedding),
            Some(record.metadata),
            "{} job reload should include embedding metadata",
            case.name()
        );
    });
}

fn run_backend_matrix(test_name: &str, mut run: impl FnMut(&BackendCase, &Path, &IndexStore)) {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _env_guard = StorageEnvGuard::capture();

    for case in backend_cases() {
        let repo_root = temp_workspace(test_name, case.name());
        configure_backend_env(&case);
        let store = IndexStore::new(&repo_root);
        store.ensure_db_dir().unwrap_or_else(|error| {
            panic!(
                "{} ensure db dir for parity test {test_name}: {error}",
                case.name()
            )
        });
        if let BackendCase::Postgres { url } = &case {
            reset_postgres_metadata(url).unwrap_or_else(|error| {
                panic!(
                    "{} reset metadata for parity test {test_name}: {error}",
                    case.name()
                )
            });
        }

        run(&case, &repo_root, &store);
    }
}

fn backend_cases() -> Vec<BackendCase> {
    let mut cases = vec![BackendCase::Sqlite];
    if let Some(url) = env::var(METADATA_POSTGRES_URL_ENV_VAR)
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        cases.push(BackendCase::Postgres { url });
    }
    cases
}

fn configure_backend_env(case: &BackendCase) {
    clear_storage_env();
    match case {
        BackendCase::Sqlite => unsafe {
            env::set_var(METADATA_DRIVER_ENV_VAR, "sqlite");
        },
        BackendCase::Postgres { url } => unsafe {
            env::set_var(METADATA_DRIVER_ENV_VAR, "postgres");
            env::set_var(METADATA_POSTGRES_URL_ENV_VAR, url);
        },
    }
}

fn clear_storage_env() {
    for key in STORAGE_ENV_KEYS {
        unsafe { env::remove_var(key) };
    }
}

fn reset_postgres_metadata(url: &str) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls)
        .map_err(|error| format!("failed to connect postgres parity fixture: {error}"))?;
    client
        .batch_execute(
            "
            TRUNCATE TABLE
                summary_embeddings,
                summaries,
                transcripts,
                stt_dictionary_keywords,
                audio_artifacts,
                tasks,
                jobs,
                model_preparations,
                queue_state;
            ",
        )
        .map_err(|error| format!("failed to reset postgres parity fixture tables: {error}"))
}

fn temp_workspace(test_name: &str, backend_name: &str) -> PathBuf {
    let path = env::temp_dir().join(format!(
        "recordroute-index-{test_name}-{backend_name}-{}",
        Uuid::now_v7()
    ));
    fs::create_dir_all(&path).expect("temp workspace");
    path
}
