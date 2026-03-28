use super::{
    BatchQueueSubmission, artifacts, embedding_stage, ffmpeg_stage, now_rfc3339, stt_stage,
    summary_stage,
};
use crate::index::{
    ActiveQueueBatch, IndexFile, IndexStore, JobRecord, JobStatus, QueueBatch, QueueCategory,
    QueueEntry, QueuePayload, TaskQueueState, TaskStatus, TaskType,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueTicket {
    pub category: QueueCategory,
    pub position: usize,
    pub queued_at: String,
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

pub fn build_ffmpeg_entry(job_id: &str, input_path: &Path, queued_at: String) -> QueueEntry {
    QueueEntry {
        job_id: job_id.to_string(),
        task_type: TaskType::Ffmpeg,
        category: QueueCategory::Ffmpeg,
        queued_at: queued_at.clone(),
        payload: QueuePayload::Ffmpeg {
            input_path: input_path.to_string_lossy().into_owned(),
        },
    }
}

pub fn build_stt_entry(job_id: &str, audio_files: &[PathBuf], queued_at: String) -> QueueEntry {
    QueueEntry {
        job_id: job_id.to_string(),
        task_type: TaskType::Stt,
        category: QueueCategory::Stt,
        queued_at: queued_at.clone(),
        payload: QueuePayload::Stt {
            audio_files: audio_files
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
        },
    }
}

pub fn build_summary_entry(job_id: &str, force_regenerate: bool, queued_at: String) -> QueueEntry {
    QueueEntry {
        job_id: job_id.to_string(),
        task_type: TaskType::Summary,
        category: QueueCategory::Llm,
        queued_at: queued_at.clone(),
        payload: QueuePayload::Summary { force_regenerate },
    }
}

pub fn build_embedding_entry(job_id: &str, queued_at: String) -> QueueEntry {
    QueueEntry {
        job_id: job_id.to_string(),
        task_type: TaskType::Embedding,
        category: QueueCategory::Embed,
        queued_at: queued_at.clone(),
        payload: QueuePayload::Embedding,
    }
}

pub fn enqueue_entry(index: &mut IndexFile, entry: QueueEntry) -> QueueTicket {
    let state = &mut index.task_queue;
    if state.burst_limit == 0 {
        state.burst_limit = TaskQueueState::default().burst_limit;
    }

    if let Some(active_batch) = state.active_batch.as_mut()
        && active_batch.category == entry.category
    {
        active_batch.entries.push(entry.clone());
        return QueueTicket {
            category: entry.category,
            position: active_batch.entries.len(),
            queued_at: entry.queued_at,
        };
    }

    let active_len = state
        .active_batch
        .as_ref()
        .map(|batch| batch.entries.len())
        .unwrap_or(0);
    let mut prefix_len = active_len;
    for batch in &mut state.pending_batches {
        if batch.category == entry.category {
            batch.entries.push(entry.clone());
            return QueueTicket {
                category: entry.category,
                position: prefix_len + batch.entries.len(),
                queued_at: entry.queued_at,
            };
        }
        prefix_len = prefix_len.saturating_add(batch.entries.len());
    }

    state.pending_batches.push(QueueBatch {
        category: entry.category,
        entries: vec![entry.clone()],
    });
    QueueTicket {
        category: entry.category,
        position: prefix_len + 1,
        queued_at: entry.queued_at,
    }
}

pub fn find_ticket(index: &IndexFile, job_id: &str, task_type: TaskType) -> Option<QueueTicket> {
    let mut position = 1usize;

    if let Some(active_batch) = index.task_queue.active_batch.as_ref() {
        for entry in &active_batch.entries {
            if entry.job_id == job_id && entry.task_type == task_type {
                return Some(QueueTicket {
                    category: entry.category,
                    position,
                    queued_at: entry.queued_at.clone(),
                });
            }
            position = position.saturating_add(1);
        }
    }

    for batch in &index.task_queue.pending_batches {
        for entry in &batch.entries {
            if entry.job_id == job_id && entry.task_type == task_type {
                return Some(QueueTicket {
                    category: entry.category,
                    position,
                    queued_at: entry.queued_at.clone(),
                });
            }
            position = position.saturating_add(1);
        }
    }

    None
}

pub fn find_task_entry(index: &IndexFile, job_id: &str, task_type: TaskType) -> Option<QueueEntry> {
    if let Some(active_batch) = index.task_queue.active_batch.as_ref() {
        if let Some(running) = active_batch.running.as_ref()
            && running.job_id == job_id
            && running.task_type == task_type
        {
            return Some(running.clone());
        }

        for entry in &active_batch.entries {
            if entry.job_id == job_id && entry.task_type == task_type {
                return Some(entry.clone());
            }
        }
    }

    for batch in &index.task_queue.pending_batches {
        for entry in &batch.entries {
            if entry.job_id == job_id && entry.task_type == task_type {
                return Some(entry.clone());
            }
        }
    }

    None
}

pub fn update_queued_entry(
    index: &mut IndexFile,
    job_id: &str,
    task_type: TaskType,
    mut update: impl FnMut(&mut QueueEntry),
) -> bool {
    if let Some(active_batch) = index.task_queue.active_batch.as_mut() {
        for entry in &mut active_batch.entries {
            if entry.job_id == job_id && entry.task_type == task_type {
                update(entry);
                return true;
            }
        }
    }

    for batch in &mut index.task_queue.pending_batches {
        for entry in &mut batch.entries {
            if entry.job_id == job_id && entry.task_type == task_type {
                update(entry);
                return true;
            }
        }
    }

    false
}

pub fn queue_snapshot(repo_root: &Path) -> Result<TaskQueueState, String> {
    IndexStore::new(repo_root).task_queue()
}

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

        requeue_entry(index, &entry, queued_at.clone())?;
        if let Some(active_batch) = index.task_queue.active_batch.as_mut() {
            active_batch.entries.insert(0, entry);
        }
        Ok(())
    })
}

pub fn dispatch_one(repo_root: &Path, state: &mut DispatchState) -> Result<bool, String> {
    let started_at = now_rfc3339()?;
    let Some(work) = reserve_next_entry(repo_root, state, started_at)? else {
        return Ok(false);
    };

    let execution_result = execute_work_item(repo_root, &work.entry);
    let clear_result = clear_running_entry(repo_root, &work.entry);

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

pub fn submit_batch_pipeline_jobs(repo_root: &Path) -> Result<BatchQueueSubmission, String> {
    let queued_at = now_rfc3339()?;
    IndexStore::new(repo_root).with_index_mut(|index| {
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
            let job_dir = PathBuf::from(&job.job_dir);

            if should_enqueue_batch_ffmpeg(index, &job) {
                let entry =
                    build_ffmpeg_entry(&job.job_id, Path::new(&job.source_path), queued_at.clone());
                index.jobs[job_index].mark_ffmpeg_queued(queued_at.clone());
                enqueue_entry(index, entry);
                summary.ffmpeg_queued = summary.ffmpeg_queued.saturating_add(1);
                continue;
            }

            if job.status != JobStatus::Completed {
                continue;
            }

            let audio_files = artifacts::supported_audio_files(&job_dir)?;
            if should_enqueue_batch_stt(index, &job, &audio_files, &job_dir)? {
                let entry = build_stt_entry(&job.job_id, &audio_files, queued_at.clone());
                index.jobs[job_index].enqueue_task(TaskType::Stt, queued_at.clone());
                enqueue_entry(index, entry);
                summary.stt_queued = summary.stt_queued.saturating_add(1);
                continue;
            }

            if should_enqueue_batch_summary(index, &job, &audio_files, &job_dir)? {
                let entry = build_summary_entry(&job.job_id, false, queued_at.clone());
                index.jobs[job_index].enqueue_task(TaskType::Summary, queued_at.clone());
                enqueue_entry(index, entry);
                summary.summary_queued = summary.summary_queued.saturating_add(1);
                continue;
            }

            if should_enqueue_batch_embedding(repo_root, index, &job, &job_dir)? {
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
    index: &IndexFile,
    job: &JobRecord,
    audio_files: &[PathBuf],
    job_dir: &Path,
) -> Result<bool, String> {
    if audio_files.is_empty() || all_transcripts_exist(job_dir, audio_files)? {
        return Ok(false);
    }

    Ok(match job.task(TaskType::Stt).map(|task| task.status) {
        Some(TaskStatus::Queued) => find_ticket(index, &job.job_id, TaskType::Stt).is_none(),
        Some(TaskStatus::Running) => false,
        Some(TaskStatus::Completed) | Some(TaskStatus::Failed) | None => true,
    })
}

fn should_enqueue_batch_summary(
    index: &IndexFile,
    job: &JobRecord,
    audio_files: &[PathBuf],
    job_dir: &Path,
) -> Result<bool, String> {
    if audio_files.is_empty() || !all_transcripts_exist(job_dir, audio_files)? {
        return Ok(false);
    }

    let summary_dir = job_dir.join("summary");
    let summary_file = artifacts::ensure_summary_output_path(&summary_dir, &job.source_file_name)?;
    if summary_file.is_file() {
        return Ok(false);
    }

    Ok(match job.task(TaskType::Summary).map(|task| task.status) {
        Some(TaskStatus::Queued) => find_ticket(index, &job.job_id, TaskType::Summary).is_none(),
        Some(TaskStatus::Running) => false,
        Some(TaskStatus::Completed) | Some(TaskStatus::Failed) | None => true,
    })
}

fn should_enqueue_batch_embedding(
    repo_root: &Path,
    index: &IndexFile,
    job: &JobRecord,
    job_dir: &Path,
) -> Result<bool, String> {
    if matches!(
        job.task(TaskType::Summary).map(|task| task.status),
        Some(TaskStatus::Queued | TaskStatus::Running)
    ) {
        return Ok(false);
    }

    let summary_dir = job_dir.join("summary");
    let summary_file = artifacts::ensure_summary_output_path(&summary_dir, &job.source_file_name)?;
    if !summary_file.is_file() {
        return Ok(false);
    }

    if !embedding_stage::is_embedding_stale(repo_root, job)? {
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

fn all_transcripts_exist(job_dir: &Path, audio_files: &[PathBuf]) -> Result<bool, String> {
    if audio_files.is_empty() {
        return Ok(false);
    }

    let stt_dir = job_dir.join("stt");
    audio_files
        .iter()
        .try_fold(true, |all_present, audio_file| -> Result<bool, String> {
            let transcript = artifacts::transcript_output_path(&stt_dir, audio_file)?;
            Ok(all_present && transcript.is_file())
        })
}

fn reserve_next_entry(
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
                    if is_entry_ready(index, &candidate) {
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
                    let total_batches =
                        1usize.saturating_add(index.task_queue.pending_batches.len());
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

fn is_entry_ready(index: &IndexFile, entry: &QueueEntry) -> bool {
    let Some(job) = index.jobs.iter().find(|job| job.job_id == entry.job_id) else {
        return false;
    };

    match entry.task_type {
        TaskType::Ffmpeg => true,
        TaskType::Stt => job.status == crate::index::JobStatus::Completed,
        TaskType::Summary => summary_inputs_ready(job).unwrap_or(false),
        TaskType::Embedding => summary_output_exists(job).unwrap_or(false),
    }
}

fn summary_inputs_ready(job: &JobRecord) -> Result<bool, String> {
    let job_dir = PathBuf::from(&job.job_dir);
    let audio_files = artifacts::supported_audio_files(&job_dir)?;
    if audio_files.is_empty() {
        return Ok(!artifacts::transcript_text_files(&job_dir.join("stt"))?.is_empty());
    }

    all_transcripts_exist(&job_dir, &audio_files)
}

fn summary_output_exists(job: &JobRecord) -> Result<bool, String> {
    let summary_dir = PathBuf::from(&job.job_dir).join("summary");
    let summary_file = artifacts::ensure_summary_output_path(&summary_dir, &job.source_file_name)?;
    Ok(summary_file.is_file())
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

fn requeue_entry(
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

fn clear_running_entry(repo_root: &Path, entry: &QueueEntry) -> Result<(), String> {
    IndexStore::new(repo_root).with_index_mut(|index| {
        clear_running_entry_in_index(index, entry);
        Ok(())
    })
}

fn clear_running_entry_in_index(index: &mut IndexFile, entry: &QueueEntry) {
    let Some(active_batch) = index.task_queue.active_batch.as_mut() else {
        return;
    };
    if active_batch.running.as_ref().is_some_and(|running| {
        running.job_id == entry.job_id && running.task_type == entry.task_type
    }) {
        active_batch.running = None;
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
                artifacts::supported_audio_files(&PathBuf::from(job.job_dir))?
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{IndexStore, JobRecord};
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
            let work =
                reserve_next_entry(&repo_root, &mut state, "2026-01-01T00:00:10Z".to_string())
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

        let rotated =
            reserve_next_entry(&repo_root, &mut state, "2026-01-01T00:00:11Z".to_string())
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
        let work = reserve_next_entry(&repo_root, &mut state, "2026-01-01T00:00:10Z".to_string())
            .expect("reserve")
            .expect("work item");
        assert_eq!(work.entry.category, QueueCategory::Stt);
        assert_eq!(work.entry.job_id, "job-stt");
    }

    #[test]
    fn submit_batch_pipeline_jobs_enqueues_unfinished_tasks() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let completed_job_dir = store.job_dir("job-complete");
        fs::create_dir_all(completed_job_dir.join("stt")).expect("completed stt dir");
        fs::write(completed_job_dir.join("mono_mix.wav"), "audio").expect("mono mix");
        fs::write(completed_job_dir.join("stt/mono_mix.txt"), "transcript").expect("transcript");

        let mut completed = JobRecord::new(
            "job-complete".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/job-complete.wav"),
            completed_job_dir,
        );
        completed
            .mark_completed(
                "2026-01-01T00:00:01Z".to_string(),
                crate::index::JobOutputs::default(),
            )
            .expect("ffmpeg complete");
        completed.enqueue_task(TaskType::Stt, "2026-01-01T00:00:02Z".to_string());
        completed
            .complete_task(TaskType::Stt, "2026-01-01T00:00:03Z".to_string())
            .expect("stt complete");
        store.insert_job(completed).expect("insert completed");

        let queued = JobRecord::new(
            "job-queued".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/job-queued.wav"),
            store.job_dir("job-queued"),
        );
        store.insert_job(queued).expect("insert queued");

        let result = submit_batch_pipeline_jobs(&repo_root).expect("batch submit");
        assert_eq!(result.total_jobs, 2);
        assert_eq!(result.ffmpeg_queued, 1);
        assert_eq!(result.stt_queued, 0);
        assert_eq!(result.summary_queued, 1);
        assert_eq!(result.embedding_queued, 0);
    }

    #[test]
    fn submit_batch_pipeline_jobs_skips_queued_and_running_tasks() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let job_dir = store.job_dir("job-inflight");
        fs::create_dir_all(job_dir.join("stt")).expect("stt dir");
        fs::write(job_dir.join("mono_mix.wav"), "audio").expect("mono mix");
        fs::write(job_dir.join("stt/mono_mix.txt"), "transcript").expect("transcript");

        let mut job = JobRecord::new(
            "job-inflight".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/job-inflight.wav"),
            job_dir,
        );
        job.mark_completed(
            "2026-01-01T00:00:00Z".to_string(),
            crate::index::JobOutputs::default(),
        )
        .expect("mark ffmpeg completed");
        job.enqueue_task(TaskType::Stt, "2026-01-01T00:00:01Z".to_string());
        job.complete_task(TaskType::Stt, "2026-01-01T00:00:02Z".to_string())
            .expect("stt complete");
        job.enqueue_task(TaskType::Summary, "2026-01-01T00:00:02Z".to_string());
        job.start_task(TaskType::Summary, "2026-01-01T00:00:03Z".to_string())
            .expect("summary running");
        store.insert_job(job).expect("insert inflight");

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

        let result = submit_batch_pipeline_jobs(&repo_root).expect("batch submit");

        assert_eq!(result.total_jobs, 1);
        assert_eq!(result.ffmpeg_queued, 0);
        assert_eq!(result.stt_queued, 0);
        assert_eq!(result.summary_queued, 0);
        assert_eq!(result.embedding_queued, 0);

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
        assert_eq!(queued_entries_after, queued_entries_before);

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

        let result = submit_batch_pipeline_jobs(&repo_root).expect("batch submit");
        assert_eq!(result.total_jobs, 1);
        assert_eq!(result.ffmpeg_queued, 0);
        assert_eq!(result.stt_queued, 1);
        assert_eq!(result.summary_queued, 0);
        assert_eq!(result.embedding_queued, 0);

        let queued_stt_ticket = IndexStore::new(&repo_root)
            .with_index_read(|index| {
                Ok(find_ticket(index, "job-missing-queue-entry", TaskType::Stt))
            })
            .expect("read index");
        assert!(queued_stt_ticket.is_some());
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
        let work = reserve_next_entry(&repo_root, &mut state, "2026-01-01T00:00:10Z".to_string())
            .expect("reserve");
        assert!(work.is_none());
    }

    fn temp_workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!("recordroute-queue-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).expect("temp workspace");
        path
    }
}
