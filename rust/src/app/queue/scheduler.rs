use super::{
    ActiveQueueBatch, DispatchState, IndexFile, IndexStore, JobRecord, Path, QueueEntry,
    QueueWorkItem, TaskType, artifacts,
};
use crate::index::{JobStatus, QueueBatch};

pub(super) fn reserve_next_entry(
    repo_root: &Path,
    state: &mut DispatchState,
    started_at: String,
) -> Result<Option<QueueWorkItem>, String> {
    IndexStore::new(repo_root).with_index_mut(|index| {
        let mut scanned_batches = 0usize;
        loop {
            promote_pending_batch(index);

            let Some(active_category) = index
                .task_queue
                .active_batch
                .as_ref()
                .map(|batch| batch.category)
            else {
                return Ok(None);
            };

            if index
                .task_queue
                .active_batch
                .as_ref()
                .and_then(|batch| batch.running.as_ref())
                .is_some()
            {
                return Ok(None);
            }

            if index
                .task_queue
                .active_batch
                .as_ref()
                .is_some_and(|batch| batch.entries.is_empty())
            {
                index.task_queue.active_batch = None;
                continue;
            }

            if !index.task_queue.pending_batches.is_empty()
                && state.should_rotate(active_category, index.task_queue.burst_limit)
            {
                rotate_active_batch(index);
                continue;
            }

            let entry = {
                let mut entries = {
                    let active_batch = index.task_queue.active_batch.as_mut().ok_or_else(|| {
                        "active batch disappeared while reserving work".to_string()
                    })?;
                    std::mem::take(&mut active_batch.entries)
                };
                let total_entries = entries.len();
                let mut selected = None;

                for _ in 0..total_entries {
                    let candidate = entries.remove(0);
                    if is_entry_ready(repo_root, index, &candidate) {
                        selected = Some(candidate);
                        break;
                    }
                    entries.push(candidate);
                }

                {
                    let active_batch = index.task_queue.active_batch.as_mut().ok_or_else(|| {
                        "active batch disappeared while restoring work".to_string()
                    })?;
                    active_batch.entries = entries;
                }

                let Some(entry) = selected else {
                    let total_batches = 1usize.saturating_add(index.task_queue.pending_batches.len());
                    scanned_batches = scanned_batches.saturating_add(1);
                    if scanned_batches >= total_batches {
                        return Ok(None);
                    }
                    if !index.task_queue.pending_batches.is_empty() {
                        rotate_active_batch(index);
                        continue;
                    }
                    return Ok(None);
                };

                index
                    .task_queue
                    .active_batch
                    .as_mut()
                    .ok_or_else(|| "active batch disappeared while marking running".to_string())?
                    .running = Some(entry.clone());
                entry
            };
            if let Err(error) = mark_entry_running(index, &entry, started_at.clone()) {
                clear_running_entry_in_index(index, &entry);
                return Err(error);
            }
            return Ok(Some(QueueWorkItem { entry }));
        }
    })
}

fn is_entry_ready(repo_root: &Path, index: &IndexFile, entry: &QueueEntry) -> bool {
    let Some(job) = index.jobs.iter().find(|job| job.job_id == entry.job_id) else {
        return false;
    };
    let store = IndexStore::new(repo_root);

    match entry.task_type {
        TaskType::Ffmpeg => true,
        TaskType::Stt => job.status == JobStatus::Completed,
        TaskType::Summary => summary_inputs_ready(&store, job).unwrap_or(false),
        TaskType::Embedding => summary_output_exists(&store, job).unwrap_or(false),
    }
}

fn summary_inputs_ready(store: &IndexStore, job: &JobRecord) -> Result<bool, String> {
    let job_dir = store.job_dir(&job.job_id);
    let audio_files = artifacts::supported_audio_files(&job_dir)?;
    if audio_files.is_empty() {
        return Ok(store.count_transcripts(&job.job_id)? > 0);
    }

    super::planner::all_transcripts_exist(store, &job.job_id, &audio_files)
}

fn summary_output_exists(store: &IndexStore, job: &JobRecord) -> Result<bool, String> {
    Ok(store.get_summary(&job.job_id)?.is_some())
}

fn promote_pending_batch(index: &mut IndexFile) {
    let needs_promote = index
        .task_queue
        .active_batch
        .as_ref()
        .is_none_or(|batch| batch.running.is_none() && batch.entries.is_empty());
    if !needs_promote || index.task_queue.pending_batches.is_empty() {
        return;
    }

    let next = index.task_queue.pending_batches.remove(0);
    index.task_queue.active_batch = Some(ActiveQueueBatch {
        category: next.category,
        running: None,
        entries: next.entries,
    });
}

fn rotate_active_batch(index: &mut IndexFile) {
    let Some(active_batch) = index.task_queue.active_batch.as_mut() else {
        return;
    };
    if active_batch.running.is_some() || active_batch.entries.is_empty() {
        return;
    }

    let rotated = QueueBatch {
        category: active_batch.category,
        entries: std::mem::take(&mut active_batch.entries),
    };
    index.task_queue.pending_batches.push(rotated);
    index.task_queue.active_batch = None;
}

fn mark_entry_running(
    index: &mut IndexFile,
    entry: &QueueEntry,
    started_at: String,
) -> Result<(), String> {
    let job = index
        .jobs
        .iter_mut()
        .find(|job| job.job_id == entry.job_id)
        .ok_or_else(|| format!("job not found in index: {}", entry.job_id))?;

    match entry.task_type {
        TaskType::Ffmpeg => job.mark_ffmpeg_running(started_at),
        _ => job.start_task(entry.task_type, started_at),
    }
}

pub(super) fn requeue_entry(
    index: &mut IndexFile,
    entry: &QueueEntry,
    queued_at: String,
) -> Result<(), String> {
    let job = index
        .jobs
        .iter_mut()
        .find(|job| job.job_id == entry.job_id)
        .ok_or_else(|| format!("job not found in index: {}", entry.job_id))?;

    match entry.task_type {
        TaskType::Ffmpeg => job.mark_ffmpeg_queued(queued_at),
        _ => job.enqueue_task(entry.task_type, queued_at),
    }

    Ok(())
}

pub(super) fn clear_running_entry(repo_root: &Path, entry: &QueueEntry) -> Result<(), String> {
    IndexStore::new(repo_root).with_index_mut(|index| {
        clear_running_entry_in_index(index, entry);
        Ok(())
    })
}

pub(super) fn clear_running_entry_in_index(index: &mut IndexFile, entry: &QueueEntry) {
    let Some(active_batch) = index.task_queue.active_batch.as_mut() else {
        return;
    };
    if active_batch.running.as_ref().is_some_and(|running| {
        running.job_id == entry.job_id && running.task_type == entry.task_type
    }) {
        active_batch.running = None;
    }
}
