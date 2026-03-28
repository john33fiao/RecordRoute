use super::{QueueTicket, now_rfc3339, queue};
use crate::index::{IndexStore, JobRecord, QueueEntry, TaskStatus, TaskType};
use std::path::Path;
use std::thread;
use std::time::Duration;

const TASK_POLL_INTERVAL: Duration = Duration::from_millis(100);

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
                _ => job.enqueue_task(task_type, queued_at.clone()),
            }
        }
        let ticket = queue::enqueue_entry(index, entry);
        Ok((index.jobs[job_index].clone(), ticket))
    })
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

pub(crate) fn record_followup_submission_failure(
    repo_root: &Path,
    job_id: &str,
    task_type: TaskType,
    error: String,
) -> Result<(), String> {
    let finished_at = now_rfc3339()?;
    IndexStore::new(repo_root).update_job(job_id, |job| {
        let mut updated = job.clone();
        if updated.task(task_type).is_none() {
            updated.enqueue_task(task_type, finished_at.clone());
        }
        let _ = updated.fail_task(task_type, finished_at.clone(), error.clone());
        updated
    })
}
