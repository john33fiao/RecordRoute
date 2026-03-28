use super::{
    StageJobDisposition, StageJobSubmission, SummaryRunSummary, artifacts, ensure_model_prepared,
    now_rfc3339, queue, read_line, stages, submit_summary_embedding_job,
};
use crate::index::{IndexStore, JobRecord, ModelKind, QueuePayload, TaskStatus, TaskType};
use crate::llama::{Toolchain as LlamaToolchain, run_summary_generation};
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
struct SummaryCandidate {
    job_id: String,
    source_file_name: String,
    job_dir: PathBuf,
    transcript_files: Vec<PathBuf>,
}

pub fn submit_summary_job(
    repo_root: &Path,
    job_id: &str,
    force_regenerate: bool,
) -> Result<StageJobSubmission, String> {
    let index_store = IndexStore::new(repo_root);
    let job = index_store
        .find_job(job_id)?
        .ok_or_else(|| format!("job not found: {job_id}"))?;
    let job_dir = PathBuf::from(&job.job_dir);
    let summary_dir = job_dir.join("summary");
    let summary_file = artifacts::ensure_summary_output_path(&summary_dir, &job.source_file_name)?;

    if let Some(task) = job.task(TaskType::Summary)
        && matches!(task.status, TaskStatus::Queued | TaskStatus::Running)
    {
        let existing_entry = index_store.with_index_read(|index| {
            Ok(queue::find_task_entry(
                index,
                &job.job_id,
                TaskType::Summary,
            ))
        })?;

        match existing_entry {
            Some(entry) if summary_payload_satisfies(&entry, force_regenerate) => {
                let queue = if task.status == TaskStatus::Queued {
                    index_store.with_index_read(|index| {
                        Ok(queue::find_ticket(index, &job.job_id, TaskType::Summary))
                    })?
                } else {
                    None
                };
                return Ok(StageJobSubmission {
                    job,
                    disposition: StageJobDisposition::Deduplicated,
                    planned_audio_files: Vec::new(),
                    queue,
                });
            }
            Some(entry)
                if task.status == TaskStatus::Queued
                    && force_regenerate
                    && matches!(
                        entry.payload,
                        QueuePayload::Summary {
                            force_regenerate: false
                        }
                    ) =>
            {
                let (job, ticket) = index_store.with_index_mut(|index| {
                    if !queue::update_queued_entry(index, job_id, TaskType::Summary, |entry| {
                        entry.payload = QueuePayload::Summary {
                            force_regenerate: true,
                        };
                    }) {
                        return Err(format!(
                            "summary task is marked queued but queue entry is missing: {job_id}"
                        ));
                    }

                    let job = index
                        .jobs
                        .iter()
                        .find(|record| record.job_id == job_id)
                        .cloned()
                        .ok_or_else(|| format!("job not found in index: {job_id}"))?;
                    let ticket =
                        queue::find_ticket(index, job_id, TaskType::Summary).ok_or_else(|| {
                            format!("summary queue ticket missing after upgrade: {job_id}")
                        })?;
                    Ok((job, ticket))
                })?;
                return Ok(StageJobSubmission {
                    job,
                    disposition: StageJobDisposition::Submitted,
                    planned_audio_files: Vec::new(),
                    queue: Some(ticket),
                });
            }
            Some(_) => {
                return Err(format!(
                    "summary already running with different force_regenerate semantics: {job_id}"
                ));
            }
            None if task.status == TaskStatus::Running => {
                return Err(format!(
                    "summary task is marked running but queue entry is missing: {job_id}"
                ));
            }
            None => {}
        }
    }
    if !force_regenerate && summary_file.is_file() {
        return Ok(StageJobSubmission {
            job,
            disposition: StageJobDisposition::Reused,
            planned_audio_files: Vec::new(),
            queue: None,
        });
    }

    if !summary_prerequisites_ready(&job_dir)? {
        return Err(format!("stt must be completed before summary: {job_id}"));
    }

    let queued_at = now_rfc3339()?;
    let entry = queue::build_summary_entry(job_id, force_regenerate, queued_at.clone());
    let (job, ticket) = index_store.with_index_mut(|index| {
        let job_index = index
            .jobs
            .iter()
            .position(|record| record.job_id == job_id)
            .ok_or_else(|| format!("job not found in index: {job_id}"))?;
        {
            let job = &mut index.jobs[job_index];
            job.enqueue_task(TaskType::Summary, queued_at.clone());
        }
        let ticket = queue::enqueue_entry(index, entry);
        Ok((index.jobs[job_index].clone(), ticket))
    })?;
    Ok(StageJobSubmission {
        job,
        disposition: StageJobDisposition::Submitted,
        planned_audio_files: Vec::new(),
        queue: Some(ticket),
    })
}

pub fn execute_summary_job(
    repo_root: &Path,
    job_id: &str,
    force_regenerate: bool,
) -> Result<JobRecord, String> {
    let index_store = IndexStore::new(repo_root);
    let job = index_store
        .find_job(job_id)?
        .ok_or_else(|| format!("job not found: {job_id}"))?;
    let job_dir = PathBuf::from(&job.job_dir);
    let stt_dir = job_dir.join("stt");
    let summary_dir = job_dir.join("summary");
    let result = (|| -> Result<(), String> {
        let transcript_files = artifacts::transcript_text_files(&stt_dir)?;
        if transcript_files.is_empty() {
            return Err(format!("no transcripts found for summary: {job_id}"));
        }

        fs::create_dir_all(&summary_dir).map_err(|error| {
            format!(
                "failed to create summary directory {}: {error}",
                summary_dir.display()
            )
        })?;
        let summary_file =
            artifacts::ensure_summary_output_path(&summary_dir, &job.source_file_name)?;
        if summary_file.is_file() && !force_regenerate {
            return Ok(());
        }

        let prompt_file = artifacts::summary_prompt_file_path(&summary_dir, &job.source_file_name)?;
        let prompt = build_summary_prompt(&transcript_files)?;
        fs::write(&prompt_file, prompt).map_err(|error| {
            format!(
                "failed to write summary prompt {}: {error}",
                prompt_file.display()
            )
        })?;
        ensure_model_prepared(repo_root, ModelKind::Llama)?;
        let toolchain = LlamaToolchain::discover(repo_root)?;
        let generation_result = run_summary_generation(&toolchain, &prompt_file, &summary_file);
        let _ = fs::remove_file(&prompt_file);
        generation_result?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            let completed =
                stages::finalize_task_success(repo_root, job, TaskType::Summary, now_rfc3339()?)?;
            if let Err(error) = submit_summary_embedding_job(repo_root, &completed.job_id) {
                let _ = stages::record_followup_submission_failure(
                    repo_root,
                    &completed.job_id,
                    TaskType::Embedding,
                    error,
                );
            }
            Ok(completed)
        }
        Err(error) => {
            stages::finalize_task_failure(
                repo_root,
                job,
                TaskType::Summary,
                now_rfc3339()?,
                error.clone(),
            )?;
            Err(error)
        }
    }
}

pub fn run_summary_with_repo_root(
    repo_root: &Path,
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
) -> Result<SummaryRunSummary, String> {
    let index_store = IndexStore::new(repo_root);
    let candidates = collect_summary_candidates(&index_store)?;
    if candidates.is_empty() {
        return Err("no job folders with transcription files found in db/index.json".to_string());
    }

    let selected = select_summary_candidate(&candidates, reader, writer)?;
    let submission = submit_summary_job(repo_root, &selected.job_id, false)?;
    if submission.should_execute() {
        queue::dispatch_until_task_terminal(repo_root, &selected.job_id, TaskType::Summary)?;
        if let Some(task) = IndexStore::new(repo_root)
            .find_job(&selected.job_id)?
            .and_then(|job| job.task(TaskType::Embedding).cloned())
            && matches!(task.status, TaskStatus::Queued | TaskStatus::Running)
        {
            queue::dispatch_until_task_terminal(repo_root, &selected.job_id, TaskType::Embedding)?;
        }
    } else if submission.deduplicated() {
        stages::wait_for_task_completion(repo_root, &selected.job_id, TaskType::Summary)?;
    }

    let summary_dir = selected.job_dir.join("summary");
    let summary_file =
        artifacts::ensure_summary_output_path(&summary_dir, &selected.source_file_name)?;
    Ok(SummaryRunSummary {
        job_id: selected.job_id,
        job_dir: selected.job_dir,
        summary_dir,
        summary_file,
    })
}

fn collect_summary_candidates(index_store: &IndexStore) -> Result<Vec<SummaryCandidate>, String> {
    let mut candidates = Vec::new();

    for job in index_store.list_jobs()? {
        let job_dir = PathBuf::from(&job.job_dir);
        if !job_dir.is_dir() {
            continue;
        }

        let stt_dir = job_dir.join("stt");
        let transcript_files = artifacts::transcript_text_files(&stt_dir)?;
        if transcript_files.is_empty() {
            continue;
        }

        candidates.push(SummaryCandidate {
            job_id: job.job_id,
            source_file_name: job.source_file_name,
            job_dir,
            transcript_files,
        });
    }

    Ok(candidates)
}

fn select_summary_candidate(
    candidates: &[SummaryCandidate],
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
) -> Result<SummaryCandidate, String> {
    loop {
        writeln!(writer, "Select summary folder:").map_err(|error| error.to_string())?;
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

        let line = read_line(reader, "failed to read summary folder selection")?;
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

fn build_summary_prompt(transcript_files: &[PathBuf]) -> Result<String, String> {
    let mut prompt = String::from(
        "당신은 한국어 음성 내용을 간단히 정리하는 도우미다.\n\
다음 입력은 하나의 녹음에서 나온 STT 결과들이며 파일마다 중복이 있을 수 있다.\n\
- 여러 파일에 겹치는 내용은 병합하고 중복 표현은 제거한다.\n\
- 확인할 수 없는 내용은 추측하지 않는다.\n\
- 출력은 한국어 Markdown으로만 작성하고 첫 줄은 반드시 `## 요약`으로 시작한다.\n\
- 통화, 회의, 음성 메모 등 입력 성격에 맞게 짧고 자연스럽게 정리한다.\n\
- 실행 항목이나 후속 조치가 있으면 마지막에 bullet list로 덧붙인다.\n\
- 채널별 파일명은 나열하지 않는다.\n",
    );

    for transcript_file in transcript_files {
        let label = transcript_file
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                format!(
                    "transcript file does not have a valid file name: {}",
                    transcript_file.display()
                )
            })?;
        let content = fs::read_to_string(transcript_file).map_err(|error| {
            format!(
                "failed to read transcript file {}: {error}",
                transcript_file.display()
            )
        })?;

        prompt.push_str("\n\n[");
        prompt.push_str(label);
        prompt.push_str("]\n");
        prompt.push_str(content.trim());
    }

    Ok(prompt)
}

fn summary_payload_satisfies(entry: &crate::index::QueueEntry, requested_force: bool) -> bool {
    match &entry.payload {
        QueuePayload::Summary { force_regenerate } => *force_regenerate || !requested_force,
        _ => false,
    }
}

fn summary_prerequisites_ready(job_dir: &Path) -> Result<bool, String> {
    let stt_dir = job_dir.join("stt");
    let transcript_files = artifacts::transcript_text_files(&stt_dir)?;
    if transcript_files.is_empty() {
        return Ok(false);
    }

    let audio_files = artifacts::supported_audio_files(job_dir)?;
    if audio_files.is_empty() {
        return Ok(true);
    }

    audio_files
        .iter()
        .try_fold(true, |all_present, audio_file| -> Result<bool, String> {
            let transcript = artifacts::transcript_output_path(&stt_dir, audio_file)?;
            Ok(all_present && transcript.is_file())
        })
}
