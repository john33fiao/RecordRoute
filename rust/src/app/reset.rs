use super::now_rfc3339;
use crate::audio_store::AudioStore;
use crate::error::{AppError, AppResult};
use crate::index::{IndexFile, IndexStore, JobRecord, JobResetSelection, TaskStatus};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub(crate) const RESET_BY_USER_MESSAGE: &str = "reset by user";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobResetResult {
    pub job: JobRecord,
    pub deleted: JobResetSelection,
}

pub fn reset_job(
    repo_root: &Path,
    job_id: &str,
    selection: JobResetSelection,
) -> AppResult<JobResetResult> {
    let selection = selection.normalized();
    if !selection.any_selected() {
        return Err(AppError::bad_request(
            "at least one stage must be selected for reset",
        ));
    }

    let store = IndexStore::new(repo_root);
    let index = store.read_index().map_err(AppError::internal)?;
    ensure_queue_idle(&index)?;
    if !index.jobs.iter().any(|job| job.job_id == job_id) {
        return Err(AppError::not_found(format!("job not found: {job_id}")));
    }

    let job_dir = store.job_dir(job_id);
    let staged_ffmpeg_dir = if selection.ffmpeg {
        stage_job_dir(&job_dir).map_err(AppError::internal)?
    } else {
        None
    };

    let finished_at = now_rfc3339().map_err(AppError::internal)?;
    let reset_result =
        store.reset_job_stages(job_id, selection, finished_at, RESET_BY_USER_MESSAGE);

    match reset_result {
        Ok(job) => {
            cleanup_spool_dirs(repo_root, job_id, selection);
            if let Some(staged_dir) = staged_ffmpeg_dir {
                let _ = fs::remove_dir_all(staged_dir);
            }
            Ok(JobResetResult {
                job,
                deleted: selection,
            })
        }
        Err(error) => {
            if let Some(staged_dir) = staged_ffmpeg_dir {
                let _ = restore_job_dir(&staged_dir, &job_dir);
            }
            Err(AppError::internal(error))
        }
    }
}

fn ensure_queue_idle(index: &IndexFile) -> AppResult<()> {
    let queue_has_entries =
        index.task_queue.active_batch.is_some() || !index.task_queue.pending_batches.is_empty();
    let task_has_inflight = index.jobs.iter().any(|job| {
        job.tasks
            .iter()
            .any(|task| matches!(task.status, TaskStatus::Queued | TaskStatus::Running))
    });
    if queue_has_entries || task_has_inflight {
        return Err(AppError::bad_request(
            "job reset requires an empty queue and no queued/running tasks",
        ));
    }
    Ok(())
}

fn stage_job_dir(job_dir: &Path) -> Result<Option<PathBuf>, String> {
    if !job_dir.exists() {
        return Ok(None);
    }
    let parent = job_dir
        .parent()
        .ok_or_else(|| format!("job directory has no parent path: {}", job_dir.display()))?;
    let file_name = job_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "job directory has no valid file name: {}",
                job_dir.display()
            )
        })?;
    let staged_dir = parent.join(format!(".{file_name}.reset-{}", Uuid::now_v7()));
    fs::rename(job_dir, &staged_dir).map_err(|error| {
        format!(
            "failed to stage ffmpeg artifacts {} -> {}: {error}",
            job_dir.display(),
            staged_dir.display()
        )
    })?;
    Ok(Some(staged_dir))
}

fn restore_job_dir(staged_dir: &Path, original_dir: &Path) -> Result<(), String> {
    if !staged_dir.exists() {
        return Ok(());
    }
    fs::rename(staged_dir, original_dir).map_err(|error| {
        format!(
            "failed to restore ffmpeg artifacts {} -> {}: {error}",
            staged_dir.display(),
            original_dir.display()
        )
    })
}

fn cleanup_spool_dirs(repo_root: &Path, job_id: &str, selection: JobResetSelection) {
    let Ok(audio_store) = AudioStore::new(repo_root) else {
        return;
    };
    let spool_root = audio_store.spool_root();
    if selection.stt {
        let _ = fs::remove_dir_all(spool_root.join("stt").join(job_id));
    }
    if selection.summary {
        let _ = fs::remove_dir_all(spool_root.join("summary").join(job_id));
    }
}
