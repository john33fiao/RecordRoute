use super::types::{SttProgressResponse, SttTranscriptText};
use crate::error::{AppError, AppResult};
use crate::index::{IndexStore, JobRecord, SummaryRecord, TaskStatus, TaskType};
use std::path::Path;

pub(crate) fn collect_job_files(repo_root: &Path, job_id: &str) -> AppResult<Vec<String>> {
    let store = IndexStore::new(repo_root);
    let _job = find_job(repo_root, job_id)?;
    let mut files = store
        .list_audio_artifacts(job_id)
        .map_err(AppError::internal)?
        .into_iter()
        .map(|artifact| artifact.logical_name)
        .collect::<Vec<_>>();
    files.extend(
        store
            .list_transcripts(job_id)
            .map_err(AppError::internal)?
            .into_iter()
            .map(|record| format!("stt/{}", record.file_name)),
    );
    if let Some(summary) = store.get_summary(job_id).map_err(AppError::internal)? {
        files.push(format!("summary/{}", summary.file_name));
    }
    files.sort();
    Ok(files)
}

pub(crate) fn read_stt_transcripts(
    repo_root: &Path,
    job_id: &str,
) -> AppResult<Vec<SttTranscriptText>> {
    let _job = find_job(repo_root, job_id)?;
    let transcripts = IndexStore::new(repo_root)
        .list_transcripts(job_id)
        .map_err(AppError::internal)?
        .into_iter()
        .map(|record| SttTranscriptText {
            transcript_id: record.transcript_id,
            file_name: record.file_name,
            text: record.text,
        })
        .collect();
    Ok(transcripts)
}

pub(crate) fn read_stt_progress_snapshot(
    repo_root: &Path,
    job_id: &str,
) -> AppResult<SttProgressResponse> {
    let store = IndexStore::new(repo_root);
    let job = find_job(repo_root, job_id)?;
    let task = job.task(TaskType::Stt).cloned();
    let total_files = store
        .list_audio_artifacts(job_id)
        .map_err(AppError::internal)?
        .len();
    let completed_files = store
        .count_transcripts(job_id)
        .map_err(AppError::internal)?;
    let phase = match task.as_ref().map(|record| record.status) {
        Some(TaskStatus::Queued) => "queued",
        Some(TaskStatus::Running) => "running",
        Some(TaskStatus::Completed) => "completed",
        Some(TaskStatus::Failed) => "failed",
        None => "idle",
    }
    .to_string();
    let progress_percent = match task.as_ref().map(|record| record.status) {
        Some(TaskStatus::Completed) => 100,
        _ => calculate_progress_percent(completed_files, total_files),
    };

    Ok(SttProgressResponse {
        job_id: job_id.to_string(),
        task,
        phase,
        total_files,
        completed_files,
        progress_percent,
    })
}

pub(crate) fn read_stt_transcript(
    repo_root: &Path,
    job_id: &str,
    transcript_id: &str,
) -> AppResult<SttTranscriptText> {
    sanitize_transcript_id(transcript_id)?;
    IndexStore::new(repo_root)
        .find_transcript(job_id, transcript_id)
        .map_err(AppError::internal)?
        .map(|record| SttTranscriptText {
            transcript_id: record.transcript_id,
            file_name: record.file_name,
            text: record.text,
        })
        .ok_or_else(|| AppError::not_found(format!("transcript not found: {transcript_id}")))
}

pub(crate) fn read_summary_text(repo_root: &Path, job_id: &str) -> AppResult<SummaryRecord> {
    let _job = find_job(repo_root, job_id)?;
    IndexStore::new(repo_root)
        .get_summary(job_id)
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found(format!("summary not found for job: {job_id}")))
}

pub(crate) fn read_job_file(repo_root: &Path, job_id: &str, file_name: &str) -> AppResult<Vec<u8>> {
    let store = IndexStore::new(repo_root);
    let _job = find_job(repo_root, job_id)?;

    let path = sanitize_file_name(file_name)?;
    if path.starts_with("summary/") {
        let summary = store
            .get_summary(job_id)
            .map_err(AppError::internal)?
            .ok_or_else(|| AppError::not_found(format!("summary not found for job: {job_id}")))?;
        if path == format!("summary/{}", summary.file_name) {
            return Ok(summary.text.into_bytes());
        }
        return Err(AppError::not_found(format!("file not found: {path}")));
    }
    if let Some(transcript_name) = path.strip_prefix("stt/") {
        let transcript = store
            .list_transcripts(job_id)
            .map_err(AppError::internal)?
            .into_iter()
            .find(|record| record.file_name == transcript_name)
            .ok_or_else(|| AppError::not_found(format!("file not found: {path}")))?;
        return Ok(transcript.text.into_bytes());
    }

    let artifact = store
        .list_audio_artifacts(job_id)
        .map_err(AppError::internal)?
        .into_iter()
        .find(|artifact| artifact.logical_name == path)
        .ok_or_else(|| AppError::not_found(format!("file not found: {path}")))?;
    store
        .audio_store()
        .and_then(|audio_store| audio_store.read_bytes(&artifact.storage_key))
        .map_err(AppError::not_found)
}

fn find_job(repo_root: &Path, job_id: &str) -> AppResult<JobRecord> {
    IndexStore::new(repo_root)
        .find_job(job_id)
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found(format!("job not found: {job_id}")))
}

fn calculate_progress_percent(completed_files: usize, total_files: usize) -> u8 {
    if total_files == 0 {
        return 0;
    }
    let ratio = completed_files.saturating_mul(100) / total_files;
    ratio.min(100) as u8
}

fn sanitize_file_name(file_name: &str) -> AppResult<&str> {
    if file_name.contains('\\') || file_name.starts_with('/') || file_name.contains("..") {
        return Err(AppError::bad_request("invalid file path"));
    }
    Ok(file_name)
}

fn sanitize_transcript_id(transcript_id: &str) -> AppResult<()> {
    if transcript_id.is_empty()
        || transcript_id.contains('\\')
        || transcript_id.contains('/')
        || transcript_id.contains("..")
    {
        return Err(AppError::bad_request("invalid transcript id"));
    }
    Ok(())
}
