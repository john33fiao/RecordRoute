use super::{SttProgressResponse, SttTranscriptText};
use crate::app::artifacts;
use crate::index::{IndexStore, TaskStatus, TaskType};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn collect_job_files(repo_root: &Path, job_id: &str) -> Result<Vec<String>, String> {
    let store = IndexStore::new(repo_root);
    let job = store
        .find_job(job_id)?
        .ok_or_else(|| format!("job not found: {job_id}"))?;
    artifacts::collect_job_files(&PathBuf::from(job.job_dir), &job.source_file_name)
}

pub(crate) fn read_stt_transcripts(
    repo_root: &Path,
    job_id: &str,
) -> Result<Vec<SttTranscriptText>, String> {
    let store = IndexStore::new(repo_root);
    let job = store
        .find_job(job_id)?
        .ok_or_else(|| format!("job not found: {job_id}"))?;
    let stt_dir = PathBuf::from(job.job_dir).join("stt");
    if !stt_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut transcripts = Vec::new();
    for entry in fs::read_dir(&stt_dir)
        .map_err(|error| format!("failed to read {}: {error}", stt_dir.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to read stt entry: {error}"))?;
        let path = entry.path();
        if !path.is_file()
            || path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_none_or(|ext| ext != "txt")
        {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let transcript_id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| {
                format!(
                    "failed to parse transcript id from file: {}",
                    path.display()
                )
            })?
            .to_string();
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read transcript {}: {error}", path.display()))?;
        transcripts.push(SttTranscriptText {
            transcript_id,
            file_name: file_name.to_string(),
            text,
        });
    }
    transcripts.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(transcripts)
}

pub(crate) fn read_stt_progress_snapshot(
    repo_root: &Path,
    job_id: &str,
) -> Result<SttProgressResponse, String> {
    let store = IndexStore::new(repo_root);
    let job = store
        .find_job(job_id)?
        .ok_or_else(|| format!("job not found: {job_id}"))?;
    let task = job.task(TaskType::Stt).cloned();
    let job_dir = PathBuf::from(job.job_dir);
    let total_files = artifacts::count_supported_audio_files(&job_dir)?;
    let completed_files = count_stt_transcript_files(&job_dir.join("stt"))?;
    let phase = match task.as_ref().map(|record| record.status) {
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
) -> Result<SttTranscriptText, String> {
    sanitize_transcript_id(transcript_id)?;
    let transcripts = read_stt_transcripts(repo_root, job_id)?;
    transcripts
        .into_iter()
        .find(|transcript| transcript.transcript_id == transcript_id)
        .ok_or_else(|| format!("transcript not found: {transcript_id}"))
}

pub(crate) fn read_summary_text(repo_root: &Path, job_id: &str) -> Result<String, String> {
    let store = IndexStore::new(repo_root);
    let job = store
        .find_job(job_id)?
        .ok_or_else(|| format!("job not found: {job_id}"))?;
    let summary_dir = PathBuf::from(job.job_dir).join("summary");
    artifacts::read_summary_text(&summary_dir, &job.source_file_name)
}

pub(crate) fn read_job_file(
    repo_root: &Path,
    job_id: &str,
    file_name: &str,
) -> Result<Vec<u8>, String> {
    let store = IndexStore::new(repo_root);
    let job = store
        .find_job(job_id)?
        .ok_or_else(|| format!("job not found: {job_id}"))?;

    let path = sanitize_file_name(file_name)?;
    let allowed_prefix = ["stt/", "summary/"];
    let is_allowed = path == "mono_mix.wav"
        || path.starts_with("channel_")
        || allowed_prefix.iter().any(|prefix| path.starts_with(prefix));
    if !is_allowed {
        return Err(format!("file not allowed: {file_name}"));
    }

    let job_dir = PathBuf::from(&job.job_dir);
    if path == format!("summary/{}", artifacts::summary_file_name()) {
        let summary_dir = job_dir.join("summary");
        let canonical = artifacts::ensure_summary_output_path(&summary_dir, &job.source_file_name)?;
        return fs::read(&canonical)
            .map_err(|error| format!("file not found: {} ({error})", canonical.display()));
    }

    let absolute = job_dir.join(path);
    fs::read(&absolute).map_err(|error| format!("file not found: {} ({error})", absolute.display()))
}

fn count_stt_transcript_files(stt_dir: &Path) -> Result<usize, String> {
    if !stt_dir.is_dir() {
        return Ok(0);
    }

    let mut count = 0usize;
    for entry in fs::read_dir(stt_dir)
        .map_err(|error| format!("failed to read {}: {error}", stt_dir.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to read stt entry: {error}"))?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"))
        {
            count = count.saturating_add(1);
        }
    }

    Ok(count)
}

fn calculate_progress_percent(completed_files: usize, total_files: usize) -> u8 {
    if total_files == 0 {
        return 0;
    }
    let ratio = completed_files.saturating_mul(100) / total_files;
    ratio.min(100) as u8
}

fn sanitize_file_name(file_name: &str) -> Result<&str, String> {
    if file_name.contains('\\') || file_name.starts_with('/') || file_name.contains("..") {
        return Err("invalid file path".to_string());
    }
    Ok(file_name)
}

fn sanitize_transcript_id(transcript_id: &str) -> Result<(), String> {
    if transcript_id.is_empty()
        || transcript_id.contains('\\')
        || transcript_id.contains('/')
        || transcript_id.contains("..")
    {
        return Err("invalid transcript id".to_string());
    }
    Ok(())
}
