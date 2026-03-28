use crate::app::{
    self, BatchQueueSubmission, FfmpegJobSubmission, ModelPrepareSubmission, ModelStatusSnapshot,
    StageJobSubmission, SummarySearchResult,
};
use crate::error::{AppError, AppResult};
use crate::index::{ModelKind, ModelPreparationRecord};
use std::path::Path;

pub(crate) fn submit_ffmpeg_job(repo_root: &Path, input: &Path) -> AppResult<FfmpegJobSubmission> {
    app::submit_ffmpeg_job(repo_root, input).map_err(classify_create_job_error)
}

pub(crate) fn submit_stt_job(
    repo_root: &Path,
    job_id: &str,
    subset_audio_files: Option<Vec<String>>,
) -> AppResult<StageJobSubmission> {
    app::submit_stt_job(repo_root, job_id, subset_audio_files)
        .map_err(classify_stage_submission_error)
}

pub(crate) fn submit_summary_job(
    repo_root: &Path,
    job_id: &str,
    force_regenerate: bool,
) -> AppResult<StageJobSubmission> {
    app::submit_summary_job(repo_root, job_id, force_regenerate)
        .map_err(classify_stage_submission_error)
}

pub(crate) fn submit_summary_embedding_job(
    repo_root: &Path,
    job_id: &str,
) -> AppResult<StageJobSubmission> {
    app::submit_summary_embedding_job(repo_root, job_id).map_err(classify_stage_submission_error)
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
    app::search_summaries(repo_root, query, limit, min_score).map_err(|error| {
        if super::errors::is_setup_related_error(&error) {
            AppError::dependency_unavailable(error)
        } else {
            AppError::internal(error)
        }
    })
}

pub(crate) fn submit_model_preparation(
    repo_root: &Path,
    model: ModelKind,
) -> AppResult<ModelPrepareSubmission> {
    app::submit_model_preparation(repo_root, model).map_err(classify_model_prepare_error)
}

pub(crate) fn submit_llama_umbrella_preparation(
    repo_root: &Path,
) -> AppResult<ModelPrepareSubmission> {
    app::submit_llama_umbrella_preparation(repo_root).map_err(classify_model_prepare_error)
}

pub(crate) fn execute_model_preparation(
    repo_root: &Path,
    model: ModelKind,
) -> AppResult<ModelPreparationRecord> {
    app::execute_model_preparation(repo_root, model).map_err(classify_model_prepare_error)
}

pub(crate) fn execute_llama_umbrella_preparation(
    repo_root: &Path,
    submission: &ModelPrepareSubmission,
) -> AppResult<ModelPreparationRecord> {
    app::execute_llama_umbrella_preparation(repo_root, submission)
        .map_err(classify_model_prepare_error)
}

pub(crate) fn collect_model_status_snapshot(repo_root: &Path) -> AppResult<ModelStatusSnapshot> {
    app::collect_model_status_snapshot(repo_root).map_err(AppError::internal)
}

fn classify_create_job_error(error: String) -> AppError {
    if error.starts_with("input file not found:")
        || error.starts_with("input path is not a file:")
        || error.starts_with("failed to resolve input path ")
    {
        AppError::bad_request(error)
    } else if super::errors::is_setup_related_error(&error) {
        AppError::dependency_unavailable(error)
    } else {
        AppError::internal(error)
    }
}

fn classify_stage_submission_error(error: String) -> AppError {
    if error.starts_with("job not found:") {
        AppError::not_found(error)
    } else {
        AppError::bad_request(error)
    }
}

fn classify_model_prepare_error(error: String) -> AppError {
    if super::errors::is_setup_related_error(&error) {
        AppError::dependency_unavailable(error)
    } else {
        AppError::internal(error)
    }
}
