use super::{
    BatchQueueSubmission, IndexFile, IndexStore, JobRecord, Path, PathBuf, TaskType, artifacts,
    build_embedding_entry, build_ffmpeg_entry, build_stt_entry_with_options, build_summary_entry,
    embedding_stage, enqueue_entry, find_ticket, now_rfc3339, summary_stage,
};
use crate::app::BatchProcessTarget;
use crate::app::RESET_BY_USER_MESSAGE;
use crate::app::dictionary;
use crate::index::{JobStatus, TaskStatus};
use crate::whisper::transcription_language_from_env;

pub fn submit_batch_pipeline_jobs(
    repo_root: &Path,
    target: BatchProcessTarget,
) -> Result<BatchQueueSubmission, String> {
    let queued_at = now_rfc3339()?;
    let store = IndexStore::new(repo_root);
    let default_language = transcription_language_from_env();
    let default_keywords = dictionary::resolve_stt_keywords(&store, &[])?;
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
            let audio_files = artifacts::listed_or_discovered_audio_files(&store, &job.job_id)?;
            match target {
                BatchProcessTarget::All => {
                    if should_enqueue_batch_ffmpeg(index, &job) {
                        let entry = build_ffmpeg_entry(
                            &job.job_id,
                            Path::new(&job.source_ref),
                            queued_at.clone(),
                        );
                        index.jobs[job_index].mark_ffmpeg_queued(queued_at.clone());
                        enqueue_entry(index, entry);
                        summary.ffmpeg_queued = summary.ffmpeg_queued.saturating_add(1);
                    }

                    if should_enqueue_batch_stt(
                        &store,
                        index,
                        &job,
                        &audio_files,
                        &default_language,
                        &default_keywords,
                    )? {
                        let entry = build_stt_entry_with_options(
                            &job.job_id,
                            &audio_files,
                            &default_language,
                            &default_keywords,
                            queued_at.clone(),
                        );
                        index.jobs[job_index].enqueue_task(TaskType::Stt, queued_at.clone());
                        index.jobs[job_index].set_task_request_fingerprint(
                            TaskType::Stt,
                            entry.payload.request_fingerprint(),
                        );
                        enqueue_entry(index, entry);
                        summary.stt_queued = summary.stt_queued.saturating_add(1);
                    }

                    if should_enqueue_batch_summary(&store, index, &job)? {
                        let entry = build_summary_entry(&job.job_id, false, queued_at.clone());
                        index.jobs[job_index].enqueue_task(TaskType::Summary, queued_at.clone());
                        enqueue_entry(index, entry);
                        summary.summary_queued = summary.summary_queued.saturating_add(1);
                    }

                    if should_enqueue_batch_embedding(&store, repo_root, index, &job, false)? {
                        let entry = build_embedding_entry(&job.job_id, queued_at.clone());
                        index.jobs[job_index].enqueue_task(TaskType::Embedding, queued_at.clone());
                        enqueue_entry(index, entry);
                        summary.embedding_queued = summary.embedding_queued.saturating_add(1);
                    }
                }
                BatchProcessTarget::Ffmpeg => {
                    if should_enqueue_batch_ffmpeg(index, &job) {
                        let entry = build_ffmpeg_entry(
                            &job.job_id,
                            Path::new(&job.source_ref),
                            queued_at.clone(),
                        );
                        index.jobs[job_index].mark_ffmpeg_queued(queued_at.clone());
                        enqueue_entry(index, entry);
                        summary.ffmpeg_queued = summary.ffmpeg_queued.saturating_add(1);
                    }
                }
                BatchProcessTarget::Stt => {
                    if stt_prerequisites_ready(&job, &audio_files)
                        && should_enqueue_batch_stt(
                            &store,
                            index,
                            &job,
                            &audio_files,
                            &default_language,
                            &default_keywords,
                        )?
                    {
                        let entry = build_stt_entry_with_options(
                            &job.job_id,
                            &audio_files,
                            &default_language,
                            &default_keywords,
                            queued_at.clone(),
                        );
                        index.jobs[job_index].enqueue_task(TaskType::Stt, queued_at.clone());
                        index.jobs[job_index].set_task_request_fingerprint(
                            TaskType::Stt,
                            entry.payload.request_fingerprint(),
                        );
                        enqueue_entry(index, entry);
                        summary.stt_queued = summary.stt_queued.saturating_add(1);
                    }
                }
                BatchProcessTarget::Summary => {
                    if summary_stage::summary_prerequisites_ready(&store, &job.job_id)?
                        && should_enqueue_batch_summary(&store, index, &job)?
                    {
                        let entry = build_summary_entry(&job.job_id, false, queued_at.clone());
                        index.jobs[job_index].enqueue_task(TaskType::Summary, queued_at.clone());
                        enqueue_entry(index, entry);
                        summary.summary_queued = summary.summary_queued.saturating_add(1);
                    }
                }
                BatchProcessTarget::Embedding => {
                    if should_enqueue_batch_embedding(&store, repo_root, index, &job, true)? {
                        let entry = build_embedding_entry(&job.job_id, queued_at.clone());
                        index.jobs[job_index].enqueue_task(TaskType::Embedding, queued_at.clone());
                        enqueue_entry(index, entry);
                        summary.embedding_queued = summary.embedding_queued.saturating_add(1);
                    }
                }
            }
        }

        Ok(summary)
    })
}

fn stt_prerequisites_ready(job: &JobRecord, audio_files: &[PathBuf]) -> bool {
    job.status == JobStatus::Completed && !audio_files.is_empty()
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
    language: &str,
    keywords: &[String],
) -> Result<bool, String> {
    if audio_files.is_empty() && job.status == JobStatus::Completed {
        return Ok(false);
    }

    if audio_files.is_empty() && store.count_transcripts(&job.job_id)? > 0 {
        return Ok(false);
    }

    if !audio_files.is_empty() {
        let default_entry = build_stt_entry_with_options(
            &job.job_id,
            audio_files,
            language,
            keywords,
            String::new(),
        );
        let default_request_fingerprint = default_entry.payload.request_fingerprint();
        let transcripts_reusable = all_transcripts_exist(store, &job.job_id, audio_files)?
            && job
                .task(TaskType::Stt)
                .and_then(|task| task.request_fingerprint.as_ref())
                == default_request_fingerprint.as_ref();
        if transcripts_reusable {
            return Ok(false);
        }
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
) -> Result<bool, String> {
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
    require_summary_ready: bool,
) -> Result<bool, String> {
    let summary_exists = store.get_summary(&job.job_id)?.is_some();
    if require_summary_ready && !summary_exists {
        return Ok(false);
    }

    let embedding_present =
        job.summary_embedding.is_some() && store.get_summary_embedding(&job.job_id)?.is_some();
    let reset_requested = job
        .task(TaskType::Embedding)
        .and_then(|task| task.last_error.as_deref())
        == Some(RESET_BY_USER_MESSAGE);
    if summary_exists && embedding_present && !reset_requested {
        return Ok(false);
    }

    if summary_exists && embedding_present && !embedding_stage::is_embedding_stale(repo_root, job)?
    {
        return Ok(false);
    }

    Ok(
        match job.task(TaskType::Embedding).map(|task| task.status) {
            Some(TaskStatus::Queued) => {
                find_ticket(index, &job.job_id, TaskType::Embedding).is_none()
            }
            Some(TaskStatus::Running) => false,
            Some(TaskStatus::Completed) | Some(TaskStatus::Failed) | None => true,
        },
    )
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
