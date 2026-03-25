use crate::ffmpeg::{
    ConversionOutputs, SplitMonoOutput, Toolchain as FfmpegToolchain, probe_audio_input,
    run_conversion,
};
use crate::index::{IndexStore, JobOutputs, JobProbe, JobRecord, JobSplitOutput};
use crate::llama::{Toolchain as LlamaToolchain, run_summary_generation};
use crate::whisper::{Toolchain as WhisperToolchain, run_transcription};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use uuid::Uuid;

const RUN_ID_FORMAT: &[time::format_description::FormatItem<'static>] =
    format_description!("[year][month][day]T[hour][minute][second]");

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliCommand {
    Ffmpeg { input: Option<PathBuf> },
    Stt,
    Summary,
    PrepareLlamaModel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SttCandidate {
    job_id: String,
    source_file_name: String,
    job_dir: PathBuf,
    audio_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SummaryCandidate {
    job_id: String,
    source_file_name: String,
    job_dir: PathBuf,
    transcript_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSummary {
    pub job_id: String,
    pub job_dir: PathBuf,
    pub outputs: ConversionOutputs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SttTranscriptOutput {
    pub source_path: PathBuf,
    pub text_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SttRunSummary {
    pub job_id: String,
    pub job_dir: PathBuf,
    pub stt_dir: PathBuf,
    pub transcripts: Vec<SttTranscriptOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryRunSummary {
    pub job_id: String,
    pub job_dir: PathBuf,
    pub summary_dir: PathBuf,
    pub summary_file: PathBuf,
}

pub fn main_cli() -> Result<(), String> {
    let repo_root = repo_root()?;
    load_repo_env(&repo_root)?;
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    match resolve_cli_command(&args, &mut reader, &mut writer)? {
        CliCommand::Ffmpeg { input } => {
            let input = match input {
                Some(input) => input,
                None => resolve_input_path(&[], &mut reader, &mut writer)?,
            };
            let summary = run_with_repo_root(&repo_root, &input)?;
            print_run_summary(&summary, &mut writer)?;
        }
        CliCommand::Stt => {
            let summary = run_stt_with_repo_root(&repo_root, &mut reader, &mut writer)?;
            print_stt_summary(&summary, &mut writer)?;
        }
        CliCommand::Summary => {
            let summary = run_summary_with_repo_root(&repo_root, &mut reader, &mut writer)?;
            print_summary_run_summary(&summary, &mut writer)?;
        }
        CliCommand::PrepareLlamaModel => {
            prepare_llama_model_with_repo_root(&repo_root)?;
        }
    }

    Ok(())
}

fn resolve_cli_command(
    args: &[OsString],
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
) -> Result<CliCommand, String> {
    match args {
        [] => prompt_for_mode(reader, writer),
        [mode] if mode == OsStr::new("ffmpeg") => Ok(CliCommand::Ffmpeg { input: None }),
        [mode, input] if mode == OsStr::new("ffmpeg") => Ok(CliCommand::Ffmpeg {
            input: Some(PathBuf::from(input)),
        }),
        [mode] if mode == OsStr::new("stt") => Ok(CliCommand::Stt),
        [mode, ..] if mode == OsStr::new("stt") => {
            Err("stt mode does not accept additional arguments".to_string())
        }
        [mode] if mode == OsStr::new("summary") => Ok(CliCommand::Summary),
        [mode, ..] if mode == OsStr::new("summary") => {
            Err("summary mode does not accept additional arguments".to_string())
        }
        [mode] if mode == OsStr::new("prepare-llama-model") => Ok(CliCommand::PrepareLlamaModel),
        [mode, ..] if mode == OsStr::new("prepare-llama-model") => {
            Err("prepare-llama-model mode does not accept additional arguments".to_string())
        }
        [path] => Ok(CliCommand::Ffmpeg {
            input: Some(PathBuf::from(path)),
        }),
        _ => Err(
            "usage: recordroute_rust [ffmpeg <input>|stt|summary|prepare-llama-model|<input>]"
                .to_string(),
        ),
    }
}

fn prompt_for_mode(reader: &mut dyn BufRead, writer: &mut dyn Write) -> Result<CliCommand, String> {
    loop {
        writeln!(writer, "Select mode:").map_err(|error| error.to_string())?;
        writeln!(writer, "1. ffmpeg 작업").map_err(|error| error.to_string())?;
        writeln!(writer, "2. stt 작업").map_err(|error| error.to_string())?;
        writeln!(writer, "3. summary 작업").map_err(|error| error.to_string())?;
        write!(writer, "Enter number: ").map_err(|error| error.to_string())?;
        writer.flush().map_err(|error| error.to_string())?;

        let line = read_line(reader, "failed to read mode selection")?;
        match line.trim() {
            "1" => return Ok(CliCommand::Ffmpeg { input: None }),
            "2" => return Ok(CliCommand::Stt),
            "3" => return Ok(CliCommand::Summary),
            _ => {
                writeln!(writer, "Invalid selection. Enter 1, 2, or 3.")
                    .map_err(|error| error.to_string())?;
            }
        }
    }
}

pub fn resolve_input_path(
    args: &[OsString],
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
) -> Result<PathBuf, String> {
    match args {
        [] => {
            write!(writer, "Input audio path: ").map_err(|error| error.to_string())?;
            writer.flush().map_err(|error| error.to_string())?;

            let line = read_line(reader, "failed to read input path")?;
            parse_input_path_value(line.trim())
        }
        [path] => parse_input_path_os(path),
        _ => Err("expected exactly one input path".to_string()),
    }
}

pub fn run_with_repo_root(repo_root: &Path, input: &Path) -> Result<RunSummary, String> {
    let input_path = normalize_input_path(input)?;
    let index_store = IndexStore::new(repo_root);

    if let Some(job) = index_store.find_reusable_completed_job(&input_path)? {
        return run_summary_from_completed_job(job);
    }

    let toolchain = FfmpegToolchain::discover(repo_root)?;
    index_store.ensure_db_dir()?;

    let started_at = now_rfc3339()?;
    let job_id = build_run_id()?;
    let job_dir = index_store.job_dir(&job_id);
    fs::create_dir_all(&job_dir).map_err(|error| {
        format!(
            "failed to create job directory {}: {error}",
            job_dir.display()
        )
    })?;

    let mut job = JobRecord::new(
        job_id.clone(),
        started_at,
        input_path.clone(),
        job_dir.clone(),
    );
    index_store.insert_job(job.clone())?;

    let probe = match probe_audio_input(&toolchain, &input_path) {
        Ok(probe) => probe,
        Err(error) => {
            job.mark_failed(now_rfc3339()?, error.clone());
            index_store.update_job(&job_id, |_| job.clone())?;
            return Err(error);
        }
    };

    job.probe = JobProbe {
        channels: Some(probe.channels),
        channel_layout: probe.channel_layout.clone(),
    };

    let planned_outputs = ConversionOutputs::new(&job_dir, probe.channels);

    match run_conversion(&toolchain, &input_path, probe.channels, &planned_outputs) {
        Ok(()) => {
            job.mark_completed(
                now_rfc3339()?,
                JobOutputs {
                    merged_mono_wav: Some(path_to_string(&planned_outputs.merged_mono_wav)),
                    split_mono_wavs: planned_outputs
                        .split_mono_wavs
                        .iter()
                        .map(|output| JobSplitOutput {
                            channel_index: output.channel_index,
                            path: path_to_string(&output.path),
                        })
                        .collect(),
                },
            );
            index_store.update_job(&job_id, |_| job.clone())?;

            Ok(RunSummary {
                job_id,
                job_dir,
                outputs: planned_outputs,
            })
        }
        Err(error) => {
            planned_outputs.cleanup_partial_files();
            job.mark_failed(now_rfc3339()?, error.clone());
            index_store.update_job(&job_id, |_| job.clone())?;
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
    let toolchain = WhisperToolchain::discover(repo_root)?;
    toolchain.ensure_model()?;

    let stt_dir = selected.job_dir.join("stt");
    fs::create_dir_all(&stt_dir).map_err(|error| {
        format!(
            "failed to create stt directory {}: {error}",
            stt_dir.display()
        )
    })?;

    let mut transcripts = Vec::new();
    for audio_file in &selected.audio_files {
        let text_path = transcript_output_path(&stt_dir, audio_file)?;
        run_transcription(&toolchain, audio_file, &text_path)?;
        transcripts.push(SttTranscriptOutput {
            source_path: audio_file.clone(),
            text_path,
        });
    }

    Ok(SttRunSummary {
        job_id: selected.job_id,
        job_dir: selected.job_dir,
        stt_dir,
        transcripts,
    })
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
    let summary_dir = selected.job_dir.join("summary");
    let summary_file = summary_output_path(&summary_dir, &selected.source_file_name)?;
    if summary_file.is_file() {
        return Ok(SummaryRunSummary {
            job_id: selected.job_id,
            job_dir: selected.job_dir,
            summary_dir,
            summary_file,
        });
    }

    let toolchain = LlamaToolchain::discover(repo_root)?;
    toolchain.ensure_model()?;
    fs::create_dir_all(&summary_dir).map_err(|error| {
        format!(
            "failed to create summary directory {}: {error}",
            summary_dir.display()
        )
    })?;

    let prompt_file = summary_prompt_file_path(&summary_dir, &selected.source_file_name)?;
    let prompt = build_summary_prompt(&selected.transcript_files)?;
    fs::write(&prompt_file, prompt).map_err(|error| {
        format!(
            "failed to write summary prompt {}: {error}",
            prompt_file.display()
        )
    })?;

    let generation_result = run_summary_generation(&toolchain, &prompt_file, &summary_file);
    let _ = fs::remove_file(&prompt_file);
    generation_result?;

    Ok(SummaryRunSummary {
        job_id: selected.job_id,
        job_dir: selected.job_dir,
        summary_dir,
        summary_file,
    })
}

pub fn prepare_llama_model_with_repo_root(repo_root: &Path) -> Result<(), String> {
    let toolchain = LlamaToolchain::discover(repo_root)?;
    toolchain.ensure_model()
}

fn run_summary_from_completed_job(job: JobRecord) -> Result<RunSummary, String> {
    let merged_mono_wav = job.outputs.merged_mono_wav.clone().ok_or_else(|| {
        format!(
            "reusable completed job is missing merged output path: {}",
            job.job_id
        )
    })?;
    if job.outputs.split_mono_wavs.is_empty() {
        return Err(format!(
            "reusable completed job is missing split output paths: {}",
            job.job_id
        ));
    }

    Ok(RunSummary {
        job_id: job.job_id,
        job_dir: PathBuf::from(job.job_dir),
        outputs: ConversionOutputs {
            merged_mono_wav: PathBuf::from(merged_mono_wav),
            split_mono_wavs: job
                .outputs
                .split_mono_wavs
                .into_iter()
                .map(|output| SplitMonoOutput {
                    channel_index: output.channel_index,
                    path: PathBuf::from(output.path),
                })
                .collect(),
        },
    })
}

fn collect_stt_candidates(index_store: &IndexStore) -> Result<Vec<SttCandidate>, String> {
    let mut candidates = Vec::new();

    for job in index_store.list_jobs()? {
        let job_dir = PathBuf::from(&job.job_dir);
        if !job_dir.is_dir() {
            continue;
        }

        let audio_files = supported_audio_files(&job_dir)?;
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

fn collect_summary_candidates(index_store: &IndexStore) -> Result<Vec<SummaryCandidate>, String> {
    let mut candidates = Vec::new();

    for job in index_store.list_jobs()? {
        let job_dir = PathBuf::from(&job.job_dir);
        if !job_dir.is_dir() {
            continue;
        }

        let stt_dir = job_dir.join("stt");
        let transcript_files = transcript_text_files(&stt_dir)?;
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

fn supported_audio_files(job_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut audio_files = Vec::new();

    for entry in fs::read_dir(job_dir).map_err(|error| {
        format!(
            "failed to read job directory {}: {error}",
            job_dir.display()
        )
    })? {
        let entry = entry.map_err(|error| {
            format!(
                "failed to inspect job directory entry {}: {error}",
                job_dir.display()
            )
        })?;
        let path = entry.path();
        if !path.is_file() || !is_supported_audio_file(&path) {
            continue;
        }
        audio_files.push(path);
    }

    audio_files.sort();
    Ok(audio_files)
}

fn transcript_text_files(stt_dir: &Path) -> Result<Vec<PathBuf>, String> {
    if !stt_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut transcript_files = Vec::new();
    for entry in fs::read_dir(stt_dir).map_err(|error| {
        format!(
            "failed to read stt directory {}: {error}",
            stt_dir.display()
        )
    })? {
        let entry = entry.map_err(|error| {
            format!(
                "failed to inspect stt directory entry {}: {error}",
                stt_dir.display()
            )
        })?;
        let path = entry.path();
        if path.is_file() && is_transcript_text_file(&path) {
            transcript_files.push(path);
        }
    }

    transcript_files.sort();
    Ok(transcript_files)
}

fn is_supported_audio_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(extension)
            if extension.eq_ignore_ascii_case("wav")
                || extension.eq_ignore_ascii_case("mp3")
                || extension.eq_ignore_ascii_case("flac")
                || extension.eq_ignore_ascii_case("ogg")
    )
}

fn is_transcript_text_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(extension) if extension.eq_ignore_ascii_case("txt")
    )
}

fn transcript_output_path(stt_dir: &Path, audio_file: &Path) -> Result<PathBuf, String> {
    let stem = audio_file.file_stem().ok_or_else(|| {
        format!(
            "audio file does not have a valid file stem: {}",
            audio_file.display()
        )
    })?;
    Ok(stt_dir.join(stem).with_extension("txt"))
}

fn summary_output_path(summary_dir: &Path, source_file_name: &str) -> Result<PathBuf, String> {
    Ok(summary_dir
        .join(source_file_stem(source_file_name)?)
        .with_extension("md"))
}

fn summary_prompt_file_path(summary_dir: &Path, source_file_name: &str) -> Result<PathBuf, String> {
    let mut file_name = OsString::from(".");
    file_name.push(source_file_stem(source_file_name)?);
    file_name.push(".prompt.txt");
    Ok(summary_dir.join(file_name))
}

fn source_file_stem(source_file_name: &str) -> Result<OsString, String> {
    Path::new(source_file_name)
        .file_stem()
        .map(OsStr::to_os_string)
        .ok_or_else(|| format!("source file does not have a valid file stem: {source_file_name}"))
}

fn build_summary_prompt(transcript_files: &[PathBuf]) -> Result<String, String> {
    let mut prompt = String::from(
        "당신은 회의 녹취를 정리하는 한국어 회의록 작성 도우미다.\n\
다음 입력은 동일한 오디오를 채널별로 분리해 생성한 STT 결과이며, 파일마다 중복된 문장이 포함될 수 있다.\n\
아래 규칙을 지켜 하나의 일관된 회의록으로 요약하라.\n\
- 여러 파일에 겹치는 내용은 병합하고 중복 표현은 제거한다.\n\
- 확인할 수 없는 내용은 추측하거나 보강하지 않는다.\n\
- 출력은 반드시 한국어 평문으로만 작성한다.\n\
- 섹션 제목은 정확히 다음 4개를 사용한다: 개요, 핵심 논의, 결정/합의, 후속 조치.\n\
- 채널별 파일명을 나열하는 대신 실제 논의 내용을 중심으로 정리한다.\n",
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

fn print_run_summary(summary: &RunSummary, writer: &mut dyn Write) -> Result<(), String> {
    writeln!(writer, "job_id: {}", summary.job_id).map_err(|error| error.to_string())?;
    writeln!(writer, "job_dir: {}", summary.job_dir.display())
        .map_err(|error| error.to_string())?;
    writeln!(
        writer,
        "merged_mono_wav: {}",
        summary.outputs.merged_mono_wav.display()
    )
    .map_err(|error| error.to_string())?;
    for split in &summary.outputs.split_mono_wavs {
        writeln!(
            writer,
            "channel_{:02}: {}",
            split.channel_index,
            split.path.display()
        )
        .map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn print_stt_summary(summary: &SttRunSummary, writer: &mut dyn Write) -> Result<(), String> {
    writeln!(writer, "job_id: {}", summary.job_id).map_err(|error| error.to_string())?;
    writeln!(writer, "job_dir: {}", summary.job_dir.display())
        .map_err(|error| error.to_string())?;
    writeln!(writer, "stt_dir: {}", summary.stt_dir.display())
        .map_err(|error| error.to_string())?;
    for transcript in &summary.transcripts {
        writeln!(
            writer,
            "{} -> {}",
            transcript.source_path.display(),
            transcript.text_path.display()
        )
        .map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn print_summary_run_summary(
    summary: &SummaryRunSummary,
    writer: &mut dyn Write,
) -> Result<(), String> {
    writeln!(writer, "job_id: {}", summary.job_id).map_err(|error| error.to_string())?;
    writeln!(writer, "job_dir: {}", summary.job_dir.display())
        .map_err(|error| error.to_string())?;
    writeln!(writer, "summary_dir: {}", summary.summary_dir.display())
        .map_err(|error| error.to_string())?;
    writeln!(writer, "summary_file: {}", summary.summary_file.display())
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn read_line(reader: &mut dyn BufRead, error_context: &str) -> Result<String, String> {
    let mut line = String::new();
    let bytes_read = reader
        .read_line(&mut line)
        .map_err(|error| format!("{error_context}: {error}"))?;
    if bytes_read == 0 {
        return Err(format!("{error_context}: reached end of input"));
    }

    Ok(line)
}

fn parse_input_path_os(input: &OsStr) -> Result<PathBuf, String> {
    match input.to_str() {
        Some(value) => parse_input_path_value(value),
        None => Ok(PathBuf::from(input)),
    }
}

fn parse_input_path_value(input: &str) -> Result<PathBuf, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input path is required".to_string());
    }

    Ok(PathBuf::from(strip_wrapping_quotes(trimmed)))
}

fn strip_wrapping_quotes(input: &str) -> &str {
    if input.len() < 2 {
        return input;
    }

    let bytes = input.as_bytes();
    let first = bytes[0];
    let last = bytes[input.len() - 1];
    if (first == b'\'' && last == b'\'') || (first == b'"' && last == b'"') {
        &input[1..input.len() - 1]
    } else {
        input
    }
}

fn normalize_input_path(input: &Path) -> Result<PathBuf, String> {
    if !input.exists() {
        return Err(format!("input file not found: {}", input.display()));
    }

    if !input.is_file() {
        return Err(format!("input path is not a file: {}", input.display()));
    }

    fs::canonicalize(input)
        .map_err(|error| format!("failed to resolve input path {}: {error}", input.display()))
}

fn repo_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to resolve repository root".to_string())
}

fn load_repo_env(repo_root: &Path) -> Result<(), String> {
    let env_path = repo_root.join(".env");
    if !env_path.is_file() {
        return Ok(());
    }

    dotenvy::from_path(&env_path)
        .map_err(|error| format!("failed to load {}: {error}", env_path.display()))?;
    Ok(())
}

fn build_run_id() -> Result<String, String> {
    let timestamp = OffsetDateTime::now_utc()
        .format(RUN_ID_FORMAT)
        .map_err(|error| format!("failed to format timestamp: {error}"))?;
    Ok(format!("{timestamp}_{}", Uuid::now_v7()))
}

fn now_rfc3339() -> Result<String, String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| format!("failed to format timestamp: {error}"))
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffmpeg::Toolchain as FfmpegToolchain;
    use crate::index::IndexStore;
    use crate::test_support::env_lock;
    use std::fs::{self, File};
    use std::io::Cursor;

    #[test]
    fn resolves_single_path_argument() {
        let args = vec![OsString::from("/tmp/input.mp3")];
        let mut reader = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();

        let path = resolve_input_path(&args, &mut reader, &mut output).expect("path should parse");

        assert_eq!(path, PathBuf::from("/tmp/input.mp3"));
        assert!(output.is_empty());
    }

    #[test]
    fn prompts_for_missing_path() {
        let args = Vec::<OsString>::new();
        let mut reader = Cursor::new(b"/tmp/from-prompt.wav\n".to_vec());
        let mut output = Vec::new();

        let path =
            resolve_input_path(&args, &mut reader, &mut output).expect("prompt should parse");

        assert_eq!(path, PathBuf::from("/tmp/from-prompt.wav"));
        assert_eq!(
            String::from_utf8(output).expect("utf8"),
            "Input audio path: "
        );
    }

    #[test]
    fn strips_wrapping_quotes_from_prompt_input() {
        let args = Vec::<OsString>::new();
        let mut reader = Cursor::new(b"'/tmp/from-prompt.wav'\n".to_vec());
        let mut output = Vec::new();

        let path =
            resolve_input_path(&args, &mut reader, &mut output).expect("prompt should parse");

        assert_eq!(path, PathBuf::from("/tmp/from-prompt.wav"));
    }

    #[test]
    fn rejects_multiple_paths() {
        let args = vec![OsString::from("one"), OsString::from("two")];
        let mut reader = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();

        let error = resolve_input_path(&args, &mut reader, &mut output).expect_err("should fail");

        assert_eq!(error, "expected exactly one input path");
    }

    #[test]
    fn strips_wrapping_quotes_from_single_path_argument() {
        let args = vec![OsString::from("\"/tmp/input.mp3\"")];
        let mut reader = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();

        let path = resolve_input_path(&args, &mut reader, &mut output).expect("path should parse");

        assert_eq!(path, PathBuf::from("/tmp/input.mp3"));
        assert!(output.is_empty());
    }

    #[test]
    fn resolves_legacy_input_as_ffmpeg_command() {
        let args = vec![OsString::from("/tmp/input.wav")];
        let mut reader = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();

        let command = resolve_cli_command(&args, &mut reader, &mut output).expect("command");

        assert_eq!(
            command,
            CliCommand::Ffmpeg {
                input: Some(PathBuf::from("/tmp/input.wav"))
            }
        );
    }

    #[test]
    fn prompts_for_mode_when_no_args() {
        let mut reader = Cursor::new(b"2\n".to_vec());
        let mut output = Vec::new();

        let command = resolve_cli_command(&[], &mut reader, &mut output).expect("command");

        assert_eq!(command, CliCommand::Stt);
        assert!(
            String::from_utf8(output)
                .expect("utf8")
                .contains("Select mode:")
        );
    }

    #[test]
    fn retries_invalid_mode_selection() {
        let mut reader = Cursor::new(b"9\n1\n".to_vec());
        let mut output = Vec::new();

        let command = resolve_cli_command(&[], &mut reader, &mut output).expect("command");

        assert_eq!(command, CliCommand::Ffmpeg { input: None });
        assert!(
            String::from_utf8(output)
                .expect("utf8")
                .contains("Invalid selection. Enter 1, 2, or 3.")
        );
    }

    #[test]
    fn stt_command_rejects_extra_arguments() {
        let args = vec![OsString::from("stt"), OsString::from("extra")];
        let mut reader = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();

        let error = resolve_cli_command(&args, &mut reader, &mut output).expect_err("should fail");

        assert_eq!(error, "stt mode does not accept additional arguments");
    }

    #[test]
    fn resolves_summary_command() {
        let args = vec![OsString::from("summary")];
        let mut reader = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();

        let command = resolve_cli_command(&args, &mut reader, &mut output).expect("command");

        assert_eq!(command, CliCommand::Summary);
    }

    #[test]
    fn resolves_prepare_llama_model_command() {
        let args = vec![OsString::from("prepare-llama-model")];
        let mut reader = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();

        let command = resolve_cli_command(&args, &mut reader, &mut output).expect("command");

        assert_eq!(command, CliCommand::PrepareLlamaModel);
    }

    #[test]
    fn summary_command_rejects_extra_arguments() {
        let args = vec![OsString::from("summary"), OsString::from("extra")];
        let mut reader = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();

        let error = resolve_cli_command(&args, &mut reader, &mut output).expect_err("should fail");

        assert_eq!(error, "summary mode does not accept additional arguments");
    }

    #[test]
    fn prepare_llama_model_command_rejects_extra_arguments() {
        let args = vec![
            OsString::from("prepare-llama-model"),
            OsString::from("extra"),
        ];
        let mut reader = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();

        let error = resolve_cli_command(&args, &mut reader, &mut output).expect_err("should fail");

        assert_eq!(
            error,
            "prepare-llama-model mode does not accept additional arguments"
        );
    }

    #[test]
    fn prompts_for_summary_mode_when_selected() {
        let mut reader = Cursor::new(b"3\n".to_vec());
        let mut output = Vec::new();

        let command = resolve_cli_command(&[], &mut reader, &mut output).expect("command");

        assert_eq!(command, CliCommand::Summary);
        assert!(
            String::from_utf8(output)
                .expect("utf8")
                .contains("3. summary 작업")
        );
    }

    #[test]
    fn run_id_contains_timestamp_and_uuid() {
        let run_id = build_run_id().expect("run id");
        let (timestamp, uuid) = run_id.split_once('_').expect("split run id");

        assert_eq!(timestamp.len(), 15);
        assert_eq!(&timestamp[8..9], "T");
        assert!(Uuid::parse_str(uuid).is_ok());
    }

    #[test]
    fn end_to_end_flow_uses_fake_toolchain() {
        let repo_root = temp_workspace();
        let scripts_dir = repo_root.join("scripts");
        let build_bin = repo_root
            .join(".build/ffmpeg")
            .join(crate::ffmpeg::target_dir_name())
            .join("install/bin");
        let input = repo_root.join("fixture.wav");
        let ffmpeg_log = repo_root.join("ffmpeg-args.log");
        fs::create_dir_all(&scripts_dir).expect("scripts dir");
        fs::create_dir_all(&build_bin).expect("toolchain dir");
        write_build_script(&build_script_path(&repo_root, "ffmpeg"));
        write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
        write_fake_ffmpeg(&fake_command_path(&build_bin, "ffmpeg"), &ffmpeg_log);
        write_test_wav(&input, 2);

        let summary = run_with_repo_root(&repo_root, &input).expect("run should succeed");

        assert!(summary.outputs.merged_mono_wav.exists());
        assert_eq!(summary.outputs.split_mono_wavs.len(), 2);
        assert!(summary.outputs.split_mono_wavs[0].path.exists());
        assert!(summary.outputs.split_mono_wavs[1].path.exists());

        let log = fs::read_to_string(ffmpeg_log).expect("ffmpeg log");
        assert!(log.contains("-filter_complex"));
        assert!(log.contains(&crate::ffmpeg::build_filter_complex(2)));

        let index = fs::read_to_string(repo_root.join("db/index.json")).expect("index");
        let parsed: serde_json::Value = serde_json::from_str(&index).expect("valid json");
        assert_eq!(parsed["version"], 1);
        let jobs = parsed["jobs"].as_array().expect("jobs array");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0]["status"], "completed");
        assert_eq!(jobs[0]["probe"]["channels"], 2);
        assert_eq!(
            jobs[0]["outputs"]["split_mono_wavs"]
                .as_array()
                .expect("array")
                .len(),
            2
        );
    }

    #[test]
    fn failure_marks_job_failed_and_cleans_partial_outputs() {
        let repo_root = temp_workspace();
        let scripts_dir = repo_root.join("scripts");
        let build_bin = repo_root
            .join(".build/ffmpeg")
            .join(crate::ffmpeg::target_dir_name())
            .join("install/bin");
        let input = repo_root.join("fixture.wav");
        fs::create_dir_all(&scripts_dir).expect("scripts dir");
        fs::create_dir_all(&build_bin).expect("toolchain dir");
        write_build_script(&build_script_path(&repo_root, "ffmpeg"));
        write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
        write_failing_ffmpeg(&fake_command_path(&build_bin, "ffmpeg"));
        write_test_wav(&input, 2);

        let error = run_with_repo_root(&repo_root, &input).expect_err("run should fail");

        assert!(error.contains("ffmpeg conversion failed"));

        let index = fs::read_to_string(repo_root.join("db/index.json")).expect("index");
        let parsed: serde_json::Value = serde_json::from_str(&index).expect("valid json");
        let jobs = parsed["jobs"].as_array().expect("jobs array");
        assert_eq!(jobs[0]["status"], "failed");
        assert!(
            jobs[0]["error_message"]
                .as_str()
                .expect("error")
                .contains("ffmpeg conversion failed")
        );

        let job_dir = PathBuf::from(jobs[0]["job_dir"].as_str().expect("job dir"));
        assert!(job_dir.exists());
        assert!(!job_dir.join("channel_01.wav").exists());
        assert!(!job_dir.join("channel_02.wav").exists());
        assert!(!job_dir.join("mono_mix.wav").exists());
    }

    #[test]
    fn reuses_completed_outputs_for_same_input() {
        let repo_root = temp_workspace();
        let scripts_dir = repo_root.join("scripts");
        let build_bin = repo_root
            .join(".build/ffmpeg")
            .join(crate::ffmpeg::target_dir_name())
            .join("install/bin");
        let input = repo_root.join("fixture.wav");
        let ffmpeg_log = repo_root.join("ffmpeg-args.log");
        let ffmpeg_count = repo_root.join("ffmpeg-count.txt");
        fs::create_dir_all(&scripts_dir).expect("scripts dir");
        fs::create_dir_all(&build_bin).expect("toolchain dir");
        write_build_script(&build_script_path(&repo_root, "ffmpeg"));
        write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
        write_counting_ffmpeg(
            &fake_command_path(&build_bin, "ffmpeg"),
            &ffmpeg_log,
            &ffmpeg_count,
        );
        write_test_wav(&input, 2);

        let first = run_with_repo_root(&repo_root, &input).expect("first run should succeed");

        fs::remove_file(fake_command_path(&build_bin, "ffmpeg")).expect("remove ffmpeg");
        fs::remove_file(fake_command_path(&build_bin, "ffprobe")).expect("remove ffprobe");

        let second = run_with_repo_root(&repo_root, &input).expect("second run should reuse");

        assert_eq!(first.job_id, second.job_id);
        assert_eq!(first.job_dir, second.job_dir);
        assert_eq!(
            first.outputs.merged_mono_wav,
            second.outputs.merged_mono_wav
        );
        assert_eq!(
            first.outputs.split_mono_wavs,
            second.outputs.split_mono_wavs
        );
        assert_eq!(read_run_count(&ffmpeg_count), 1);

        let index = fs::read_to_string(repo_root.join("db/index.json")).expect("index");
        let parsed: serde_json::Value = serde_json::from_str(&index).expect("valid json");
        assert_eq!(parsed["jobs"].as_array().expect("jobs array").len(), 1);
    }

    #[test]
    fn missing_reusable_output_triggers_new_conversion() {
        let repo_root = temp_workspace();
        let scripts_dir = repo_root.join("scripts");
        let build_bin = repo_root
            .join(".build/ffmpeg")
            .join(crate::ffmpeg::target_dir_name())
            .join("install/bin");
        let input = repo_root.join("fixture.wav");
        let ffmpeg_log = repo_root.join("ffmpeg-args.log");
        let ffmpeg_count = repo_root.join("ffmpeg-count.txt");
        fs::create_dir_all(&scripts_dir).expect("scripts dir");
        fs::create_dir_all(&build_bin).expect("toolchain dir");
        write_build_script(&build_script_path(&repo_root, "ffmpeg"));
        write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
        write_counting_ffmpeg(
            &fake_command_path(&build_bin, "ffmpeg"),
            &ffmpeg_log,
            &ffmpeg_count,
        );
        write_test_wav(&input, 2);

        let first = run_with_repo_root(&repo_root, &input).expect("first run should succeed");
        fs::remove_file(&first.outputs.split_mono_wavs[0].path).expect("remove split output");

        let second =
            run_with_repo_root(&repo_root, &input).expect("second run should create new job");

        assert_ne!(first.job_id, second.job_id);
        assert_eq!(read_run_count(&ffmpeg_count), 2);
        assert!(second.outputs.merged_mono_wav.exists());
        assert!(second.outputs.split_mono_wavs[0].path.exists());
        assert!(second.outputs.split_mono_wavs[1].path.exists());

        let index = fs::read_to_string(repo_root.join("db/index.json")).expect("index");
        let parsed: serde_json::Value = serde_json::from_str(&index).expect("valid json");
        assert_eq!(parsed["jobs"].as_array().expect("jobs array").len(), 2);
    }

    #[test]
    fn reuses_previous_completed_job_when_latest_job_failed() {
        let repo_root = temp_workspace();
        let scripts_dir = repo_root.join("scripts");
        let build_bin = repo_root
            .join(".build/ffmpeg")
            .join(crate::ffmpeg::target_dir_name())
            .join("install/bin");
        let input = repo_root.join("fixture.wav");
        let ffmpeg_log = repo_root.join("ffmpeg-args.log");
        let ffmpeg_count = repo_root.join("ffmpeg-count.txt");
        fs::create_dir_all(&scripts_dir).expect("scripts dir");
        fs::create_dir_all(&build_bin).expect("toolchain dir");
        write_build_script(&build_script_path(&repo_root, "ffmpeg"));
        write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
        write_counting_ffmpeg(
            &fake_command_path(&build_bin, "ffmpeg"),
            &ffmpeg_log,
            &ffmpeg_count,
        );
        write_test_wav(&input, 2);

        let first = run_with_repo_root(&repo_root, &input).expect("first run should succeed");

        let store = IndexStore::new(&repo_root);
        let mut failed_job = JobRecord::new(
            "job-failed".to_string(),
            "2026-01-01T00:00:02Z".to_string(),
            fs::canonicalize(&input).expect("canonical input"),
            store.job_dir("job-failed"),
        );
        failed_job.mark_failed(
            "2026-01-01T00:00:03Z".to_string(),
            "synthetic failure".to_string(),
        );
        store.insert_job(failed_job).expect("insert failed job");

        fs::remove_file(fake_command_path(&build_bin, "ffmpeg")).expect("remove ffmpeg");
        fs::remove_file(fake_command_path(&build_bin, "ffprobe")).expect("remove ffprobe");

        let second = run_with_repo_root(&repo_root, &input).expect("run should reuse old job");

        assert_eq!(second.job_id, first.job_id);
        assert_eq!(second.job_dir, first.job_dir);
        assert_eq!(read_run_count(&ffmpeg_count), 1);

        let index = fs::read_to_string(repo_root.join("db/index.json")).expect("index");
        let parsed: serde_json::Value = serde_json::from_str(&index).expect("valid json");
        assert_eq!(parsed["jobs"].as_array().expect("jobs array").len(), 2);
        assert_eq!(parsed["jobs"][1]["status"], "failed");
    }

    #[test]
    fn missing_toolchain_reports_bootstrap_path() {
        let repo_root = temp_workspace();
        let input = repo_root.join("fixture.wav");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        write_build_script(&build_script_path(&repo_root, "ffmpeg"));
        write_test_wav(&input, 1);

        let error =
            run_with_repo_root(&repo_root, &input).expect_err("toolchain should be required");

        assert!(
            error.contains(
                build_script_path(&repo_root, "ffmpeg")
                    .to_string_lossy()
                    .as_ref()
            )
        );
        assert!(!repo_root.join("db/index.json").exists());
    }

    #[test]
    fn run_stt_processes_audio_files_in_selected_job_dir() {
        let repo_root = temp_workspace();
        let selected_job_dir = repo_root.join("db/job-1");
        let ignored_job_dir = repo_root.join("db/job-2");
        let whisper_bin = repo_root
            .join(".build/whisper")
            .join(crate::ffmpeg::target_dir_name())
            .join("bin");
        let whisper_log = repo_root.join("whisper-args.log");
        let download_log = repo_root.join("download.log");
        fs::create_dir_all(&selected_job_dir).expect("selected job dir");
        fs::create_dir_all(&ignored_job_dir).expect("ignored job dir");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        fs::create_dir_all(repo_root.join("whisper.cpp/models")).expect("download dir");
        fs::create_dir_all(&whisper_bin).expect("whisper bin");

        write_test_wav(&selected_job_dir.join("channel_01.wav"), 1);
        write_test_wav(&selected_job_dir.join("mono_mix.wav"), 1);
        fs::write(selected_job_dir.join("notes.txt"), "ignore").expect("notes");
        fs::write(ignored_job_dir.join("not-audio.txt"), "ignore").expect("ignored");

        write_build_script(&build_script_path(&repo_root, "whisper"));
        write_fake_whisper_cli(
            &fake_command_path(&whisper_bin, "whisper-cli"),
            &whisper_log,
            false,
        );
        write_fake_download_script(&whisper_download_script_path(&repo_root), &download_log);

        let store = IndexStore::new(&repo_root);
        store
            .insert_job(JobRecord::new(
                "job-1".to_string(),
                "2026-01-01T00:00:00Z".to_string(),
                PathBuf::from("/tmp/input.wav"),
                selected_job_dir.clone(),
            ))
            .expect("insert selected job");
        store
            .insert_job(JobRecord::new(
                "job-2".to_string(),
                "2026-01-01T00:00:01Z".to_string(),
                PathBuf::from("/tmp/other.wav"),
                ignored_job_dir,
            ))
            .expect("insert ignored job");

        let mut reader = Cursor::new(b"1\n".to_vec());
        let mut output = Vec::new();

        let summary =
            run_stt_with_repo_root(&repo_root, &mut reader, &mut output).expect("stt run");

        assert_eq!(summary.job_id, "job-1");
        assert_eq!(summary.transcripts.len(), 2);
        assert!(summary.stt_dir.is_dir());
        assert!(summary.stt_dir.join("channel_01.txt").is_file());
        assert!(summary.stt_dir.join("mono_mix.txt").is_file());
        assert!(
            fs::read_to_string(download_log)
                .expect("download log")
                .contains("base")
        );

        let whisper_log = fs::read_to_string(whisper_log).expect("whisper log");
        assert!(whisper_log.contains("-l"));
        assert!(whisper_log.contains("auto"));
        assert!(whisper_log.contains("channel_01.wav"));
        assert!(whisper_log.contains("mono_mix.wav"));
    }

    #[test]
    fn run_stt_retries_invalid_folder_selection() {
        let repo_root = temp_workspace();
        let selected_job_dir = repo_root.join("db/job-1");
        let whisper_bin = repo_root
            .join(".build/whisper")
            .join(crate::ffmpeg::target_dir_name())
            .join("bin");
        fs::create_dir_all(&selected_job_dir).expect("selected job dir");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        fs::create_dir_all(repo_root.join("whisper.cpp/models")).expect("download dir");
        fs::create_dir_all(repo_root.join("models/whisper")).expect("models dir");
        fs::create_dir_all(&whisper_bin).expect("whisper bin");

        write_test_wav(&selected_job_dir.join("channel_01.wav"), 1);
        write_build_script(&build_script_path(&repo_root, "whisper"));
        write_fake_whisper_cli(
            &fake_command_path(&whisper_bin, "whisper-cli"),
            &repo_root.join("whisper.log"),
            false,
        );
        write_fake_download_script(
            &whisper_download_script_path(&repo_root),
            &repo_root.join("download.log"),
        );
        fs::write(repo_root.join("models/whisper/ggml-base.bin"), "model").expect("model");

        let store = IndexStore::new(&repo_root);
        store
            .insert_job(JobRecord::new(
                "job-1".to_string(),
                "2026-01-01T00:00:00Z".to_string(),
                PathBuf::from("/tmp/input.wav"),
                selected_job_dir,
            ))
            .expect("insert selected job");

        let mut reader = Cursor::new(b"9\n1\n".to_vec());
        let mut output = Vec::new();

        let summary =
            run_stt_with_repo_root(&repo_root, &mut reader, &mut output).expect("stt run");

        assert_eq!(summary.job_id, "job-1");
        assert!(
            String::from_utf8(output)
                .expect("utf8")
                .contains("Invalid selection. Enter a number between 1 and 1.")
        );
    }

    #[test]
    fn run_summary_processes_transcript_files_in_selected_job_dir() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        unsafe {
            std::env::remove_var("RECORDROUTE_LLAMA_MODEL");
            std::env::set_var("HF_TOKEN", "summary-token");
        }

        let repo_root = temp_workspace();
        let selected_job_dir = repo_root.join("db/job-1");
        let ignored_job_dir = repo_root.join("db/job-2");
        let llama_bin = repo_root
            .join(".build/llama")
            .join(crate::ffmpeg::target_dir_name())
            .join("bin");
        let llama_log = repo_root.join("llama-args.log");
        let prompt_capture = repo_root.join("llama-prompt.txt");
        fs::create_dir_all(selected_job_dir.join("stt")).expect("selected stt dir");
        fs::create_dir_all(ignored_job_dir.join("stt")).expect("ignored stt dir");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        fs::create_dir_all(&llama_bin).expect("llama bin");

        fs::write(
            selected_job_dir.join("stt/channel_01.txt"),
            "화자 A가 일정과 비용을 설명했다.",
        )
        .expect("channel 01");
        fs::write(
            selected_job_dir.join("stt/channel_02.txt"),
            "화자 B가 일정과 비용을 다시 확인했다.",
        )
        .expect("channel 02");
        fs::write(
            selected_job_dir.join("stt/mono_mix.txt"),
            "전체 대화에서 다음 주 방문과 견적 검토가 언급되었다.",
        )
        .expect("mono mix");
        fs::write(ignored_job_dir.join("stt/notes.md"), "ignore").expect("ignored");

        write_build_script(&build_script_path(&repo_root, "llama"));
        write_fake_llama_cli(
            &fake_command_path(&llama_bin, "llama-cli"),
            &llama_log,
            &prompt_capture,
            false,
        );

        let store = IndexStore::new(&repo_root);
        store
            .insert_job(JobRecord::new(
                "job-1".to_string(),
                "2026-01-01T00:00:00Z".to_string(),
                PathBuf::from("/tmp/input.wav"),
                selected_job_dir.clone(),
            ))
            .expect("insert selected job");
        store
            .insert_job(JobRecord::new(
                "job-2".to_string(),
                "2026-01-01T00:00:01Z".to_string(),
                PathBuf::from("/tmp/other.wav"),
                ignored_job_dir,
            ))
            .expect("insert ignored job");

        let mut reader = Cursor::new(b"1\n".to_vec());
        let mut output = Vec::new();

        let summary =
            run_summary_with_repo_root(&repo_root, &mut reader, &mut output).expect("summary run");

        unsafe { std::env::remove_var("HF_TOKEN") };

        assert_eq!(summary.job_id, "job-1");
        assert_eq!(
            summary.summary_file,
            selected_job_dir.join("summary/input.md")
        );
        assert!(summary.summary_dir.is_dir());
        assert_eq!(
            fs::read_to_string(&summary.summary_file).expect("summary file"),
            "synthetic summary"
        );
        assert!(!summary.summary_dir.join(".input.prompt.txt").exists());

        let prompt = fs::read_to_string(prompt_capture).expect("prompt capture");
        assert!(prompt.contains("[channel_01.txt]"));
        assert!(prompt.contains("[channel_02.txt]"));
        assert!(prompt.contains("[mono_mix.txt]"));
        assert!(prompt.contains("개요"));
        assert!(prompt.contains("후속 조치"));
        assert!(prompt.contains("화자 A가 일정과 비용을 설명했다."));

        let llama_log = fs::read_to_string(llama_log).expect("llama log");
        assert!(llama_log.contains("--single-turn"));
        assert!(llama_log.contains("-hf"));
        assert!(llama_log.contains("ggml-org/gemma-3-4b-it-GGUF"));
        assert!(llama_log.contains("HF_TOKEN=summary-token"));
    }

    #[test]
    fn run_summary_reuses_existing_summary_file_without_llama_toolchain() {
        let repo_root = temp_workspace();
        let selected_job_dir = repo_root.join("db/job-1");
        let existing_summary = selected_job_dir.join("summary/input.md");
        fs::create_dir_all(selected_job_dir.join("stt")).expect("selected stt dir");
        fs::create_dir_all(existing_summary.parent().expect("summary dir")).expect("summary dir");

        fs::write(
            selected_job_dir.join("stt/mono_mix.txt"),
            "현장 방문 일정을 논의했다.",
        )
        .expect("mono mix");
        fs::write(&existing_summary, "existing summary").expect("existing summary");

        let store = IndexStore::new(&repo_root);
        store
            .insert_job(JobRecord::new(
                "job-1".to_string(),
                "2026-01-01T00:00:00Z".to_string(),
                PathBuf::from("/tmp/input.wav"),
                selected_job_dir.clone(),
            ))
            .expect("insert selected job");

        let mut reader = Cursor::new(b"1\n".to_vec());
        let mut output = Vec::new();

        let summary =
            run_summary_with_repo_root(&repo_root, &mut reader, &mut output).expect("summary run");

        assert_eq!(summary.job_id, "job-1");
        assert_eq!(summary.summary_dir, selected_job_dir.join("summary"));
        assert_eq!(summary.summary_file, existing_summary);
        assert_eq!(
            fs::read_to_string(&summary.summary_file).expect("summary file"),
            "existing summary"
        );
        assert!(!summary.summary_dir.join(".input.prompt.txt").exists());
    }

    #[test]
    fn run_summary_uses_local_model_path_when_file_exists() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let repo_root = temp_workspace();
        let selected_job_dir = repo_root.join("db/job-1");
        let llama_bin = repo_root
            .join(".build/llama")
            .join(crate::ffmpeg::target_dir_name())
            .join("bin");
        let llama_log = repo_root.join("llama-local.log");
        let prompt_capture = repo_root.join("llama-local-prompt.txt");
        let model_path = repo_root.join("models/llama/custom.gguf");
        fs::create_dir_all(selected_job_dir.join("stt")).expect("selected stt dir");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        fs::create_dir_all(&llama_bin).expect("llama bin");
        fs::create_dir_all(model_path.parent().expect("parent")).expect("models dir");

        fs::write(
            selected_job_dir.join("stt/mono_mix.txt"),
            "현장 방문 일정을 논의했다.",
        )
        .expect("mono mix");
        fs::write(&model_path, "model").expect("model");

        write_build_script(&build_script_path(&repo_root, "llama"));
        write_fake_llama_cli(
            &fake_command_path(&llama_bin, "llama-cli"),
            &llama_log,
            &prompt_capture,
            false,
        );

        unsafe { std::env::set_var("RECORDROUTE_LLAMA_MODEL", "models/llama/custom.gguf") };

        let store = IndexStore::new(&repo_root);
        store
            .insert_job(JobRecord::new(
                "job-1".to_string(),
                "2026-01-01T00:00:00Z".to_string(),
                PathBuf::from("/tmp/input.wav"),
                selected_job_dir,
            ))
            .expect("insert selected job");

        let mut reader = Cursor::new(b"1\n".to_vec());
        let mut output = Vec::new();

        let summary =
            run_summary_with_repo_root(&repo_root, &mut reader, &mut output).expect("summary run");

        unsafe { std::env::remove_var("RECORDROUTE_LLAMA_MODEL") };

        assert!(summary.summary_file.is_file());
        let llama_log = fs::read_to_string(llama_log).expect("llama log");
        assert!(llama_log.contains("-m"));
        assert!(llama_log.contains(model_path.to_string_lossy().as_ref()));
        assert!(!llama_log.contains("-hf"));
    }

    #[test]
    fn prepare_llama_model_downloads_default_hugging_face_repo() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        unsafe {
            std::env::remove_var("RECORDROUTE_LLAMA_MODEL");
            std::env::set_var("HF_TOKEN", "prepare-token");
        }

        let repo_root = temp_workspace();
        let llama_bin = repo_root
            .join(".build/llama")
            .join(crate::ffmpeg::target_dir_name())
            .join("bin");
        let llama_log = repo_root.join("llama-prepare.log");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        fs::create_dir_all(&llama_bin).expect("llama bin");

        write_build_script(&build_script_path(&repo_root, "llama"));
        write_fake_llama_cli(
            &fake_command_path(&llama_bin, "llama-cli"),
            &llama_log,
            &repo_root.join("unused-prompt.txt"),
            false,
        );

        prepare_llama_model_with_repo_root(&repo_root).expect("prepare llama model");

        unsafe {
            std::env::remove_var("RECORDROUTE_LLAMA_MODEL");
            std::env::remove_var("HF_TOKEN");
        }

        let cached_model = repo_root
            .join("models/llama/hf")
            .join("ggml-org__gemma-3-4b-it-GGUF.gguf");
        assert!(cached_model.is_file());

        let llama_log = fs::read_to_string(llama_log).expect("llama log");
        assert!(llama_log.contains("-hf"));
        assert!(llama_log.contains("ggml-org/gemma-3-4b-it-GGUF"));
        assert!(llama_log.contains("HF_TOKEN=prepare-token"));
        assert!(llama_log.contains("LLAMA_CACHE="));
    }

    #[test]
    fn load_repo_env_reads_only_dot_env() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let repo_root = temp_workspace();

        unsafe {
            std::env::remove_var("RECORDROUTE_TEST_DOTENV_LOADED");
            std::env::remove_var("RECORDROUTE_TEST_DOTENV_IGNORED");
        }

        fs::write(
            repo_root.join(".env"),
            "RECORDROUTE_TEST_DOTENV_LOADED=from-dotenv\n",
        )
        .expect("env");
        fs::write(
            repo_root.join(".env.example"),
            "RECORDROUTE_TEST_DOTENV_IGNORED=from-example\n",
        )
        .expect("env example");

        load_repo_env(&repo_root).expect("dotenv load");

        assert_eq!(
            std::env::var("RECORDROUTE_TEST_DOTENV_LOADED").expect("loaded"),
            "from-dotenv"
        );
        assert!(std::env::var("RECORDROUTE_TEST_DOTENV_IGNORED").is_err());

        unsafe {
            std::env::remove_var("RECORDROUTE_TEST_DOTENV_LOADED");
            std::env::remove_var("RECORDROUTE_TEST_DOTENV_IGNORED");
        }
    }

    fn temp_workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!("recordroute-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).expect("temp workspace");
        path
    }

    fn build_script_path(repo_root: &Path, tool: &str) -> PathBuf {
        crate::ffmpeg::build_script_path(repo_root, tool)
    }

    fn fake_command_path(base_dir: &Path, name: &str) -> PathBuf {
        crate::ffmpeg::fake_command_path(base_dir, name)
    }

    fn whisper_download_script_path(repo_root: &Path) -> PathBuf {
        if cfg!(windows) {
            repo_root.join("whisper.cpp/models/download-ggml-model.cmd")
        } else {
            repo_root.join("whisper.cpp/models/download-ggml-model.sh")
        }
    }

    fn write_build_script(path: &Path) {
        write_platform_script(path, "#!/bin/sh\nexit 0\n", "@echo off\nexit /b 0\n");
    }

    fn write_platform_script(path: &Path, unix_content: &str, windows_content: &str) {
        let content = if cfg!(windows) {
            windows_content.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            unix_content.to_string()
        };
        fs::write(path, content).expect("script");
        make_executable(path);
    }

    fn write_fake_ffprobe(path: &Path, channels: u32, channel_layout: Option<&str>) {
        let json = match channel_layout {
            Some(layout) => format!(
                "{{\"streams\":[{{\"channels\":{channels},\"channel_layout\":\"{layout}\"}}]}}"
            ),
            None => format!("{{\"streams\":[{{\"channels\":{channels}}}]}}"),
        };
        let unix_script = format!(
            "#!/bin/sh\nprintf '%s' '{}'\n",
            json.replace('\'', "'\"'\"'")
        );
        let windows_script = format!("@echo off\necho {json}\n");
        write_platform_script(path, &unix_script, &windows_script);
    }

    fn write_fake_ffmpeg(path: &Path, log_path: &Path) {
        let log = log_path.display();
        let unix_script = format!(
            "#!/bin/sh\n: > '{log}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\n  case \"$arg\" in\n    *.wav)\n      mkdir -p \"$(dirname \"$arg\")\"\n      : > \"$arg\"\n      ;;\n  esac\ndone\n"
        );
        let windows_script = format!(
            "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\n> \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo(!arg!\nfor %%I in (\"!arg!\") do (\n  if /I \"%%~xI\"==\".wav\" (\n    if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n    > \"%%~fI\" type nul\n  )\n)\nshift\ngoto loop\n:done\nexit /b 0\n"
        );
        write_platform_script(path, &unix_script, &windows_script);
    }

    fn write_counting_ffmpeg(path: &Path, log_path: &Path, count_path: &Path) {
        let log = log_path.display();
        let count = count_path.display();
        let unix_script = format!(
            "#!/bin/sh\ncount=0\nif [ -f '{count}' ]; then\n  count=$(cat '{count}')\nfi\ncount=$((count + 1))\nprintf '%s' \"$count\" > '{count}'\n: > '{log}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\n  case \"$arg\" in\n    *.wav)\n      mkdir -p \"$(dirname \"$arg\")\"\n      : > \"$arg\"\n      ;;\n  esac\ndone\n"
        );
        let windows_script = format!(
            "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nset /a count=0\nif exist \"{count}\" set /p count=<\"{count}\"\nset /a count+=1\n> \"{count}\" <nul set /p =!count!\n> \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo(!arg!\nfor %%I in (\"!arg!\") do (\n  if /I \"%%~xI\"==\".wav\" (\n    if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n    > \"%%~fI\" type nul\n  )\n)\nshift\ngoto loop\n:done\nexit /b 0\n"
        );
        write_platform_script(path, &unix_script, &windows_script);
    }

    fn write_failing_ffmpeg(path: &Path) {
        let unix_script = "#!/bin/sh\nlast=''\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    *.wav)\n      last=\"$arg\"\n      ;;\n  esac\ndone\nif [ -n \"$last\" ]; then\n  mkdir -p \"$(dirname \"$last\")\"\n  : > \"$last\"\nfi\nprintf 'synthetic ffmpeg failure' >&2\nexit 1\n";
        let windows_script = "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nset \"last=\"\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\nfor %%I in (\"!arg!\") do if /I \"%%~xI\"==\".wav\" set \"last=%%~fI\"\nshift\ngoto loop\n:done\nif defined last (\n  for %%I in (\"!last!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n  > \"!last!\" type nul\n)\necho synthetic ffmpeg failure 1>&2\nexit /b 1\n";
        write_platform_script(path, unix_script, windows_script);
    }

    fn write_fake_whisper_cli(path: &Path, log_path: &Path, fail: bool) {
        let log = log_path.display();
        let unix_script = if fail {
            format!(
                "#!/bin/sh\ntouch '{log}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\ndone\nprintf 'synthetic whisper failure' >&2\nexit 1\n"
            )
        } else {
            format!(
                "#!/bin/sh\ntouch '{log}'\nout=''\nnext=''\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\n  if [ \"$next\" = 'of' ]; then\n    out=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -of)\n      next='of'\n      ;;\n  esac\ndone\nmkdir -p \"$(dirname \"$out\")\"\nprintf 'synthetic transcript' > \"$out.txt\"\n"
            )
        };
        let windows_script = if fail {
            format!(
                "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nif not exist \"{log}\" > \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo(!arg!\nshift\ngoto loop\n:done\necho synthetic whisper failure 1>&2\nexit /b 1\n"
            )
        } else {
            format!(
                "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nif not exist \"{log}\" > \"{log}\" type nul\nset \"out=\"\nset \"next=\"\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo(!arg!\nif /I \"!next!\"==\"of\" (\n  set \"out=!arg!\"\n  set \"next=\"\n) else if /I \"!arg!\"==\"-of\" (\n  set \"next=of\"\n)\nshift\ngoto loop\n:done\nif defined out (\n  for %%I in (\"!out!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n  > \"!out!.txt\" <nul set /p =synthetic transcript\n)\nexit /b 0\n"
            )
        };
        write_platform_script(path, &unix_script, &windows_script);
    }

    fn write_fake_llama_cli(path: &Path, log_path: &Path, prompt_capture_path: &Path, fail: bool) {
        let log = log_path.display();
        let prompt_capture = prompt_capture_path.display();
        let unix_script = if fail {
            format!(
                "#!/bin/sh\ntouch '{log}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\ndone\nprintf 'HF_TOKEN=%s\\n' \"${{HF_TOKEN:-}}\" >> '{log}'\nprintf 'LLAMA_CACHE=%s\\n' \"${{LLAMA_CACHE:-}}\" >> '{log}'\nprintf 'synthetic llama failure' >&2\nexit 1\n"
            )
        } else {
            format!(
                "#!/bin/sh\ntouch '{log}'\nprompt=''\nmodel=''\nhf_repo=''\nnext=''\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\n  if [ \"$next\" = 'f' ]; then\n    prompt=\"$arg\"\n    next=''\n    continue\n  fi\n  if [ \"$next\" = 'm' ]; then\n    model=\"$arg\"\n    next=''\n    continue\n  fi\n  if [ \"$next\" = 'hf' ]; then\n    hf_repo=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -f)\n      next='f'\n      ;;\n    -m)\n      next='m'\n      ;;\n    -hf)\n      next='hf'\n      ;;\n  esac\ndone\nprintf 'HF_TOKEN=%s\\n' \"${{HF_TOKEN:-}}\" >> '{log}'\nprintf 'LLAMA_CACHE=%s\\n' \"${{LLAMA_CACHE:-}}\" >> '{log}'\nif [ -n \"$prompt\" ]; then\n  cat \"$prompt\" > '{prompt_capture}'\nfi\nif [ -n \"$model\" ]; then\n  mkdir -p \"$(dirname \"$model\")\"\n  printf 'synthetic model' > \"$model\"\nelif [ -n \"$hf_repo\" ] && [ -n \"${{LLAMA_CACHE:-}}\" ]; then\n  mkdir -p \"$LLAMA_CACHE\"\n  printf 'synthetic downloaded model' > \"$LLAMA_CACHE/downloaded-model.gguf\"\nfi\nif [ -n \"$prompt\" ]; then\n  printf 'synthetic summary'\nfi\n"
            )
        };
        let windows_script = if fail {
            format!(
                "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nif not exist \"{log}\" > \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo(!arg!\nshift\ngoto loop\n:done\nset \"llama_cache=!LLAMA_CACHE!\"\nif \"!llama_cache:~0,4!\"==\"\\\\?\\\" set \"llama_cache=!llama_cache:~4!\"\n>> \"{log}\" echo HF_TOKEN=!HF_TOKEN!\n>> \"{log}\" echo LLAMA_CACHE=!llama_cache!\necho synthetic llama failure 1>&2\nexit /b 1\n"
            )
        } else {
            format!(
                "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nif not exist \"{log}\" > \"{log}\" type nul\nset \"prompt=\"\nset \"model=\"\nset \"hf_repo=\"\nset \"next=\"\n:loop\nif \"%~1\"==\"\" goto after\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo(!arg!\nif /I \"!next!\"==\"f\" (\n  set \"prompt=!arg!\"\n  set \"next=\"\n) else if /I \"!next!\"==\"m\" (\n  set \"model=!arg!\"\n  set \"next=\"\n) else if /I \"!next!\"==\"hf\" (\n  set \"hf_repo=!arg!\"\n  set \"next=\"\n) else if /I \"!arg!\"==\"-f\" (\n  set \"next=f\"\n) else if /I \"!arg!\"==\"-m\" (\n  set \"next=m\"\n) else if /I \"!arg!\"==\"-hf\" (\n  set \"next=hf\"\n)\nshift\ngoto loop\n:after\nset \"llama_cache=!LLAMA_CACHE!\"\nif \"!llama_cache:~0,4!\"==\"\\\\?\\\" set \"llama_cache=!llama_cache:~4!\"\n>> \"{log}\" echo HF_TOKEN=!HF_TOKEN!\n>> \"{log}\" echo LLAMA_CACHE=!llama_cache!\nif defined prompt copy /y \"!prompt!\" \"{prompt_capture}\" >nul\nif defined model (\n  for %%I in (\"!model!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n  > \"!model!\" <nul set /p =synthetic model\n) else if defined hf_repo if defined llama_cache (\n  if not exist \"!llama_cache!\" mkdir \"!llama_cache!\"\n  > \"!llama_cache!\\downloaded-model.gguf\" <nul set /p =synthetic downloaded model\n)\nif defined prompt <nul set /p =synthetic summary\nexit /b 0\n"
            )
        };
        write_platform_script(path, &unix_script, &windows_script);
    }
    fn write_fake_download_script(path: &Path, log_path: &Path) {
        let log = log_path.display();
        let unix_script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{log}'\nmkdir -p \"$2\"\n: > \"$2/ggml-$1.bin\"\n"
        );
        let windows_script = format!(
            "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nset \"model=%~1\"\nset \"out_dir=%~2\"\nif \"!out_dir:~0,4!\"==\"\\\\?\\\" set \"out_dir=!out_dir:~4!\"\n> \"{log}\" echo(!model! !out_dir!\nif not exist \"!out_dir!\" mkdir \"!out_dir!\"\n> \"!out_dir!\\ggml-!model!.bin\" type nul\nexit /b 0\n"
        );
        write_platform_script(path, &unix_script, &windows_script);
    }
    fn write_test_wav(path: &Path, channels: u16) {
        let mut file = File::create(path).expect("fixture wav");
        let sample_rate: u32 = 16_000;
        let bits_per_sample: u16 = 16;
        let samples_per_channel: u32 = 16;
        let bytes_per_sample = u32::from(bits_per_sample / 8);
        let data_size = samples_per_channel * u32::from(channels) * bytes_per_sample;
        let byte_rate = sample_rate * u32::from(channels) * bytes_per_sample;
        let block_align = channels * (bits_per_sample / 8);

        use std::io::Write as _;
        file.write_all(b"RIFF").expect("riff");
        file.write_all(&(36 + data_size).to_le_bytes())
            .expect("chunk size");
        file.write_all(b"WAVE").expect("wave");
        file.write_all(b"fmt ").expect("fmt");
        file.write_all(&16u32.to_le_bytes())
            .expect("fmt chunk size");
        file.write_all(&1u16.to_le_bytes()).expect("pcm");
        file.write_all(&channels.to_le_bytes()).expect("channels");
        file.write_all(&sample_rate.to_le_bytes()).expect("rate");
        file.write_all(&byte_rate.to_le_bytes()).expect("byte rate");
        file.write_all(&block_align.to_le_bytes())
            .expect("block align");
        file.write_all(&bits_per_sample.to_le_bytes())
            .expect("bits");
        file.write_all(b"data").expect("data");
        file.write_all(&data_size.to_le_bytes()).expect("data size");
        file.write_all(&vec![0u8; data_size as usize])
            .expect("samples");
    }

    fn make_executable(_path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(_path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(_path, permissions).expect("permissions");
        }
    }

    fn read_run_count(path: &Path) -> u32 {
        fs::read_to_string(path)
            .expect("count file")
            .trim()
            .parse()
            .expect("count should parse")
    }

    #[test]
    fn toolchain_discovers_test_target() {
        let repo_root = temp_workspace();
        let scripts_dir = repo_root.join("scripts");
        let build_bin = repo_root
            .join(".build/ffmpeg")
            .join(crate::ffmpeg::target_dir_name())
            .join("install/bin");
        fs::create_dir_all(&scripts_dir).expect("scripts dir");
        fs::create_dir_all(&build_bin).expect("toolchain dir");
        write_build_script(&build_script_path(&repo_root, "ffmpeg"));
        write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 1, Some("mono"));
        write_fake_ffmpeg(
            &fake_command_path(&build_bin, "ffmpeg"),
            &repo_root.join("ffmpeg.log"),
        );

        let toolchain = FfmpegToolchain::discover(&repo_root).expect("toolchain");

        assert_eq!(
            toolchain.ffmpeg_path,
            fake_command_path(&build_bin, "ffmpeg")
        );
        assert_eq!(
            toolchain.ffprobe_path,
            fake_command_path(&build_bin, "ffprobe")
        );
    }
}
