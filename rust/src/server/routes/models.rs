use super::app_api;
use super::types::{
    AppState, SystemStatusResponse, build_model_prepare_response, build_model_status_response,
};
use super::{error_response, run_blocking, run_blocking_app};
use crate::ffmpeg::Toolchain as FfmpegToolchain;
use crate::index::ModelKind;
use crate::llama::{ModelSource as LlamaModelSource, Toolchain as LlamaToolchain};
use crate::whisper::Toolchain as WhisperToolchain;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::path::Path;

pub(crate) async fn get_system_status(State(state): State<AppState>) -> Response {
    let repo_root = state.repo_root.clone();
    match run_blocking(move || gather_system_status(&repo_root)).await {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(crate::error::AppError::internal(error)),
    }
}

pub(crate) async fn get_models_status(State(state): State<AppState>) -> Response {
    let repo_root = state.repo_root.clone();
    match run_blocking_app(move || app_api::collect_model_status_snapshot(&repo_root)).await {
        Ok(status) => Json(build_model_status_response(status)).into_response(),
        Err(error) => error_response(error),
    }
}

pub(crate) async fn post_prepare_whisper_model(State(state): State<AppState>) -> Response {
    post_prepare_model(state, ModelKind::Whisper).await
}

pub(crate) async fn post_prepare_llama_model(State(state): State<AppState>) -> Response {
    post_prepare_model(state, ModelKind::Llama).await
}

async fn post_prepare_model(state: AppState, model: ModelKind) -> Response {
    let repo_root = state.repo_root.clone();
    let submission = match run_blocking_app(move || {
        app_api::submit_model_preparation(&repo_root, model)
    })
    .await
    {
        Ok(submission) => submission,
        Err(error) => return error_response(error),
    };

    if submission.should_execute() {
        let repo_root = state.repo_root.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(error) = app_api::execute_model_preparation(&repo_root, model) {
                eprintln!("{error}");
            }
        });
    }

    let status = if submission.already_ready() {
        StatusCode::OK
    } else {
        StatusCode::ACCEPTED
    };
    (status, Json(build_model_prepare_response(&submission))).into_response()
}

fn gather_system_status(repo_root: &Path) -> Result<SystemStatusResponse, String> {
    let mut errors = Vec::new();

    let ffmpeg_available = match FfmpegToolchain::discover(repo_root) {
        Ok(_) => true,
        Err(error) => {
            errors.push(error);
            false
        }
    };

    let whisper_discovery = WhisperToolchain::discover(repo_root);
    let whisper_available = whisper_discovery.is_ok();
    if let Err(error) = &whisper_discovery {
        errors.push(error.clone());
    }
    let whisper_model_ready = whisper_discovery
        .as_ref()
        .ok()
        .is_some_and(WhisperToolchain::is_model_ready);
    if whisper_available
        && !whisper_model_ready
        && let Ok(toolchain) = &whisper_discovery
    {
        errors.push(format!(
            "whisper model file not found: {}",
            toolchain.model_path.display()
        ));
    }

    let llama_discovery = LlamaToolchain::discover(repo_root);
    let llama_available = llama_discovery.is_ok();
    if let Err(error) = &llama_discovery {
        errors.push(error.clone());
    }
    let llama_model_ready = llama_discovery
        .as_ref()
        .ok()
        .is_some_and(LlamaToolchain::is_model_ready);
    if llama_available
        && !llama_model_ready
        && let Ok(toolchain) = &llama_discovery
    {
        errors.push(llama_model_missing_message(toolchain));
    }

    Ok(SystemStatusResponse {
        ffmpeg_available,
        whisper_available,
        llama_available,
        whisper_model_ready,
        llama_model_ready,
        errors,
    })
}

fn llama_model_missing_message(toolchain: &LlamaToolchain) -> String {
    match (&toolchain.model_source, &toolchain.cached_model_path) {
        (LlamaModelSource::LocalPath(path), _) => {
            format!("llama model file not found: {}", path.display())
        }
        (LlamaModelSource::HuggingFaceRepo(repo), Some(cache_path)) => format!(
            "llama model cache not found for {repo}: {}",
            cache_path.display()
        ),
        (LlamaModelSource::HuggingFaceRepo(repo), None) => {
            format!(
                "llama model cache path is unavailable for configured Hugging Face repo: {repo}"
            )
        }
    }
}
