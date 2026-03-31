#[cfg(test)]
use super::BatchProcessTarget;
use super::{
    BatchQueueSubmission, artifacts, embedding_stage, ffmpeg_stage, now_rfc3339, stt_stage,
    summary_stage,
};
use crate::index::{
    ActiveQueueBatch, IndexFile, IndexStore, JobRecord, QueueCategory, QueueEntry, TaskQueueState,
    TaskType,
};
use std::path::{Path, PathBuf};

#[path = "queue/entries.rs"]
mod entries;
#[path = "queue/executor.rs"]
mod executor;
#[path = "queue/planner.rs"]
mod planner;
#[path = "queue/scheduler.rs"]
mod scheduler;

#[cfg(test)]
pub use entries::build_stt_entry;
pub use entries::{
    build_embedding_entry, build_ffmpeg_entry, build_stt_entry_with_options, build_summary_entry,
    enqueue_entry, find_task_entry, find_ticket, update_queued_entry,
};
pub use executor::{dispatch_one, dispatch_until_task_terminal, recover_interrupted_active_entry};
pub use planner::submit_batch_pipeline_jobs;
#[cfg(test)]
use scheduler::{clear_running_entry_in_index, reserve_next_entry};

const QUEUE_CANCELLED_BY_USER_MESSAGE: &str = "cancelled by user";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueTicket {
    pub category: QueueCategory,
    pub position: usize,
    pub queued_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QueueCancelPendingResult {
    pub total_cancelled: usize,
    pub ffmpeg_cancelled: usize,
    pub stt_cancelled: usize,
    pub summary_cancelled: usize,
    pub embedding_cancelled: usize,
}

impl QueueCancelPendingResult {
    fn record_cancelled(&mut self, task_type: TaskType) {
        self.total_cancelled = self.total_cancelled.saturating_add(1);
        match task_type {
            TaskType::Ffmpeg => {
                self.ffmpeg_cancelled = self.ffmpeg_cancelled.saturating_add(1);
            }
            TaskType::Stt => {
                self.stt_cancelled = self.stt_cancelled.saturating_add(1);
            }
            TaskType::Summary => {
                self.summary_cancelled = self.summary_cancelled.saturating_add(1);
            }
            TaskType::Embedding => {
                self.embedding_cancelled = self.embedding_cancelled.saturating_add(1);
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DispatchState {
    last_category: Option<QueueCategory>,
    burst_count: u32,
}

impl DispatchState {
    fn record_execution(&mut self, category: QueueCategory) {
        if self.last_category == Some(category) {
            self.burst_count = self.burst_count.saturating_add(1);
        } else {
            self.last_category = Some(category);
            self.burst_count = 1;
        }
    }

    fn should_rotate(&self, category: QueueCategory, burst_limit: u32) -> bool {
        self.last_category == Some(category) && self.burst_count >= burst_limit
    }
}

#[derive(Debug, Clone)]
struct QueueWorkItem {
    entry: QueueEntry,
}

pub fn queue_snapshot(repo_root: &Path) -> Result<TaskQueueState, String> {
    IndexStore::new(repo_root).task_queue()
}

pub fn set_queue_paused(repo_root: &Path, paused: bool) -> Result<TaskQueueState, String> {
    IndexStore::new(repo_root).with_index_mut(|index| {
        index.task_queue.paused = paused;
        Ok(index.task_queue.clone())
    })
}

pub fn cancel_pending_entries(repo_root: &Path) -> Result<QueueCancelPendingResult, String> {
    let finished_at = now_rfc3339()?;
    IndexStore::new(repo_root).with_index_mut(|index| {
        let mut cancelled = QueueCancelPendingResult::default();
        let mut entries = Vec::new();

        if let Some(active_batch) = index.task_queue.active_batch.as_mut() {
            entries.append(&mut active_batch.entries);
        }
        for batch in &mut index.task_queue.pending_batches {
            entries.append(&mut batch.entries);
        }

        index
            .task_queue
            .pending_batches
            .retain(|batch| !batch.entries.is_empty());
        if index
            .task_queue
            .active_batch
            .as_ref()
            .is_some_and(|batch| batch.running.is_none() && batch.entries.is_empty())
        {
            index.task_queue.active_batch = None;
        }

        for entry in entries {
            cancel_entry(index, &entry, &finished_at)?;
            cancelled.record_cancelled(entry.task_type);
        }

        Ok(cancelled)
    })
}

fn cancel_entry(
    index: &mut IndexFile,
    entry: &QueueEntry,
    finished_at: &str,
) -> Result<(), String> {
    let job = index
        .jobs
        .iter_mut()
        .find(|job| job.job_id == entry.job_id)
        .ok_or_else(|| format!("job not found in index: {}", entry.job_id))?;

    match entry.task_type {
        TaskType::Ffmpeg => job.mark_failed(
            finished_at.to_string(),
            QUEUE_CANCELLED_BY_USER_MESSAGE.to_string(),
        )?,
        _ => job.fail_task(
            entry.task_type,
            finished_at.to_string(),
            QUEUE_CANCELLED_BY_USER_MESSAGE.to_string(),
        )?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{IndexStore, JobRecord, QueueBatch, QueuePayload, TaskStatus};
    use crate::test_support::{mark_job_completed_with_audio, seed_transcripts, test_job};
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn enqueue_merges_same_category_ahead_of_pending_batch() {
        let mut index = IndexFile::empty();
        index.task_queue.active_batch = Some(ActiveQueueBatch {
            category: QueueCategory::Ffmpeg,
            running: None,
            entries: vec![build_ffmpeg_entry(
                "job-1",
                Path::new("/tmp/job-1.wav"),
                "2026-01-01T00:00:00Z".to_string(),
            )],
        });
        index.task_queue.pending_batches.push(QueueBatch {
            category: QueueCategory::Stt,
            entries: vec![build_stt_entry(
                "job-2",
                &[PathBuf::from("mono_mix.wav")],
                "2026-01-01T00:00:01Z".to_string(),
            )],
        });

        let ticket = enqueue_entry(
            &mut index,
            build_ffmpeg_entry(
                "job-3",
                Path::new("/tmp/job-3.wav"),
                "2026-01-01T00:00:02Z".to_string(),
            ),
        );

        assert_eq!(ticket.category, QueueCategory::Ffmpeg);
        assert_eq!(ticket.position, 2);
        assert_eq!(
            index
                .task_queue
                .active_batch
                .as_ref()
                .expect("active batch")
                .entries
                .iter()
                .map(|entry| entry.job_id.as_str())
                .collect::<Vec<_>>(),
            vec!["job-1", "job-3"]
        );
        assert_eq!(
            index.task_queue.pending_batches[0]
                .entries
                .iter()
                .map(|entry| entry.job_id.as_str())
                .collect::<Vec<_>>(),
            vec!["job-2"]
        );
    }

    #[test]
    fn burst_rotation_moves_remaining_batch_behind_waiting_category() {
        let _guard = crate::test_support::env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _env_guard =
            crate::test_support::EnvVarGuard::capture(crate::index::QUEUE_BURST_LIMIT_ENV_VAR);
        unsafe { std::env::set_var(crate::index::QUEUE_BURST_LIMIT_ENV_VAR, "3") };
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);

        for job_id in ["job-1", "job-2", "job-3", "job-4"] {
            store
                .insert_job(JobRecord::new(
                    job_id.to_string(),
                    "2026-01-01T00:00:00Z".to_string(),
                    PathBuf::from(format!("/tmp/{job_id}.wav")),
                    store.job_dir(job_id),
                ))
                .expect("insert ffmpeg job");
        }

        let mut stt_job = JobRecord::new(
            "job-5".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/job-5.wav"),
            store.job_dir("job-5"),
        );
        stt_job
            .mark_completed(
                "2026-01-01T00:00:01Z".to_string(),
                crate::index::JobOutputs::default(),
            )
            .expect("mark stt job completed");
        stt_job.enqueue_task(TaskType::Stt, "2026-01-01T00:00:05Z".to_string());
        store.insert_job(stt_job).expect("insert stt job");

        store
            .with_index_mut(|index| {
                index.task_queue.active_batch = Some(ActiveQueueBatch {
                    category: QueueCategory::Ffmpeg,
                    running: None,
                    entries: vec![
                        build_ffmpeg_entry(
                            "job-1",
                            Path::new("/tmp/job-1.wav"),
                            "2026-01-01T00:00:01Z".to_string(),
                        ),
                        build_ffmpeg_entry(
                            "job-2",
                            Path::new("/tmp/job-2.wav"),
                            "2026-01-01T00:00:02Z".to_string(),
                        ),
                        build_ffmpeg_entry(
                            "job-3",
                            Path::new("/tmp/job-3.wav"),
                            "2026-01-01T00:00:03Z".to_string(),
                        ),
                        build_ffmpeg_entry(
                            "job-4",
                            Path::new("/tmp/job-4.wav"),
                            "2026-01-01T00:00:04Z".to_string(),
                        ),
                    ],
                });
                index.task_queue.pending_batches = vec![QueueBatch {
                    category: QueueCategory::Stt,
                    entries: vec![build_stt_entry(
                        "job-5",
                        &[PathBuf::from("mono_mix.wav")],
                        "2026-01-01T00:00:05Z".to_string(),
                    )],
                }];
                Ok(())
            })
            .expect("seed queue");

        let mut state = DispatchState::default();
        for expected_job_id in ["job-1", "job-2", "job-3"] {
            let work = reserve_next_entry(
                &repo_root,
                &mut state,
                "2026-01-01T00:00:10Z".to_string(),
                false,
            )
            .expect("reserve work")
            .expect("queued work");
            assert_eq!(work.entry.category, QueueCategory::Ffmpeg);
            assert_eq!(work.entry.job_id, expected_job_id);
            store
                .with_index_mut(|index| {
                    clear_running_entry_in_index(index, &work.entry);
                    Ok(())
                })
                .expect("clear running entry");
            state.record_execution(work.entry.category);
        }

        let rotated = reserve_next_entry(
            &repo_root,
            &mut state,
            "2026-01-01T00:00:11Z".to_string(),
            false,
        )
        .expect("reserve rotated work")
        .expect("rotated work");
        assert_eq!(rotated.entry.category, QueueCategory::Stt);
        assert_eq!(rotated.entry.job_id, "job-5");

        let snapshot = queue_snapshot(&repo_root).expect("queue snapshot");
        assert_eq!(
            snapshot
                .active_batch
                .as_ref()
                .expect("active batch")
                .category,
            QueueCategory::Stt
        );
        assert_eq!(
            snapshot
                .active_batch
                .as_ref()
                .and_then(|batch| batch.running.as_ref())
                .map(|entry| entry.job_id.as_str()),
            Some("job-5")
        );
        assert_eq!(snapshot.pending_batches.len(), 1);
        assert_eq!(snapshot.pending_batches[0].category, QueueCategory::Ffmpeg);
        assert_eq!(
            snapshot.pending_batches[0]
                .entries
                .iter()
                .map(|entry| entry.job_id.as_str())
                .collect::<Vec<_>>(),
            vec!["job-4"]
        );
    }

    #[test]
    fn recover_requeues_interrupted_active_entry_to_front_of_batch() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);

        let mut first_job = JobRecord::new(
            "job-1".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/job-1.wav"),
            store.job_dir("job-1"),
        );
        first_job.enqueue_task(TaskType::Summary, "2026-01-01T00:00:01Z".to_string());
        first_job
            .start_task(TaskType::Summary, "2026-01-01T00:00:02Z".to_string())
            .expect("start summary");
        store.insert_job(first_job).expect("insert first job");

        let mut second_job = JobRecord::new(
            "job-2".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/job-2.wav"),
            store.job_dir("job-2"),
        );
        second_job.enqueue_task(TaskType::Summary, "2026-01-01T00:00:03Z".to_string());
        store.insert_job(second_job).expect("insert second job");

        store
            .with_index_mut(|index| {
                index.task_queue.active_batch = Some(ActiveQueueBatch {
                    category: QueueCategory::Llm,
                    running: Some(build_summary_entry(
                        "job-1",
                        false,
                        "2026-01-01T00:00:01Z".to_string(),
                    )),
                    entries: vec![build_summary_entry(
                        "job-2",
                        false,
                        "2026-01-01T00:00:03Z".to_string(),
                    )],
                });
                Ok(())
            })
            .expect("seed running batch");

        recover_interrupted_active_entry(&repo_root).expect("recover queue");

        let snapshot = queue_snapshot(&repo_root).expect("queue snapshot");
        let active_batch = snapshot.active_batch.expect("active batch");
        assert!(active_batch.running.is_none());
        assert_eq!(
            active_batch
                .entries
                .iter()
                .map(|entry| entry.job_id.as_str())
                .collect::<Vec<_>>(),
            vec!["job-1", "job-2"]
        );

        let recovered_job = store
            .find_job("job-1")
            .expect("read recovered job")
            .expect("recovered job");
        let task = recovered_job
            .task(TaskType::Summary)
            .expect("summary task should exist");
        assert_eq!(task.status, TaskStatus::Queued);
        assert!(task.started_at.is_none());
    }

    #[test]
    fn reserve_next_entry_skips_blocked_summary_until_stt_completed() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);

        let mut blocked_job = JobRecord::new(
            "job-summary".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/job-summary.wav"),
            store.job_dir("job-summary"),
        );
        blocked_job
            .mark_completed(
                "2026-01-01T00:00:01Z".to_string(),
                crate::index::JobOutputs::default(),
            )
            .expect("ffmpeg completed");
        blocked_job.enqueue_task(TaskType::Summary, "2026-01-01T00:00:03Z".to_string());
        store.insert_job(blocked_job).expect("insert blocked job");

        let mut stt_ready_job = JobRecord::new(
            "job-stt".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/job-stt.wav"),
            store.job_dir("job-stt"),
        );
        stt_ready_job
            .mark_completed(
                "2026-01-01T00:00:01Z".to_string(),
                crate::index::JobOutputs::default(),
            )
            .expect("ffmpeg completed");
        stt_ready_job.enqueue_task(TaskType::Stt, "2026-01-01T00:00:04Z".to_string());
        store.insert_job(stt_ready_job).expect("insert stt job");

        store
            .with_index_mut(|index| {
                index.task_queue.active_batch = Some(ActiveQueueBatch {
                    category: QueueCategory::Llm,
                    running: None,
                    entries: vec![build_summary_entry(
                        "job-summary",
                        false,
                        "2026-01-01T00:00:03Z".to_string(),
                    )],
                });
                index.task_queue.pending_batches = vec![QueueBatch {
                    category: QueueCategory::Stt,
                    entries: vec![build_stt_entry(
                        "job-stt",
                        &[PathBuf::from("mono_mix.wav")],
                        "2026-01-01T00:00:04Z".to_string(),
                    )],
                }];
                Ok(())
            })
            .expect("seed queue");

        let mut state = DispatchState::default();
        let work = reserve_next_entry(
            &repo_root,
            &mut state,
            "2026-01-01T00:00:10Z".to_string(),
            false,
        )
        .expect("reserve")
        .expect("work item");
        assert_eq!(work.entry.category, QueueCategory::Stt);
        assert_eq!(work.entry.job_id, "job-stt");
    }

    #[test]
    fn submit_batch_pipeline_jobs_enqueues_unfinished_tasks() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let mut completed = test_job(
            "job-complete",
            "2026-01-01T00:00:00Z",
            "sources/job-complete/source.wav",
            "hash-job-complete",
            "job-complete.wav",
        );
        mark_job_completed_with_audio(
            &store,
            &mut completed,
            "2026-01-01T00:00:01Z",
            &["mono_mix.wav"],
        )
        .expect("ffmpeg complete");
        completed.enqueue_task(TaskType::Stt, "2026-01-01T00:00:02Z".to_string());
        completed.set_task_request_fingerprint(
            TaskType::Stt,
            QueuePayload::Stt {
                audio_files: vec!["mono_mix.wav".to_string()],
                language: "ko".to_string(),
                keywords: Vec::new(),
            }
            .request_fingerprint(),
        );
        completed
            .complete_task(TaskType::Stt, "2026-01-01T00:00:03Z".to_string())
            .expect("stt complete");
        store.insert_job(completed).expect("insert completed");
        seed_transcripts(&store, "job-complete", &[("mono_mix", "transcript")])
            .expect("seed transcripts");

        let queued = JobRecord::new(
            "job-queued".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/job-queued.wav"),
            store.job_dir("job-queued"),
        );
        store.insert_job(queued).expect("insert queued");

        let result =
            submit_batch_pipeline_jobs(&repo_root, BatchProcessTarget::All).expect("batch submit");
        assert_eq!(result.total_jobs, 2);
        assert_eq!(result.ffmpeg_queued, 1);
        assert_eq!(result.stt_queued, 1);
        assert_eq!(result.summary_queued, 2);
        assert_eq!(result.embedding_queued, 2);
    }

    #[test]
    fn submit_batch_pipeline_jobs_injects_dictionary_keywords_into_stt_queue() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        store
            .upsert_stt_dictionary_keyword(
                "RecordRoute",
                crate::index::DictionaryKeywordSource::User,
            )
            .expect("insert dictionary keyword");
        store
            .upsert_stt_dictionary_keyword("회의록", crate::index::DictionaryKeywordSource::Auto)
            .expect("insert auto dictionary keyword");

        let mut job = test_job(
            "job-stt",
            "2026-01-01T00:00:00Z",
            "sources/job-stt/source.wav",
            "hash-job-stt",
            "job-stt.wav",
        );
        mark_job_completed_with_audio(&store, &mut job, "2026-01-01T00:00:01Z", &["mono_mix.wav"])
            .expect("ffmpeg complete");
        store.insert_job(job).expect("insert job");

        let result =
            submit_batch_pipeline_jobs(&repo_root, BatchProcessTarget::All).expect("batch submit");

        assert_eq!(result.stt_queued, 1);

        let queued = store
            .with_index_read(|index| Ok(find_task_entry(index, "job-stt", TaskType::Stt)))
            .expect("read queue")
            .expect("queue entry");
        match queued.payload {
            QueuePayload::Stt { keywords, .. } => {
                assert_eq!(keywords, vec!["RecordRoute".to_string()]);
            }
            other => panic!("expected stt payload, got {other:?}"),
        }
    }

    #[test]
    fn submit_batch_pipeline_jobs_skips_queued_and_running_tasks() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let mut job = test_job(
            "job-inflight",
            "2026-01-01T00:00:00Z",
            "sources/job-inflight/source.wav",
            "hash-job-inflight",
            "job-inflight.wav",
        );
        mark_job_completed_with_audio(&store, &mut job, "2026-01-01T00:00:00Z", &["mono_mix.wav"])
            .expect("mark ffmpeg completed");
        job.enqueue_task(TaskType::Stt, "2026-01-01T00:00:01Z".to_string());
        job.set_task_request_fingerprint(
            TaskType::Stt,
            QueuePayload::Stt {
                audio_files: vec!["mono_mix.wav".to_string()],
                language: "ko".to_string(),
                keywords: Vec::new(),
            }
            .request_fingerprint(),
        );
        job.complete_task(TaskType::Stt, "2026-01-01T00:00:02Z".to_string())
            .expect("stt complete");
        job.enqueue_task(TaskType::Summary, "2026-01-01T00:00:02Z".to_string());
        job.start_task(TaskType::Summary, "2026-01-01T00:00:03Z".to_string())
            .expect("summary running");
        store.insert_job(job).expect("insert inflight");
        seed_transcripts(&store, "job-inflight", &[("mono_mix", "transcript")])
            .expect("seed transcripts");

        let snapshot_before = queue_snapshot(&repo_root).expect("snapshot before");
        let queued_entries_before = snapshot_before
            .active_batch
            .map(|batch| batch.entries.len())
            .unwrap_or(0)
            + snapshot_before
                .pending_batches
                .iter()
                .map(|batch| batch.entries.len())
                .sum::<usize>();

        let result =
            submit_batch_pipeline_jobs(&repo_root, BatchProcessTarget::All).expect("batch submit");

        assert_eq!(result.total_jobs, 1);
        assert_eq!(result.ffmpeg_queued, 0);
        assert_eq!(result.stt_queued, 0);
        assert_eq!(result.summary_queued, 0);
        assert_eq!(result.embedding_queued, 1);

        let snapshot_after = queue_snapshot(&repo_root).expect("snapshot after");
        let queued_entries_after = snapshot_after
            .active_batch
            .map(|batch| batch.entries.len())
            .unwrap_or(0)
            + snapshot_after
                .pending_batches
                .iter()
                .map(|batch| batch.entries.len())
                .sum::<usize>();
        assert_eq!(queued_entries_after, queued_entries_before + 1);

        let persisted_job = store
            .find_job("job-inflight")
            .expect("find job")
            .expect("job exists");
        assert_eq!(
            persisted_job.task(TaskType::Stt).expect("stt task").status,
            TaskStatus::Completed
        );
        assert_eq!(
            persisted_job
                .task(TaskType::Summary)
                .expect("summary task")
                .status,
            TaskStatus::Running
        );
    }

    #[test]
    fn submit_batch_pipeline_jobs_reenqueues_queued_task_when_queue_entry_missing() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let job_dir = store.job_dir("job-missing-queue-entry");
        fs::create_dir_all(&job_dir).expect("job dir");
        fs::write(job_dir.join("mono_mix.wav"), "audio").expect("mono mix");

        let mut job = JobRecord::new(
            "job-missing-queue-entry".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/job-missing-queue-entry.wav"),
            job_dir,
        );
        job.mark_completed(
            "2026-01-01T00:00:01Z".to_string(),
            crate::index::JobOutputs::default(),
        )
        .expect("mark ffmpeg completed");
        job.enqueue_task(TaskType::Stt, "2026-01-01T00:00:01Z".to_string());
        store.insert_job(job).expect("insert job");

        let result =
            submit_batch_pipeline_jobs(&repo_root, BatchProcessTarget::All).expect("batch submit");
        assert_eq!(result.total_jobs, 1);
        assert_eq!(result.ffmpeg_queued, 0);
        assert_eq!(result.stt_queued, 1);
        assert_eq!(result.summary_queued, 1);
        assert_eq!(result.embedding_queued, 1);

        let queued_stt_ticket = IndexStore::new(&repo_root)
            .with_index_read(|index| {
                Ok(find_ticket(index, "job-missing-queue-entry", TaskType::Stt))
            })
            .expect("read index");
        assert!(queued_stt_ticket.is_some());
    }

    #[test]
    fn submit_batch_pipeline_jobs_target_stt_requires_completed_ffmpeg_outputs() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let job = JobRecord::new(
            "job-stt-prereq".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/job-stt-prereq.wav"),
            store.job_dir("job-stt-prereq"),
        );
        store.insert_job(job).expect("insert job");

        let result =
            submit_batch_pipeline_jobs(&repo_root, BatchProcessTarget::Stt).expect("batch submit");

        assert_eq!(result.total_jobs, 1);
        assert_eq!(result.ffmpeg_queued, 0);
        assert_eq!(result.stt_queued, 0);
        assert_eq!(result.summary_queued, 0);
        assert_eq!(result.embedding_queued, 0);
    }

    #[test]
    fn submit_batch_pipeline_jobs_target_stt_only_enqueues_completed_jobs() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);

        for index in 0..5 {
            let job_id = format!("job-stt-ready-{index}");
            let mut job = test_job(
                &job_id,
                "2026-01-01T00:00:00Z",
                &format!("sources/{job_id}/source.wav"),
                &format!("hash-{job_id}"),
                &format!("{job_id}.wav"),
            );
            mark_job_completed_with_audio(
                &store,
                &mut job,
                "2026-01-01T00:00:01Z",
                &["mono_mix.wav"],
            )
            .expect("mark ffmpeg completed");
            store.insert_job(job).expect("insert job");
        }

        let result =
            submit_batch_pipeline_jobs(&repo_root, BatchProcessTarget::Stt).expect("batch submit");

        assert_eq!(result.total_jobs, 5);
        assert_eq!(result.ffmpeg_queued, 0);
        assert_eq!(result.stt_queued, 5);
        assert_eq!(result.summary_queued, 0);
        assert_eq!(result.embedding_queued, 0);
    }

    #[test]
    fn submit_batch_pipeline_jobs_target_summary_skips_jobs_without_transcripts() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let mut job = test_job(
            "job-summary-prereq",
            "2026-01-01T00:00:00Z",
            "sources/job-summary-prereq/source.wav",
            "hash-job-summary-prereq",
            "job-summary-prereq.wav",
        );
        mark_job_completed_with_audio(&store, &mut job, "2026-01-01T00:00:01Z", &["mono_mix.wav"])
            .expect("mark ffmpeg completed");
        store.insert_job(job).expect("insert job");

        let result = submit_batch_pipeline_jobs(&repo_root, BatchProcessTarget::Summary)
            .expect("batch submit");

        assert_eq!(result.total_jobs, 1);
        assert_eq!(result.ffmpeg_queued, 0);
        assert_eq!(result.stt_queued, 0);
        assert_eq!(result.summary_queued, 0);
        assert_eq!(result.embedding_queued, 0);
    }

    #[test]
    fn submit_batch_pipeline_jobs_target_embedding_requires_summary() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let mut job = test_job(
            "job-embedding-prereq",
            "2026-01-01T00:00:00Z",
            "sources/job-embedding-prereq/source.wav",
            "hash-job-embedding-prereq",
            "job-embedding-prereq.wav",
        );
        mark_job_completed_with_audio(&store, &mut job, "2026-01-01T00:00:01Z", &["mono_mix.wav"])
            .expect("mark ffmpeg completed");
        store.insert_job(job).expect("insert job");

        let result = submit_batch_pipeline_jobs(&repo_root, BatchProcessTarget::Embedding)
            .expect("batch submit");

        assert_eq!(result.total_jobs, 1);
        assert_eq!(result.ffmpeg_queued, 0);
        assert_eq!(result.stt_queued, 0);
        assert_eq!(result.summary_queued, 0);
        assert_eq!(result.embedding_queued, 0);
    }

    #[test]
    fn reserve_next_entry_returns_none_when_all_batches_are_blocked() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);

        let mut failed_job = JobRecord::new(
            "job-failed".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/job-failed.wav"),
            store.job_dir("job-failed"),
        );
        failed_job
            .mark_failed(
                "2026-01-01T00:00:01Z".to_string(),
                "synthetic ffmpeg failure".to_string(),
            )
            .expect("mark failed");
        failed_job.enqueue_task(TaskType::Stt, "2026-01-01T00:00:02Z".to_string());
        failed_job.enqueue_task(TaskType::Summary, "2026-01-01T00:00:03Z".to_string());
        failed_job.enqueue_task(TaskType::Embedding, "2026-01-01T00:00:04Z".to_string());
        store.insert_job(failed_job).expect("insert failed job");

        store
            .with_index_mut(|index| {
                index.task_queue.active_batch = Some(ActiveQueueBatch {
                    category: QueueCategory::Stt,
                    running: None,
                    entries: vec![build_stt_entry(
                        "job-failed",
                        &[PathBuf::from("mono_mix.wav")],
                        "2026-01-01T00:00:02Z".to_string(),
                    )],
                });
                index.task_queue.pending_batches = vec![
                    QueueBatch {
                        category: QueueCategory::Llm,
                        entries: vec![build_summary_entry(
                            "job-failed",
                            false,
                            "2026-01-01T00:00:03Z".to_string(),
                        )],
                    },
                    QueueBatch {
                        category: QueueCategory::Embed,
                        entries: vec![build_embedding_entry(
                            "job-failed",
                            "2026-01-01T00:00:04Z".to_string(),
                        )],
                    },
                ];
                Ok(())
            })
            .expect("seed blocked queue");

        let mut state = DispatchState::default();
        let work = reserve_next_entry(
            &repo_root,
            &mut state,
            "2026-01-01T00:00:10Z".to_string(),
            false,
        )
        .expect("reserve");
        assert!(work.is_none());
    }

    #[test]
    fn reserve_next_entry_returns_none_while_paused_then_resumes_same_order() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);

        for job_id in ["job-1", "job-2"] {
            store
                .insert_job(JobRecord::new(
                    job_id.to_string(),
                    "2026-01-01T00:00:00Z".to_string(),
                    PathBuf::from(format!("/tmp/{job_id}.wav")),
                    store.job_dir(job_id),
                ))
                .expect("insert job");
        }

        store
            .with_index_mut(|index| {
                index.task_queue.paused = true;
                index.task_queue.active_batch = Some(ActiveQueueBatch {
                    category: QueueCategory::Ffmpeg,
                    running: None,
                    entries: vec![
                        build_ffmpeg_entry(
                            "job-1",
                            Path::new("/tmp/job-1.wav"),
                            "2026-01-01T00:00:01Z".to_string(),
                        ),
                        build_ffmpeg_entry(
                            "job-2",
                            Path::new("/tmp/job-2.wav"),
                            "2026-01-01T00:00:02Z".to_string(),
                        ),
                    ],
                });
                Ok(())
            })
            .expect("seed queue");

        let mut state = DispatchState::default();
        let paused = reserve_next_entry(
            &repo_root,
            &mut state,
            "2026-01-01T00:00:03Z".to_string(),
            true,
        )
        .expect("pause check");
        assert!(paused.is_none());
        assert!(
            queue_snapshot(&repo_root)
                .expect("snapshot")
                .active_batch
                .as_ref()
                .and_then(|batch| batch.running.as_ref())
                .is_none()
        );

        set_queue_paused(&repo_root, false).expect("resume queue");

        let resumed = reserve_next_entry(
            &repo_root,
            &mut state,
            "2026-01-01T00:00:04Z".to_string(),
            true,
        )
        .expect("resume reserve")
        .expect("work item");
        assert_eq!(resumed.entry.job_id, "job-1");
        assert_eq!(resumed.entry.category, QueueCategory::Ffmpeg);

        let job = store.find_job("job-1").expect("find job").expect("job");
        assert_eq!(job.status, crate::index::JobStatus::Running);
    }

    #[test]
    fn cancel_pending_entries_marks_waiting_tasks_failed_and_keeps_running_entry() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);

        let mut running_job = JobRecord::new(
            "job-running".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/job-running.wav"),
            store.job_dir("job-running"),
        );
        running_job
            .mark_ffmpeg_running("2026-01-01T00:00:01Z".to_string())
            .expect("mark running");
        store.insert_job(running_job).expect("insert running job");

        let queued_ffmpeg = JobRecord::new(
            "job-queued".to_string(),
            "2026-01-01T00:00:02Z".to_string(),
            PathBuf::from("/tmp/job-queued.wav"),
            store.job_dir("job-queued"),
        );
        store
            .insert_job(queued_ffmpeg)
            .expect("insert queued ffmpeg");

        let mut summary_job = test_job(
            "job-summary",
            "2026-01-01T00:00:03Z",
            "sources/job-summary/source.wav",
            "hash-job-summary",
            "job-summary.wav",
        );
        mark_job_completed_with_audio(
            &store,
            &mut summary_job,
            "2026-01-01T00:00:04Z",
            &["mono_mix.wav"],
        )
        .expect("mark summary completed");
        summary_job.enqueue_task(TaskType::Summary, "2026-01-01T00:00:05Z".to_string());
        store.insert_job(summary_job).expect("insert summary job");

        store
            .with_index_mut(|index| {
                index.task_queue.active_batch = Some(ActiveQueueBatch {
                    category: QueueCategory::Ffmpeg,
                    running: Some(build_ffmpeg_entry(
                        "job-running",
                        Path::new("/tmp/job-running.wav"),
                        "2026-01-01T00:00:01Z".to_string(),
                    )),
                    entries: vec![build_ffmpeg_entry(
                        "job-queued",
                        Path::new("/tmp/job-queued.wav"),
                        "2026-01-01T00:00:02Z".to_string(),
                    )],
                });
                index.task_queue.pending_batches = vec![QueueBatch {
                    category: QueueCategory::Llm,
                    entries: vec![build_summary_entry(
                        "job-summary",
                        false,
                        "2026-01-01T00:00:05Z".to_string(),
                    )],
                }];
                Ok(())
            })
            .expect("seed cancel queue");

        let cancelled = cancel_pending_entries(&repo_root).expect("cancel pending");
        assert_eq!(cancelled.total_cancelled, 2);
        assert_eq!(cancelled.ffmpeg_cancelled, 1);
        assert_eq!(cancelled.summary_cancelled, 1);

        let snapshot = queue_snapshot(&repo_root).expect("snapshot");
        assert_eq!(
            snapshot
                .active_batch
                .as_ref()
                .and_then(|batch| batch.running.as_ref())
                .map(|entry| entry.job_id.as_str()),
            Some("job-running")
        );
        assert!(
            snapshot
                .active_batch
                .as_ref()
                .is_some_and(|batch| batch.entries.is_empty())
        );
        assert!(snapshot.pending_batches.is_empty());

        let queued_job = store
            .find_job("job-queued")
            .expect("find queued job")
            .expect("queued job");
        assert_eq!(queued_job.status, crate::index::JobStatus::Failed);
        assert_eq!(
            queued_job.error_message.as_deref(),
            Some(QUEUE_CANCELLED_BY_USER_MESSAGE)
        );

        let summary_job = store
            .find_job("job-summary")
            .expect("find summary job")
            .expect("summary job");
        assert_eq!(
            summary_job.task(TaskType::Summary).map(|task| task.status),
            Some(TaskStatus::Failed)
        );
        assert_eq!(
            summary_job
                .task(TaskType::Summary)
                .and_then(|task| task.last_error.as_deref()),
            Some(QUEUE_CANCELLED_BY_USER_MESSAGE)
        );

        let running_job = store
            .find_job("job-running")
            .expect("find running job")
            .expect("running job");
        assert_eq!(running_job.status, crate::index::JobStatus::Running);
    }

    #[test]
    fn cancel_pending_entries_marks_waiting_tasks_failed_across_all_categories() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);

        let mut running_job = JobRecord::new(
            "job-running".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/job-running.wav"),
            store.job_dir("job-running"),
        );
        running_job
            .mark_ffmpeg_running("2026-01-01T00:00:01Z".to_string())
            .expect("mark running");
        store.insert_job(running_job).expect("insert running job");

        let queued_ffmpeg = JobRecord::new(
            "job-queued".to_string(),
            "2026-01-01T00:00:02Z".to_string(),
            PathBuf::from("/tmp/job-queued.wav"),
            store.job_dir("job-queued"),
        );
        store
            .insert_job(queued_ffmpeg)
            .expect("insert queued ffmpeg");

        let mut stt_job = test_job(
            "job-stt",
            "2026-01-01T00:00:03Z",
            "sources/job-stt/source.wav",
            "hash-job-stt",
            "job-stt.wav",
        );
        mark_job_completed_with_audio(
            &store,
            &mut stt_job,
            "2026-01-01T00:00:04Z",
            &["mono_mix.wav"],
        )
        .expect("mark stt job completed");
        stt_job.enqueue_task(TaskType::Stt, "2026-01-01T00:00:05Z".to_string());
        store.insert_job(stt_job).expect("insert stt job");

        let mut summary_job = test_job(
            "job-summary",
            "2026-01-01T00:00:06Z",
            "sources/job-summary/source.wav",
            "hash-job-summary",
            "job-summary.wav",
        );
        mark_job_completed_with_audio(
            &store,
            &mut summary_job,
            "2026-01-01T00:00:07Z",
            &["mono_mix.wav"],
        )
        .expect("mark summary completed");
        summary_job.enqueue_task(TaskType::Summary, "2026-01-01T00:00:08Z".to_string());
        store.insert_job(summary_job).expect("insert summary job");

        let mut embedding_job = test_job(
            "job-embedding",
            "2026-01-01T00:00:09Z",
            "sources/job-embedding/source.wav",
            "hash-job-embedding",
            "job-embedding.wav",
        );
        mark_job_completed_with_audio(
            &store,
            &mut embedding_job,
            "2026-01-01T00:00:10Z",
            &["mono_mix.wav"],
        )
        .expect("mark embedding job completed");
        embedding_job.enqueue_task(TaskType::Embedding, "2026-01-01T00:00:11Z".to_string());
        store
            .insert_job(embedding_job)
            .expect("insert embedding job");

        store
            .with_index_mut(|index| {
                index.task_queue.active_batch = Some(ActiveQueueBatch {
                    category: QueueCategory::Ffmpeg,
                    running: Some(build_ffmpeg_entry(
                        "job-running",
                        Path::new("/tmp/job-running.wav"),
                        "2026-01-01T00:00:01Z".to_string(),
                    )),
                    entries: vec![build_ffmpeg_entry(
                        "job-queued",
                        Path::new("/tmp/job-queued.wav"),
                        "2026-01-01T00:00:02Z".to_string(),
                    )],
                });
                index.task_queue.pending_batches = vec![
                    QueueBatch {
                        category: QueueCategory::Stt,
                        entries: vec![build_stt_entry(
                            "job-stt",
                            &[PathBuf::from("mono_mix.wav")],
                            "2026-01-01T00:00:05Z".to_string(),
                        )],
                    },
                    QueueBatch {
                        category: QueueCategory::Llm,
                        entries: vec![build_summary_entry(
                            "job-summary",
                            false,
                            "2026-01-01T00:00:08Z".to_string(),
                        )],
                    },
                    QueueBatch {
                        category: QueueCategory::Embed,
                        entries: vec![build_embedding_entry(
                            "job-embedding",
                            "2026-01-01T00:00:11Z".to_string(),
                        )],
                    },
                ];
                Ok(())
            })
            .expect("seed cancel queue");

        let cancelled = cancel_pending_entries(&repo_root).expect("cancel pending");
        assert_eq!(cancelled.total_cancelled, 4);
        assert_eq!(cancelled.ffmpeg_cancelled, 1);
        assert_eq!(cancelled.stt_cancelled, 1);
        assert_eq!(cancelled.summary_cancelled, 1);
        assert_eq!(cancelled.embedding_cancelled, 1);
    }
    fn temp_workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!("recordroute-queue-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).expect("temp workspace");
        path
    }
}
