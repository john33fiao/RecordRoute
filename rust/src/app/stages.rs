use crate::index::{IndexStore, JobRecord, TaskStatus, TaskType};
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
            TaskStatus::Running => thread::sleep(TASK_POLL_INTERVAL),
            TaskStatus::Completed => return Ok(job),
            TaskStatus::Failed => {
                return Err(task.last_error.unwrap_or_else(|| {
                    format!("{} task failed for job {job_id}", task_type.as_str())
                }));
            }
        }
    }
}
