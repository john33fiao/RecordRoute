#[path = "app/artifacts.rs"]
pub(crate) mod artifacts;

#[path = "app/cli.rs"]
mod cli;
#[path = "app/embedding_stage.rs"]
mod embedding_stage;
#[path = "app/ffmpeg_stage.rs"]
mod ffmpeg_stage;
#[path = "app/models.rs"]
mod models;
#[path = "app/stages.rs"]
mod stages;
#[path = "app/stt_stage.rs"]
mod stt_stage;
#[path = "app/summary_stage.rs"]
mod summary_stage;

use crate::ffmpeg::ConversionOutputs;
use crate::index::{JobRecord, ModelKind, ModelPreparationRecord};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingRunSummary {
    pub processed_jobs: Vec<(String, bool)>,
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

pub use cli::main_cli;
#[allow(unused_imports)]
pub use cli::resolve_input_path;
pub use embedding_stage::{
    SummarySearchResult, backfill_summary_embeddings, execute_summary_embedding_job,
    search_summaries, submit_summary_embedding_job,
};
pub use ffmpeg_stage::{execute_ffmpeg_job, run_with_repo_root, submit_ffmpeg_job};
#[allow(unused_imports)]
pub use models::wait_for_model_preparation;
pub use models::{
    collect_model_status_snapshot, ensure_model_prepared, execute_model_preparation,
    prepare_llama_model_with_repo_root, submit_model_preparation,
};
pub use stt_stage::{execute_stt_job, run_stt_with_repo_root, submit_stt_job};
pub use summary_stage::{execute_summary_job, run_summary_with_repo_root, submit_summary_job};

#[cfg(test)]
pub(crate) use cli::resolve_cli_command;

pub(crate) fn repo_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to resolve repository root".to_string())
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

    dotenvy::from_path(&env_path)
        .map_err(|error| format!("failed to load {}: {error}", env_path.display()))?;
    Ok(())
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

pub(crate) fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests;
