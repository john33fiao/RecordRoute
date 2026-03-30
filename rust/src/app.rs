#[path = "app/artifacts.rs"]
pub(crate) mod artifacts;

#[path = "app/cli.rs"]
mod cli;
#[path = "app/dictionary.rs"]
mod dictionary;
#[path = "app/embedding_stage.rs"]
mod embedding_stage;
#[path = "app/ffmpeg_stage.rs"]
mod ffmpeg_stage;
#[path = "app/models.rs"]
mod models;
#[path = "app/queue.rs"]
mod queue;
#[path = "app/stages.rs"]
mod stages;
#[path = "app/stt_stage.rs"]
mod stt_stage;
#[path = "app/summary_stage.rs"]
mod summary_stage;

use crate::ffmpeg::ConversionOutputs;
use crate::index::{JobRecord, ModelKind, ModelPreparationRecord};
use crate::runtime_root;
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use uuid::Uuid;

const RUN_ID_FORMAT: &[time::format_description::FormatItem<'static>] =
    format_description!("[year][month][day]T[hour][minute][second]");
pub(crate) const WAIT_FOR_RUNNING_JOB_POLL_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(100);
pub(crate) const MODEL_PREPARATION_WAIT_POLL_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(100);
pub(crate) const MODEL_PREPARATION_HEARTBEAT_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(10);
pub(crate) const MODEL_PREPARATION_STALE_THRESHOLD_SECS: i64 = 60;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageJobDisposition {
    Submitted,
    Reused,
    Deduplicated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageJobSubmission {
    pub job: JobRecord,
    pub disposition: StageJobDisposition,
    pub planned_audio_files: Vec<PathBuf>,
    pub queue: Option<QueueTicket>,
}

impl StageJobSubmission {
    pub fn reused(&self) -> bool {
        self.disposition == StageJobDisposition::Reused
    }

    pub fn deduplicated(&self) -> bool {
        self.disposition == StageJobDisposition::Deduplicated
    }

    pub fn should_execute(&self) -> bool {
        self.disposition == StageJobDisposition::Submitted
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfmpegJobDisposition {
    Submitted,
    Reused,
    Deduplicated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfmpegJobSubmission {
    pub job: JobRecord,
    pub input_path: PathBuf,
    pub disposition: FfmpegJobDisposition,
    pub queue: Option<QueueTicket>,
}

impl FfmpegJobSubmission {
    pub fn reused(&self) -> bool {
        self.disposition == FfmpegJobDisposition::Reused
    }

    pub fn deduplicated(&self) -> bool {
        self.disposition == FfmpegJobDisposition::Deduplicated
    }

    pub fn should_execute(&self) -> bool {
        self.disposition == FfmpegJobDisposition::Submitted
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelPrepareDisposition {
    Submitted,
    AlreadyReady,
    Deduplicated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPrepareSubmission {
    pub model: ModelKind,
    pub disposition: ModelPrepareDisposition,
    pub preparation: ModelPreparationRecord,
    pub execute_requested: bool,
    pub execute_embedding_requested: bool,
}

impl ModelPrepareSubmission {
    pub fn already_ready(&self) -> bool {
        self.disposition == ModelPrepareDisposition::AlreadyReady
    }

    pub fn deduplicated(&self) -> bool {
        self.disposition == ModelPrepareDisposition::Deduplicated
    }

    pub fn should_execute(&self) -> bool {
        self.disposition == ModelPrepareDisposition::Submitted
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelStatusEntry {
    pub model: ModelKind,
    pub available: bool,
    pub ready: bool,
    pub error: Option<String>,
    pub embedding_available: bool,
    pub embedding_ready: bool,
    pub embedding_error: Option<String>,
    pub preparation: ModelPreparationRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelStatusSnapshot {
    pub whisper: ModelStatusEntry,
    pub llama: ModelStatusEntry,
}

impl ModelStatusSnapshot {
    pub fn entry(&self, model: ModelKind) -> &ModelStatusEntry {
        match model {
            ModelKind::Whisper => &self.whisper,
            ModelKind::Llama => &self.llama,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BatchQueueSubmission {
    pub total_jobs: usize,
    pub ffmpeg_queued: usize,
    pub stt_queued: usize,
    pub summary_queued: usize,
    pub embedding_queued: usize,
}

pub use cli::main_cli;
#[allow(unused_imports)]
pub use cli::resolve_input_path;
pub use dictionary::{
    add_stt_dictionary_keyword, delete_auto_stt_dictionary_keyword, delete_stt_dictionary_keyword,
    list_stt_dictionary_keywords, promote_auto_stt_dictionary_keyword,
};
pub use embedding_stage::{
    SummarySearchResult, backfill_summary_embeddings, search_summaries,
    submit_summary_embedding_job,
};
pub use ffmpeg_stage::{
    run_with_repo_root, submit_ffmpeg_job, submit_ffmpeg_job_from_imported_source,
};
#[allow(unused_imports)]
pub use models::wait_for_model_preparation;
pub use models::{
    collect_model_status_snapshot, ensure_model_prepared, prepare_llama_model_with_repo_root,
    prepare_models_with_repo_root,
};
pub use models::{
    collect_model_status_snapshot_api, execute_llama_umbrella_preparation_api,
    execute_model_preparation_api, submit_llama_umbrella_preparation_api,
    submit_model_preparation_api,
};
pub use stt_stage::{run_stt_with_repo_root, submit_stt_job};
pub use summary_stage::{run_summary_with_repo_root, submit_summary_job};

#[cfg(test)]
pub(crate) use cli::resolve_cli_command;
#[cfg(test)]
pub(crate) use models::submit_model_preparation;

pub(crate) fn repo_root() -> Result<PathBuf, String> {
    runtime_root::resolve_runtime_root()
}

pub(crate) fn read_line(reader: &mut dyn BufRead, error_context: &str) -> Result<String, String> {
    let mut line = String::new();
    let bytes_read = reader
        .read_line(&mut line)
        .map_err(|error| format!("{error_context}: {error}"))?;
    if bytes_read == 0 {
        return Err(format!("{error_context}: reached end of input"));
    }

    Ok(line)
}

pub(crate) fn load_repo_env(repo_root: &Path) -> Result<(), String> {
    let env_path = repo_root.join(".env");
    if !env_path.is_file() {
        return Ok(());
    }

    load_dotenv_path(&env_path)
        .map_err(|error| format!("failed to load {}: {error}", env_path.display()))?;
    Ok(())
}

fn load_dotenv_path(env_path: &Path) -> dotenvy::Result<()> {
    let contents = fs::read_to_string(env_path).map_err(dotenvy::Error::Io)?;
    let entries = match collect_dotenv_entries(dotenvy::from_read_iter(contents.as_bytes())) {
        Ok(entries) => entries,
        Err(error @ dotenvy::Error::LineParse(_, _)) => {
            let Some(normalized) = normalize_windows_path_env_contents(&contents) else {
                return Err(error);
            };
            match collect_dotenv_entries(dotenvy::from_read_iter(normalized.as_bytes())) {
                Ok(entries) => entries,
                Err(_) => return Err(error),
            }
        }
        Err(error) => return Err(error),
    };
    apply_dotenv_entries(entries);
    Ok(())
}

fn collect_dotenv_entries<R: std::io::Read>(
    iter: dotenvy::Iter<R>,
) -> dotenvy::Result<Vec<(String, String)>> {
    iter.collect()
}

fn apply_dotenv_entries(entries: Vec<(String, String)>) {
    for (key, value) in entries {
        if std::env::var_os(&key).is_none() {
            unsafe { std::env::set_var(key, value) };
        }
    }
}

fn normalize_windows_path_env_contents(contents: &str) -> Option<String> {
    let mut changed = false;
    let mut normalized = String::with_capacity(contents.len());

    for segment in contents.split_inclusive('\n') {
        let (line, newline) = if let Some(stripped) = segment.strip_suffix("\r\n") {
            (stripped, "\r\n")
        } else if let Some(stripped) = segment.strip_suffix('\n') {
            (stripped, "\n")
        } else {
            (segment, "")
        };
        let normalized_line = normalize_windows_path_env_line(line);
        if normalized_line != line {
            changed = true;
        }
        normalized.push_str(&normalized_line);
        normalized.push_str(newline);
    }

    changed.then_some(normalized)
}

fn normalize_windows_path_env_line(line: &str) -> String {
    let trimmed_start = line.trim_start_matches(|ch: char| ch == ' ' || ch == '\t');
    if trimmed_start.is_empty() || trimmed_start.starts_with('#') {
        return line.to_string();
    }

    let Some((lhs, rhs)) = line.split_once('=') else {
        return line.to_string();
    };
    let value_start = rhs
        .find(|ch: char| ch != ' ' && ch != '\t')
        .unwrap_or(rhs.len());
    let value_end = rhs
        .rfind(|ch: char| ch != ' ' && ch != '\t')
        .map(|index| index + 1)
        .unwrap_or(value_start);
    let value = &rhs[value_start..value_end];
    let Some(normalized_value) = normalize_windows_path_env_value(value) else {
        return line.to_string();
    };

    format!(
        "{lhs}={}{normalized_value}{}",
        &rhs[..value_start],
        &rhs[value_end..]
    )
}

fn normalize_windows_path_env_value(value: &str) -> Option<String> {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        let inner = &value[1..value.len() - 1];
        return normalize_windows_path_literal(inner).map(|path| format!("'{path}'"));
    }
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return None;
    }

    normalize_windows_path_literal(value).map(|path| format!("'{path}'"))
}

fn normalize_windows_path_literal(value: &str) -> Option<&str> {
    if !is_windows_absolute_path(value) {
        return None;
    }
    if value
        .chars()
        .any(|ch| matches!(ch, '\'' | '#' | '\r' | '\n' | '$'))
    {
        return None;
    }

    Some(value)
}

fn is_windows_absolute_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\'
}

pub(crate) fn build_run_id() -> Result<String, String> {
    let timestamp = OffsetDateTime::now_utc()
        .format(RUN_ID_FORMAT)
        .map_err(|error| format!("failed to format timestamp: {error}"))?;
    Ok(format!("{timestamp}_{}", Uuid::now_v7()))
}

pub fn now_rfc3339() -> Result<String, String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| format!("failed to format timestamp: {error}"))
}

#[cfg(test)]
mod tests;
pub use queue::{
    DispatchState, QueueCancelPendingResult, QueueTicket, cancel_pending_entries, dispatch_one,
    queue_snapshot, recover_interrupted_active_entry, set_queue_paused, submit_batch_pipeline_jobs,
};
