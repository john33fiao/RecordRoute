use super::{QueueTicket, StageJobDisposition, StageJobSubmission, queue};
use crate::index::{IndexStore, JobRecord, QueueEntry, TaskStatus, TaskType};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

const TASK_POLL_INTERVAL: Duration = Duration::from_millis(100);

pub(crate) enum InflightSubmissionResolution {
    Continue,
    Deduplicate,
    UpgradeQueuedEntry,
    Conflict(String),
}

pub(crate) fn finalize_task_success(
    repo_root: &Path,
    mut job: JobRecord,
    task_type: TaskType,
    finished_at: String,
) -> Result<JobRecord, String> {
    job.complete_task(task_type, finished_at)?;
    IndexStore::new(repo_root).update_job(&job.job_id, |_| job.clone())?;
    Ok(job)
}

pub(crate) fn finalize_task_failure(
    repo_root: &Path,
    mut job: JobRecord,
    task_type: TaskType,
    finished_at: String,
    error: String,
) -> Result<(), String> {
    job.fail_task(task_type, finished_at, error)?;
    IndexStore::new(repo_root).update_job(&job.job_id, |_| job.clone())
}

pub(crate) fn queued_task_ticket(
    index_store: &IndexStore,
    job_id: &str,
    task_type: TaskType,
) -> Result<Option<QueueTicket>, String> {
    index_store.with_index_read(|index| Ok(queue::find_ticket(index, job_id, task_type)))
}

pub(crate) fn enqueue_existing_task(
    index_store: &IndexStore,
    job_id: &str,
    task_type: TaskType,
    queued_at: String,
    entry: QueueEntry,
) -> Result<(JobRecord, QueueTicket), String> {
    index_store.with_index_mut(|index| {
        let job_index = index
            .jobs
            .iter()
            .position(|record| record.job_id == job_id)
            .ok_or_else(|| format!("job not found in index: {job_id}"))?;
        {
            let job = &mut index.jobs[job_index];
            match task_type {
                TaskType::Ffmpeg => job.mark_ffmpeg_queued(queued_at.clone()),
                _ => {
                    job.enqueue_task(task_type, queued_at.clone());
                    job.set_task_request_fingerprint(
                        task_type,
                        entry.payload.request_fingerprint(),
                    );
                }
            }
        }
        let ticket = queue::enqueue_entry(index, entry);
        Ok((index.jobs[job_index].clone(), ticket))
    })
}

pub(crate) fn resolve_inflight_stage_submission<M, U>(
    index_store: &IndexStore,
    job: &JobRecord,
    task_type: TaskType,
    planned_audio_files: Vec<PathBuf>,
    resolve: M,
    upgrade_queued: U,
) -> Result<Option<StageJobSubmission>, String>
where
    M: FnOnce(Option<&QueueEntry>, TaskStatus) -> InflightSubmissionResolution,
    U: FnOnce(&IndexStore, &str, TaskType) -> Result<Option<(JobRecord, QueueTicket)>, String>,
{
    let Some(task) = job
        .task(task_type)
        .filter(|task| matches!(task.status, TaskStatus::Queued | TaskStatus::Running))
    else {
        return Ok(None);
    };

    let existing_entry = index_store
        .with_index_read(|index| Ok(queue::find_task_entry(index, &job.job_id, task_type)))?;

    match resolve(existing_entry.as_ref(), task.status) {
        InflightSubmissionResolution::Continue => Ok(None),
        InflightSubmissionResolution::Deduplicate => Ok(Some(StageJobSubmission {
            job: job.clone(),
            disposition: StageJobDisposition::Deduplicated,
            planned_audio_files,
            queue: if task.status == TaskStatus::Queued {
                queued_task_ticket(index_store, &job.job_id, task_type)?
            } else {
                None
            },
        })),
        InflightSubmissionResolution::UpgradeQueuedEntry => {
            let Some((job, ticket)) = upgrade_queued(index_store, &job.job_id, task_type)? else {
                return Err(format!(
                    "{} task requested queued upgrade without an updated queue entry: {}",
                    task_type.as_str(),
                    job.job_id
                ));
            };
            Ok(Some(StageJobSubmission {
                job,
                disposition: StageJobDisposition::Submitted,
                planned_audio_files,
                queue: Some(ticket),
            }))
        }
        InflightSubmissionResolution::Conflict(message) => Err(message),
    }
}

pub(crate) fn wait_for_task_completion(
    repo_root: &Path,
    job_id: &str,
    task_type: TaskType,
) -> Result<JobRecord, String> {
    let index_store = IndexStore::new(repo_root);

    loop {
        let job = index_store
            .find_job(job_id)?
            .ok_or_else(|| format!("job not found in index: {job_id}"))?;
        let task = job
            .task(task_type)
            .cloned()
            .ok_or_else(|| format!("task not found for job {job_id}: {}", task_type.as_str()))?;

        match task.status {
            TaskStatus::Queued | TaskStatus::Running => thread::sleep(TASK_POLL_INTERVAL),
            TaskStatus::Completed => return Ok(job),
            TaskStatus::Failed => {
                return Err(task.last_error.unwrap_or_else(|| {
                    format!("{} task failed for job {job_id}", task_type.as_str())
                }));
            }
        }
    }
}
