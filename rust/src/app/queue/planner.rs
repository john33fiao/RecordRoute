use super::{
    BatchQueueSubmission, IndexFile, IndexStore, JobRecord, Path, PathBuf, TaskType, artifacts,
    build_embedding_entry, build_ffmpeg_entry, build_stt_entry,
    build_summary_entry, embedding_stage, enqueue_entry, find_ticket, now_rfc3339,
};
use crate::index::{JobStatus, TaskStatus};

pub fn submit_batch_pipeline_jobs(repo_root: &Path) -> Result<BatchQueueSubmission, String> {
    let queued_at = now_rfc3339()?;
    let store = IndexStore::new(repo_root);
    store.with_index_mut(|index| {
        let mut summary = BatchQueueSubmission {
            total_jobs: index.jobs.len(),
            ..BatchQueueSubmission::default()
        };

        let job_ids = index
            .jobs
            .iter()
            .map(|job| job.job_id.clone())
            .collect::<Vec<_>>();

        for job_id in job_ids {
            let Some(job_index) = index.jobs.iter().position(|job| job.job_id == job_id) else {
                continue;
            };
            let job = index.jobs[job_index].clone();
            let job_dir = store.job_dir(&job.job_id);

            if should_enqueue_batch_ffmpeg(index, &job) {
                let entry =
                    build_ffmpeg_entry(&job.job_id, Path::new(&job.source_ref), queued_at.clone());
                index.jobs[job_index].mark_ffmpeg_queued(queued_at.clone());
                enqueue_entry(index, entry);
                summary.ffmpeg_queued = summary.ffmpeg_queued.saturating_add(1);
                continue;
            }

            if job.status != JobStatus::Completed {
                continue;
            }

            let audio_files = artifacts::supported_audio_files(&job_dir)?;
            if should_enqueue_batch_stt(&store, index, &job, &audio_files)? {
                let entry = build_stt_entry(&job.job_id, &audio_files, queued_at.clone());
                index.jobs[job_index].enqueue_task(TaskType::Stt, queued_at.clone());
                enqueue_entry(index, entry);
                summary.stt_queued = summary.stt_queued.saturating_add(1);
                continue;
            }

            if should_enqueue_batch_summary(&store, index, &job, &audio_files)? {
                let entry = build_summary_entry(&job.job_id, false, queued_at.clone());
                index.jobs[job_index].enqueue_task(TaskType::Summary, queued_at.clone());
                enqueue_entry(index, entry);
                summary.summary_queued = summary.summary_queued.saturating_add(1);
                continue;
            }

            if should_enqueue_batch_embedding(&store, repo_root, index, &job)? {
                let entry = build_embedding_entry(&job.job_id, queued_at.clone());
                index.jobs[job_index].enqueue_task(TaskType::Embedding, queued_at.clone());
                enqueue_entry(index, entry);
                summary.embedding_queued = summary.embedding_queued.saturating_add(1);
            }
        }

        Ok(summary)
    })
}

fn should_enqueue_batch_ffmpeg(index: &IndexFile, job: &JobRecord) -> bool {
    match job.status {
        JobStatus::Completed | JobStatus::Running => false,
        JobStatus::Queued | JobStatus::Failed => {
            find_ticket(index, &job.job_id, TaskType::Ffmpeg).is_none()
        }
    }
}

fn should_enqueue_batch_stt(
    store: &IndexStore,
    index: &IndexFile,
    job: &JobRecord,
    audio_files: &[PathBuf],
) -> Result<bool, String> {
    if audio_files.is_empty() || all_transcripts_exist(store, &job.job_id, audio_files)? {
        return Ok(false);
    }

    Ok(match job.task(TaskType::Stt).map(|task| task.status) {
        Some(TaskStatus::Queued) => find_ticket(index, &job.job_id, TaskType::Stt).is_none(),
        Some(TaskStatus::Running) => false,
        Some(TaskStatus::Completed) | Some(TaskStatus::Failed) | None => true,
    })
}

fn should_enqueue_batch_summary(
    store: &IndexStore,
    index: &IndexFile,
    job: &JobRecord,
    audio_files: &[PathBuf],
) -> Result<bool, String> {
    if audio_files.is_empty() || !all_transcripts_exist(store, &job.job_id, audio_files)? {
        return Ok(false);
    }

    if store.get_summary(&job.job_id)?.is_some() {
        return Ok(false);
    }

    Ok(match job.task(TaskType::Summary).map(|task| task.status) {
        Some(TaskStatus::Queued) => find_ticket(index, &job.job_id, TaskType::Summary).is_none(),
        Some(TaskStatus::Running) => false,
        Some(TaskStatus::Completed) | Some(TaskStatus::Failed) | None => true,
    })
}

fn should_enqueue_batch_embedding(
    store: &IndexStore,
    repo_root: &Path,
    index: &IndexFile,
    job: &JobRecord,
) -> Result<bool, String> {
    if matches!(
        job.task(TaskType::Summary).map(|task| task.status),
        Some(TaskStatus::Queued | TaskStatus::Running)
    ) {
        return Ok(false);
    }

    if store.get_summary(&job.job_id)?.is_none() {
        return Ok(false);
    }

    if !embedding_stage::is_embedding_stale(repo_root, job)? {
        return Ok(false);
    }

    Ok(match job.task(TaskType::Embedding).map(|task| task.status) {
        Some(TaskStatus::Queued) => find_ticket(index, &job.job_id, TaskType::Embedding).is_none(),
        Some(TaskStatus::Running) => false,
        Some(TaskStatus::Completed) | Some(TaskStatus::Failed) | None => true,
    })
}

pub(super) fn all_transcripts_exist(
    store: &IndexStore,
    job_id: &str,
    audio_files: &[PathBuf],
) -> Result<bool, String> {
    if audio_files.is_empty() {
        return Ok(false);
    }

    audio_files
        .iter()
        .try_fold(true, |all_present, audio_file| -> Result<bool, String> {
            let file_name = artifacts::transcript_file_name(audio_file)?;
            let transcript_id = artifacts::transcript_id_from_file_name(&file_name)?;
            Ok(all_present && store.find_transcript(job_id, &transcript_id)?.is_some())
        })
}
