use super::{
    DispatchState, IndexStore, Path, PathBuf, QueueEntry, TaskType, artifacts, embedding_stage,
    ffmpeg_stage, now_rfc3339, stt_stage, summary_stage,
};
use crate::index::{QueuePayload, TaskStatus};

pub fn recover_interrupted_active_entry(repo_root: &Path) -> Result<(), String> {
    let queued_at = now_rfc3339()?;
    IndexStore::new(repo_root).with_index_mut(|index| {
        let running_entry = index
            .task_queue
            .active_batch
            .as_mut()
            .and_then(|active_batch| active_batch.running.take());
        let Some(entry) = running_entry else {
            return Ok(());
        };

        super::scheduler::requeue_entry(index, &entry, queued_at.clone())?;
        if let Some(active_batch) = index.task_queue.active_batch.as_mut() {
            active_batch.entries.insert(0, entry);
        }
        Ok(())
    })
}

pub fn dispatch_one(repo_root: &Path, state: &mut DispatchState) -> Result<bool, String> {
    let started_at = now_rfc3339()?;
    let Some(work) = super::scheduler::reserve_next_entry(repo_root, state, started_at)? else {
        return Ok(false);
    };

    let execution_result = execute_work_item(repo_root, &work.entry);
    let clear_result = super::scheduler::clear_running_entry(repo_root, &work.entry);

    if let Err(error) = clear_result {
        eprintln!("{error}");
    }

    state.record_execution(work.entry.category);

    if let Err(error) = execution_result {
        eprintln!("{error}");
    }

    Ok(true)
}

pub fn dispatch_until_task_terminal(
    repo_root: &Path,
    job_id: &str,
    task_type: TaskType,
) -> Result<(), String> {
    let mut state = DispatchState::default();

    loop {
        let job = IndexStore::new(repo_root)
            .find_job(job_id)?
            .ok_or_else(|| format!("job not found in index: {job_id}"))?;
        let task = job
            .task(task_type)
            .cloned()
            .ok_or_else(|| format!("task not found for job {job_id}: {}", task_type.as_str()))?;

        match task.status {
            TaskStatus::Queued | TaskStatus::Running => {
                if !dispatch_one(repo_root, &mut state)? {
                    return Err(format!(
                        "queue stalled before {} task completed for job {job_id}",
                        task_type.as_str()
                    ));
                }
            }
            TaskStatus::Completed => return Ok(()),
            TaskStatus::Failed => {
                return Err(task.last_error.unwrap_or_else(|| {
                    format!("{} task failed for job {job_id}", task_type.as_str())
                }));
            }
        }
    }
}

fn execute_work_item(repo_root: &Path, entry: &QueueEntry) -> Result<(), String> {
    match &entry.payload {
        QueuePayload::Ffmpeg { input_path } => {
            ffmpeg_stage::execute_ffmpeg_job(repo_root, &entry.job_id, Path::new(input_path))?;
            Ok(())
        }
        QueuePayload::Stt { audio_files } => {
            let paths = if audio_files.is_empty() {
                let job = IndexStore::new(repo_root)
                    .find_job(&entry.job_id)?
                    .ok_or_else(|| format!("job not found: {}", entry.job_id))?;
                artifacts::supported_audio_files(&IndexStore::new(repo_root).job_dir(&job.job_id))?
            } else {
                audio_files.iter().map(PathBuf::from).collect::<Vec<_>>()
            };
            stt_stage::execute_stt_job(repo_root, &entry.job_id, &paths)?;
            Ok(())
        }
        QueuePayload::Summary { force_regenerate } => {
            summary_stage::execute_summary_job(repo_root, &entry.job_id, *force_regenerate)?;
            Ok(())
        }
        QueuePayload::Embedding => {
            embedding_stage::execute_summary_embedding_job(repo_root, &entry.job_id)?;
            Ok(())
        }
    }
}
