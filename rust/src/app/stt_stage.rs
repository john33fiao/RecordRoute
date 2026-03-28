use super::{
    StageJobDisposition, StageJobSubmission, SttRunSummary, SttTranscriptOutput, artifacts,
    ensure_model_prepared, now_rfc3339, queue, read_line, stages, submit_summary_job,
};
use crate::audio_store::AudioStore;
use crate::index::{
    IndexStore, JobRecord, JobStatus, ModelKind, QueuePayload, TaskStatus, TaskType,
    TranscriptRecord,
};
use crate::whisper::{Toolchain as WhisperToolchain, run_transcription};
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
struct SttCandidate {
    job_id: String,
    source_file_name: String,
    job_dir: PathBuf,
    audio_files: Vec<PathBuf>,
}

pub fn submit_stt_job(
    repo_root: &Path,
    job_id: &str,
    subset_audio_files: Option<Vec<String>>,
) -> Result<StageJobSubmission, String> {
    let index_store = IndexStore::new(repo_root);
    let job = index_store
        .find_job(job_id)?
        .ok_or_else(|| format!("job not found: {job_id}"))?;
    if job.status != JobStatus::Completed {
        return Err(format!("ffmpeg must be completed before stt: {job_id}"));
    }

    let job_dir = index_store.job_dir(job_id);
    let all_audio_files = artifacts::supported_audio_files(&job_dir)?;
    let audio_files = select_subset_audio_files(&all_audio_files, subset_audio_files)?;
    if audio_files.is_empty() {
        return Err(format!("no supported audio files found in job: {job_id}"));
    }

    if let Some(task) = job.task(TaskType::Stt)
        && matches!(task.status, TaskStatus::Queued | TaskStatus::Running)
    {
        let requested_audio_files = normalized_audio_files(&audio_files);
        let existing_entry = index_store.with_index_read(|index| {
            Ok(queue::find_task_entry(index, &job.job_id, TaskType::Stt))
        })?;

        match existing_entry {
            Some(entry) if stt_payload_matches(&entry, &requested_audio_files) => {
                let queue = if task.status == TaskStatus::Queued {
                    index_store.with_index_read(|index| {
                        Ok(queue::find_ticket(index, &job.job_id, TaskType::Stt))
                    })?
                } else {
                    None
                };
                return Ok(StageJobSubmission {
                    job,
                    disposition: StageJobDisposition::Deduplicated,
                    planned_audio_files: audio_files,
                    queue,
                });
            }
            Some(_) => {
                return Err(format!(
                    "stt already queued or running with a different audio selection: {job_id}"
                ));
            }
            None if task.status == TaskStatus::Running => {
                return Err(format!(
                    "stt task is marked running but queue entry is missing: {job_id}"
                ));
            }
            None => {}
        }
    }

    let all_transcripts_exist = all_transcripts_exist(&index_store, job_id, &audio_files)?;
    if all_transcripts_exist {
        return Ok(StageJobSubmission {
            job,
            disposition: StageJobDisposition::Reused,
            planned_audio_files: audio_files,
            queue: None,
        });
    }

    let queued_at = now_rfc3339()?;
    let entry = queue::build_stt_entry(job_id, &audio_files, queued_at.clone());
    let (job, ticket) = index_store.with_index_mut(|index| {
        let job_index = index
            .jobs
            .iter()
            .position(|record| record.job_id == job_id)
            .ok_or_else(|| format!("job not found in index: {job_id}"))?;
        {
            let job = &mut index.jobs[job_index];
            job.enqueue_task(TaskType::Stt, queued_at.clone());
        }
        let ticket = queue::enqueue_entry(index, entry);
        Ok((index.jobs[job_index].clone(), ticket))
    })?;
    Ok(StageJobSubmission {
        job,
        disposition: StageJobDisposition::Submitted,
        planned_audio_files: audio_files,
        queue: Some(ticket),
    })
}

pub fn execute_stt_job(
    repo_root: &Path,
    job_id: &str,
    audio_files: &[PathBuf],
) -> Result<JobRecord, String> {
    let index_store = IndexStore::new(repo_root);
    let job = index_store
        .find_job(job_id)?
        .ok_or_else(|| format!("job not found: {job_id}"))?;
    let audio_store = AudioStore::new(repo_root)?;
    let stt_dir = audio_store.spool_root().join("stt").join(job_id);
    let result = (|| -> Result<(), String> {
        if audio_files.is_empty() {
            return Err(format!("no audio files selected for stt: {job_id}"));
        }
        ensure_model_prepared(repo_root, ModelKind::Whisper)?;
        let toolchain = WhisperToolchain::discover(repo_root)?;
        fs::create_dir_all(&stt_dir).map_err(|error| {
            format!(
                "failed to create stt directory {}: {error}",
                stt_dir.display()
            )
        })?;

        for audio in audio_files {
            let absolute_audio = if audio.is_absolute() {
                audio.clone()
            } else {
                index_store.job_dir(job_id).join(audio)
            };
            let transcript = artifacts::transcript_output_path(&stt_dir, &absolute_audio)?;
            run_transcription(&toolchain, &absolute_audio, &transcript)?;
            let file_name = artifacts::transcript_file_name(&absolute_audio)?;
            let transcript_id = artifacts::transcript_id_from_file_name(&file_name)?;
            let text = fs::read_to_string(&transcript).map_err(|error| {
                format!("failed to read transcript {}: {error}", transcript.display())
            })?;
            index_store.upsert_transcript(&TranscriptRecord {
                job_id: job_id.to_string(),
                transcript_id,
                file_name,
                text,
            })?;
        }
        Ok(())
    })();

    match result {
        Ok(()) => {
            let completed =
                stages::finalize_task_success(repo_root, job, TaskType::Stt, now_rfc3339()?)?;
            if let Err(error) = submit_summary_job(repo_root, &completed.job_id, false) {
                let _ = stages::record_followup_submission_failure(
                    repo_root,
                    &completed.job_id,
                    TaskType::Summary,
                    error,
                );
            }
            Ok(completed)
        }
        Err(error) => {
            stages::finalize_task_failure(
                repo_root,
                job,
                TaskType::Stt,
                now_rfc3339()?,
                error.clone(),
            )?;
            Err(error)
        }
    }
}

pub fn run_stt_with_repo_root(
    repo_root: &Path,
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
) -> Result<SttRunSummary, String> {
    let index_store = IndexStore::new(repo_root);
    let candidates = collect_stt_candidates(&index_store)?;
    if candidates.is_empty() {
        return Err("no job folders with supported audio files found in db/index.json".to_string());
    }

    let selected = select_stt_candidate(&candidates, reader, writer)?;
    let submission = submit_stt_job(repo_root, &selected.job_id, None)?;
    if submission.should_execute() {
        queue::dispatch_until_task_terminal(repo_root, &selected.job_id, TaskType::Stt)?;
    } else if submission.deduplicated() {
        stages::wait_for_task_completion(repo_root, &selected.job_id, TaskType::Stt)?;
    }

    let stt_dir = AudioStore::new(repo_root)?
        .spool_root()
        .join("stt")
        .join(&selected.job_id);
    let transcripts = selected
        .audio_files
        .iter()
        .map(|audio_file| {
            let text_path = artifacts::transcript_output_path(&stt_dir, audio_file)?;
            Ok(SttTranscriptOutput {
                source_path: audio_file.clone(),
                text_path,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(SttRunSummary {
        job_id: selected.job_id,
        job_dir: selected.job_dir,
        stt_dir,
        transcripts,
    })
}

fn collect_stt_candidates(index_store: &IndexStore) -> Result<Vec<SttCandidate>, String> {
    let mut candidates = Vec::new();

    for job in index_store.list_jobs()? {
        let job_dir = index_store.job_dir(&job.job_id);
        if !job_dir.is_dir() {
            continue;
        }

        let audio_files = artifacts::supported_audio_files(&job_dir)?;
        if audio_files.is_empty() {
            continue;
        }

        candidates.push(SttCandidate {
            job_id: job.job_id,
            source_file_name: job.source_file_name,
            job_dir,
            audio_files,
        });
    }

    Ok(candidates)
}

fn select_stt_candidate(
    candidates: &[SttCandidate],
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
) -> Result<SttCandidate, String> {
    loop {
        writeln!(writer, "Select STT folder:").map_err(|error| error.to_string())?;
        for (index, candidate) in candidates.iter().enumerate() {
            writeln!(
                writer,
                "{}. {} | {} | {}",
                index + 1,
                candidate.job_id,
                candidate.source_file_name,
                candidate.job_dir.display()
            )
            .map_err(|error| error.to_string())?;
        }
        write!(writer, "Enter number: ").map_err(|error| error.to_string())?;
        writer.flush().map_err(|error| error.to_string())?;

        let line = read_line(reader, "failed to read stt folder selection")?;
        match line.trim().parse::<usize>() {
            Ok(selection) if selection >= 1 && selection <= candidates.len() => {
                return Ok(candidates[selection - 1].clone());
            }
            _ => {
                writeln!(
                    writer,
                    "Invalid selection. Enter a number between 1 and {}.",
                    candidates.len()
                )
                .map_err(|error| error.to_string())?;
            }
        }
    }
}

fn select_subset_audio_files(
    all_audio_files: &[PathBuf],
    subset_audio_files: Option<Vec<String>>,
) -> Result<Vec<PathBuf>, String> {
    let Some(subset_audio_files) = subset_audio_files else {
        return Ok(all_audio_files.to_vec());
    };
    if subset_audio_files.is_empty() {
        return Ok(all_audio_files.to_vec());
    }

    let mut selected = Vec::new();
    for requested in subset_audio_files {
        let found = all_audio_files.iter().find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == requested)
        });
        let Some(path) = found else {
            return Err(format!("requested stt subset file not found: {requested}"));
        };
        if !selected.contains(path) {
            selected.push(path.clone());
        }
    }
    Ok(selected)
}

fn normalized_audio_files(audio_files: &[PathBuf]) -> Vec<String> {
    audio_files
        .iter()
        .map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

fn stt_payload_matches(entry: &crate::index::QueueEntry, requested_audio_files: &[String]) -> bool {
    match &entry.payload {
        QueuePayload::Stt { audio_files } => audio_files == requested_audio_files,
        _ => false,
    }
}

fn all_transcripts_exist(
    index_store: &IndexStore,
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
            Ok(all_present && index_store.find_transcript(job_id, &transcript_id)?.is_some())
        })
}
