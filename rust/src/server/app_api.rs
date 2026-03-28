use crate::app::{
    self, BatchQueueSubmission, FfmpegJobSubmission, ModelPrepareSubmission, ModelStatusSnapshot,
    StageJobSubmission, SummarySearchResult,
};
use crate::audio_store::ImportedSource;
use crate::error::{AppError, AppResult};
use crate::index::{ModelKind, ModelPreparationRecord};
use std::path::Path;

pub(crate) fn submit_ffmpeg_job(repo_root: &Path, input: &Path) -> AppResult<FfmpegJobSubmission> {
    app::submit_ffmpeg_job(repo_root, input)
}

pub(crate) fn submit_uploaded_ffmpeg_job(
    repo_root: &Path,
    imported: ImportedSource,
) -> AppResult<FfmpegJobSubmission> {
    app::submit_ffmpeg_job_from_imported_source(repo_root, imported)
}

pub(crate) fn submit_stt_job(
    repo_root: &Path,
    job_id: &str,
    subset_audio_files: Option<Vec<String>>,
) -> AppResult<StageJobSubmission> {
    app::submit_stt_job(repo_root, job_id, subset_audio_files)
}

pub(crate) fn submit_summary_job(
    repo_root: &Path,
    job_id: &str,
    force_regenerate: bool,
) -> AppResult<StageJobSubmission> {
    app::submit_summary_job(repo_root, job_id, force_regenerate)
}

pub(crate) fn submit_summary_embedding_job(
    repo_root: &Path,
    job_id: &str,
) -> AppResult<StageJobSubmission> {
    app::submit_summary_embedding_job(repo_root, job_id)
}

pub(crate) fn submit_batch_pipeline_jobs(repo_root: &Path) -> AppResult<BatchQueueSubmission> {
    app::submit_batch_pipeline_jobs(repo_root).map_err(AppError::internal)
}

pub(crate) fn search_summaries(
    repo_root: &Path,
    query: &str,
    limit: usize,
    min_score: Option<f32>,
) -> AppResult<Vec<SummarySearchResult>> {
    app::search_summaries(repo_root, query, limit, min_score)
}

pub(crate) fn submit_model_preparation(
    repo_root: &Path,
    model: ModelKind,
) -> AppResult<ModelPrepareSubmission> {
    app::submit_model_preparation_api(repo_root, model)
}

pub(crate) fn submit_llama_umbrella_preparation(
    repo_root: &Path,
) -> AppResult<ModelPrepareSubmission> {
    app::submit_llama_umbrella_preparation_api(repo_root)
}

pub(crate) fn execute_model_preparation(
    repo_root: &Path,
    model: ModelKind,
) -> AppResult<ModelPreparationRecord> {
    app::execute_model_preparation_api(repo_root, model)
}

pub(crate) fn execute_llama_umbrella_preparation(
    repo_root: &Path,
    submission: &ModelPrepareSubmission,
) -> AppResult<ModelPreparationRecord> {
    app::execute_llama_umbrella_preparation_api(repo_root, submission)
}

pub(crate) fn collect_model_status_snapshot(repo_root: &Path) -> AppResult<ModelStatusSnapshot> {
    app::collect_model_status_snapshot_api(repo_root)
}
