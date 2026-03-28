use super::app_api;
use super::types::{
    AppState, SystemStatusResponse, build_model_prepare_response, build_model_status_response,
};
use super::{error_response, run_blocking, run_blocking_app};
use crate::app;
use crate::error::sanitize_dependency_message;
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
    let repo_root = state.repo_root.clone();
    let submission = match run_blocking_app(move || {
        app_api::submit_llama_umbrella_preparation(&repo_root)
    })
    .await
    {
        Ok(submission) => submission,
        Err(error) => return error_response(error),
    };

    if submission.should_execute() {
        let repo_root = state.repo_root.clone();
        let submission_for_execution = submission.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(error) =
                app_api::execute_llama_umbrella_preparation(&repo_root, &submission_for_execution)
            {
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
            push_unique_error(&mut errors, error);
            false
        }
    };

    let whisper_discovery = WhisperToolchain::discover(repo_root);
    let whisper_available = whisper_discovery.is_ok();
    if let Err(error) = &whisper_discovery {
        push_unique_error(&mut errors, error.clone());
    }

    let llama_discovery = LlamaToolchain::discover(repo_root);
    let llama_available = llama_discovery.is_ok();
    if let Err(error) = &llama_discovery {
        push_unique_error(&mut errors, error.clone());
    }

    let model_status = app::collect_model_status_snapshot(repo_root)?;
    let whisper_model_ready = model_status.whisper.ready;
    let llama_model_ready = model_status.llama.ready;
    let llama_embedding_model_ready = model_status.llama.embedding_ready;

    if whisper_available && !whisper_model_ready {
        if let Some(error) = model_status.whisper.error {
            push_unique_error(&mut errors, error);
        }
    }

    if llama_available && !llama_model_ready {
        if let Some(error) = model_status.llama.error.clone() {
            push_unique_error(&mut errors, error);
        } else if let Ok(toolchain) = &llama_discovery {
            push_unique_error(&mut errors, llama_model_missing_message(toolchain));
        }
    }

    if llama_available && !llama_embedding_model_ready {
        if let Some(error) = model_status.llama.embedding_error.clone() {
            push_unique_error(&mut errors, error);
        }
    }

    Ok(SystemStatusResponse {
        ffmpeg_available,
        whisper_available,
        llama_available,
        whisper_model_ready,
        llama_model_ready,
        llama_embedding_model_ready,
        errors: super::errors::sanitize_system_errors(errors),
    })
}

fn push_unique_error(errors: &mut Vec<String>, error: String) {
    let error = sanitize_dependency_message(&error);
    if !errors.iter().any(|existing| existing == &error) {
        errors.push(error);
    }
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
