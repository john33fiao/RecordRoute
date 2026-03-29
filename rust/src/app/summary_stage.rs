use super::{
    StageJobDisposition, StageJobSubmission, SummaryRunSummary, artifacts, ensure_model_prepared,
    now_rfc3339, queue, read_line, stages, submit_summary_embedding_job,
};
use crate::audio_store::AudioStore;
use crate::error::{AppError, AppResult};
use crate::index::{
    IndexStore, JobRecord, ModelKind, QueuePayload, SummaryRecord, TaskStatus, TaskType,
    TranscriptRecord,
};
use crate::llama::{Toolchain as LlamaToolchain, run_summary_generation};
use crate::whisper::normalize_keywords;
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

const MAX_SUMMARY_KEYWORDS: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SummaryCandidate {
    job_id: String,
    source_file_name: String,
    job_dir: PathBuf,
    transcript_files: Vec<String>,
}

enum SummaryExecutionResult {
    ReusedExisting,
    Generated {
        summary: SummaryRecord,
        auto_keywords: Vec<String>,
    },
}

pub fn submit_summary_job(
    repo_root: &Path,
    job_id: &str,
    force_regenerate: bool,
) -> AppResult<StageJobSubmission> {
    let index_store = IndexStore::new(repo_root);
    let job = index_store
        .find_job(job_id)
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found(format!("job not found: {job_id}")))?;
    let job_dir = index_store.job_dir(job_id);

    if let Some(submission) = stages::resolve_inflight_stage_submission(
        &index_store,
        &job,
        TaskType::Summary,
        Vec::new(),
        |existing_entry, status| match existing_entry {
            Some(entry) if summary_payload_satisfies(entry, force_regenerate) => {
                stages::InflightSubmissionResolution::Deduplicate
            }
            Some(entry)
                if status == TaskStatus::Queued
                    && force_regenerate
                    && matches!(
                        entry.payload,
                        QueuePayload::Summary {
                            force_regenerate: false
                        }
                    ) =>
            {
                stages::InflightSubmissionResolution::UpgradeQueuedEntry
            }
            Some(_) => stages::InflightSubmissionResolution::Conflict(format!(
                "summary already running with different force_regenerate semantics: {job_id}"
            )),
            None if status == TaskStatus::Running => {
                stages::InflightSubmissionResolution::Conflict(format!(
                    "summary task is marked running but queue entry is missing: {job_id}"
                ))
            }
            None => stages::InflightSubmissionResolution::Continue,
        },
        upgrade_queued_summary_request,
    )
    .map_err(AppError::internal)?
    {
        return Ok(submission);
    }
    if !force_regenerate
        && index_store
            .get_summary(job_id)
            .map_err(AppError::internal)?
            .is_some()
    {
        return Ok(StageJobSubmission {
            job,
            disposition: StageJobDisposition::Reused,
            planned_audio_files: Vec::new(),
            queue: None,
        });
    }

    if !summary_prerequisites_ready(&index_store, job_id, &job_dir).map_err(AppError::internal)? {
        return Err(AppError::bad_request(format!(
            "stt must be completed before summary: {job_id}"
        )));
    }

    let queued_at = now_rfc3339().map_err(AppError::internal)?;
    let entry = queue::build_summary_entry(job_id, force_regenerate, queued_at.clone());
    let (job, ticket) =
        stages::enqueue_existing_task(&index_store, job_id, TaskType::Summary, queued_at, entry)
            .map_err(AppError::internal)?;
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
    let summary_dir = AudioStore::new(repo_root)?
        .spool_root()
        .join("summary")
        .join(job_id);
    let result = (|| -> Result<SummaryExecutionResult, String> {
        let transcript_files = index_store.list_transcripts(job_id)?;
        if transcript_files.is_empty() {
            return Err(format!("no transcripts found for summary: {job_id}"));
        }

        fs::create_dir_all(&summary_dir).map_err(|error| {
            format!(
                "failed to create summary directory {}: {error}",
                summary_dir.display()
            )
        })?;
        let summary_file = artifacts::summary_output_path(&summary_dir, &job.source_file_name)?;
        if !force_regenerate && index_store.get_summary(job_id)?.is_some() {
            return Ok(SummaryExecutionResult::ReusedExisting);
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
        let summary_text = fs::read_to_string(&summary_file).map_err(|error| {
            format!(
                "failed to read generated summary {}: {error}",
                summary_file.display()
            )
        })?;
        let one_line_summary = generate_one_line_summary(
            &toolchain,
            &summary_dir,
            &job.source_file_name,
            &summary_text,
        )?;
        let auto_keywords = generate_summary_keywords(
            &toolchain,
            &summary_dir,
            &job.source_file_name,
            &summary_text,
        )?;
        Ok(SummaryExecutionResult::Generated {
            summary: SummaryRecord {
                job_id: job_id.to_string(),
                file_name: artifacts::summary_file_name().to_string(),
                text: summary_text,
                one_line_summary: Some(one_line_summary),
            },
            auto_keywords,
        })
    })();

    match result {
        Ok(execution) => {
            let finished_at = now_rfc3339()?;
            let completed = match execution {
                SummaryExecutionResult::ReusedExisting => {
                    stages::finalize_task_success(repo_root, job, TaskType::Summary, finished_at)?
                }
                SummaryExecutionResult::Generated {
                    summary,
                    auto_keywords,
                } => index_store.commit_summary_success(
                    job_id,
                    finished_at,
                    &summary,
                    &auto_keywords,
                )?,
            };
            let _ = stages::submit_followup_task(
                repo_root,
                &completed.job_id,
                TaskType::Embedding,
                || submit_summary_embedding_job(repo_root, &completed.job_id),
            );
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
        return Err("no completed jobs with transcription data found".to_string());
    }

    let selected = select_summary_candidate(&candidates, reader, writer)?;
    let submission = submit_summary_job(repo_root, &selected.job_id, false)
        .map_err(|error| error.to_string())?;
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

    let summary_dir = AudioStore::new(repo_root)?
        .spool_root()
        .join("summary")
        .join(&selected.job_id);
    let summary_file = artifacts::summary_output_path(&summary_dir, &selected.source_file_name)?;
    fs::create_dir_all(&summary_dir).map_err(|error| {
        format!(
            "failed to create summary spool directory {}: {error}",
            summary_dir.display()
        )
    })?;
    if let Some(summary) = IndexStore::new(repo_root).get_summary(&selected.job_id)? {
        fs::write(&summary_file, summary.text).map_err(|error| {
            format!(
                "failed to materialize summary file {}: {error}",
                summary_file.display()
            )
        })?;
    }
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
        let job_dir = index_store.job_dir(&job.job_id);
        if !job_dir.is_dir() {
            continue;
        }

        let transcript_files = index_store
            .list_transcripts(&job.job_id)?
            .into_iter()
            .map(|record| record.file_name)
            .collect::<Vec<_>>();
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

fn build_summary_prompt(transcript_files: &[TranscriptRecord]) -> Result<String, String> {
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
        prompt.push_str("\n\n[");
        prompt.push_str(&transcript_file.file_name);
        prompt.push_str("]\n");
        prompt.push_str(transcript_file.text.trim());
    }

    Ok(prompt)
}

fn build_one_line_summary_prompt(summary_text: &str) -> String {
    format!(
        "당신은 녹음 파일의 제목 아래에 붙일 한국어 한줄 별칭(alias)을 만드는 도우미다.\n\
- 아래 요약을 읽고 핵심 맥락만 담은 한줄 별칭을 만든다.\n\
- 출력은 한국어 평문 한 줄만 작성한다.\n\
- 마크다운 헤더, bullet, 번호, 따옴표, 파일명, job id는 쓰지 않는다.\n\
- 길이는 가능하면 30자 안팎으로 유지한다.\n\
- 확인할 수 없는 내용은 추측하지 않는다.\n\
\n[summary]\n{summary_text}\n"
    )
}

fn build_summary_keywords_prompt(summary_text: &str) -> String {
    format!(
        "당신은 한국어 녹음 요약에서 STT 힌트용 핵심 키워드만 추려내는 도우미다.\n\
- 아래 요약에서 고유명사, 제품명, 서비스명, 조직명, 전문 용어처럼 STT 품질에 도움이 될 표현만 고른다.\n\
- 출력은 한국어 평문으로만 작성하고 한 줄당 키워드 1개만 쓴다.\n\
- 최대 {MAX_SUMMARY_KEYWORDS}개까지만 출력한다.\n\
- 번호, bullet, 설명 문장, 따옴표, 헤더, `키워드:` 같은 접두어는 쓰지 않는다.\n\
- 일반 명사나 불필요하게 짧은 표현은 제외한다.\n\
- 확인할 수 없는 내용은 추측하지 않는다.\n\
\n[summary]\n{summary_text}\n"
    )
}

fn generate_one_line_summary(
    toolchain: &LlamaToolchain,
    summary_dir: &Path,
    source_file_name: &str,
    summary_text: &str,
) -> Result<String, String> {
    let prompt_file = artifacts::one_line_summary_prompt_file_path(summary_dir, source_file_name)?;
    let output_file = artifacts::one_line_summary_output_path(summary_dir, source_file_name)?;
    let raw = run_summary_followup_prompt(
        toolchain,
        &prompt_file,
        &output_file,
        &build_one_line_summary_prompt(summary_text),
        "one-line summary",
    )?;
    let one_line_summary = normalize_one_line_summary(&raw);
    if one_line_summary.is_empty() {
        return Err("generated one-line summary is empty".to_string());
    }
    Ok(one_line_summary)
}

fn generate_summary_keywords(
    toolchain: &LlamaToolchain,
    summary_dir: &Path,
    source_file_name: &str,
    summary_text: &str,
) -> Result<Vec<String>, String> {
    let prompt_file = artifacts::summary_keywords_prompt_file_path(summary_dir, source_file_name)?;
    let output_file = artifacts::summary_keywords_output_path(summary_dir, source_file_name)?;
    let raw = run_summary_followup_prompt(
        toolchain,
        &prompt_file,
        &output_file,
        &build_summary_keywords_prompt(summary_text),
        "summary keywords",
    )?;

    let mut parsed = normalize_keywords(
        &raw.lines()
            .filter_map(normalize_summary_keyword_line)
            .collect::<Vec<_>>(),
    );
    if parsed.len() > MAX_SUMMARY_KEYWORDS {
        parsed.truncate(MAX_SUMMARY_KEYWORDS);
    }
    if parsed.is_empty() {
        return Err("generated summary keywords are empty".to_string());
    }
    Ok(parsed)
}

fn run_summary_followup_prompt(
    toolchain: &LlamaToolchain,
    prompt_file: &Path,
    output_file: &Path,
    prompt: &str,
    label: &str,
) -> Result<String, String> {
    fs::write(prompt_file, prompt).map_err(|error| {
        format!(
            "failed to write {label} prompt {}: {error}",
            prompt_file.display()
        )
    })?;
    let generation_result = run_summary_generation(toolchain, prompt_file, output_file);
    let _ = fs::remove_file(prompt_file);
    if let Err(error) = generation_result {
        let _ = fs::remove_file(output_file);
        return Err(error);
    }
    let raw = match fs::read_to_string(output_file) {
        Ok(raw) => raw,
        Err(error) => {
            let _ = fs::remove_file(output_file);
            return Err(format!(
                "failed to read generated {label} {}: {error}",
                output_file.display()
            ));
        }
    };
    let _ = fs::remove_file(output_file);
    Ok(raw)
}

fn normalize_one_line_summary(text: &str) -> String {
    let collapsed = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let trimmed = collapsed.trim();
    let trimmed = trimmed
        .strip_prefix("## ")
        .or_else(|| trimmed.strip_prefix("# "))
        .unwrap_or(trimmed);
    let trimmed = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("> "))
        .unwrap_or(trimmed);
    trimmed.trim_matches('"').trim().to_string()
}

fn normalize_summary_keyword_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let trimmed = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("> "))
        .unwrap_or(trimmed);
    let trimmed = strip_ordered_list_prefix(trimmed);
    let trimmed = trimmed
        .strip_prefix("키워드:")
        .or_else(|| trimmed.strip_prefix("키워드 :"))
        .or_else(|| trimmed.strip_prefix("keywords:"))
        .or_else(|| trimmed.strip_prefix("keywords :"))
        .unwrap_or(trimmed)
        .trim();
    let trimmed = trimmed.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn strip_ordered_list_prefix(text: &str) -> &str {
    let digit_end = text
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_digit())
        .last()
        .map(|(index, ch)| index + ch.len_utf8())
        .unwrap_or(0);
    if digit_end == 0 || digit_end >= text.len() {
        return text;
    }

    let suffix = &text[digit_end..];
    if let Some(rest) = suffix.strip_prefix(". ") {
        return rest;
    }
    if let Some(rest) = suffix.strip_prefix(") ") {
        return rest;
    }
    text
}

fn summary_payload_satisfies(entry: &crate::index::QueueEntry, requested_force: bool) -> bool {
    match &entry.payload {
        QueuePayload::Summary { force_regenerate } => *force_regenerate || !requested_force,
        _ => false,
    }
}

fn upgrade_queued_summary_request(
    index_store: &IndexStore,
    job_id: &str,
    _task_type: TaskType,
) -> Result<Option<(JobRecord, crate::app::QueueTicket)>, String> {
    index_store.with_index_mut(|index| {
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
        let ticket = queue::find_ticket(index, job_id, TaskType::Summary)
            .ok_or_else(|| format!("summary queue ticket missing after upgrade: {job_id}"))?;
        Ok(Some((job, ticket)))
    })
}

fn summary_prerequisites_ready(
    index_store: &IndexStore,
    job_id: &str,
    job_dir: &Path,
) -> Result<bool, String> {
    let transcript_files = index_store.list_transcripts(job_id)?;
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
            let file_name = artifacts::transcript_file_name(audio_file)?;
            let transcript_id = artifacts::transcript_id_from_file_name(&file_name)?;
            Ok(all_present
                && index_store
                    .find_transcript(job_id, &transcript_id)?
                    .is_some())
        })
}
