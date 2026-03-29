use super::app_api;
use super::types::{AppState, DictionaryKeywordListResponse, DictionaryKeywordRequest};
use super::{error_response, run_blocking_app};
use crate::error::AppError;
use axum::Json;
use axum::extract::Path as AxumPath;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub(crate) async fn get_stt_dictionary_keywords(State(state): State<AppState>) -> Response {
    let repo_root = state.repo_root.clone();
    match run_blocking_app(move || app_api::list_stt_dictionary_keywords(&repo_root)).await {
        Ok(keywords) => Json(DictionaryKeywordListResponse::from(keywords)).into_response(),
        Err(error) => error_response(error),
    }
}

pub(crate) async fn post_stt_dictionary_keyword(
    State(state): State<AppState>,
    payload: Result<Json<DictionaryKeywordRequest>, JsonRejection>,
) -> Response {
    let keyword = match payload {
        Ok(Json(request)) => request.keyword,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(state.invalid_request_body.clone()),
            )
                .into_response();
        }
    };

    let repo_root = state.repo_root.clone();
    match run_blocking_app(move || app_api::add_stt_dictionary_keyword(&repo_root, keyword)).await {
        Ok(keywords) => Json(DictionaryKeywordListResponse::from(keywords)).into_response(),
        Err(error) => error_response(error),
    }
}

pub(crate) async fn delete_stt_dictionary_keyword(
    State(state): State<AppState>,
    AxumPath(keyword): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let delete_keyword = keyword.clone();
    let deleted = match run_blocking_app(move || {
        app_api::delete_stt_dictionary_keyword(&repo_root, delete_keyword)
    })
    .await
    {
        Ok(deleted) => deleted,
        Err(error) => return error_response(error),
    };

    if !deleted {
        return error_response(AppError::not_found(format!(
            "dictionary keyword not found: {keyword}"
        )));
    }

    let repo_root = state.repo_root.clone();
    match run_blocking_app(move || app_api::list_stt_dictionary_keywords(&repo_root)).await {
        Ok(keywords) => Json(DictionaryKeywordListResponse::from(keywords)).into_response(),
        Err(error) => error_response(error),
    }
}

pub(crate) async fn post_promote_auto_stt_dictionary_keyword(
    State(state): State<AppState>,
    AxumPath(keyword): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let promote_keyword = keyword.clone();
    let promoted = match run_blocking_app(move || {
        app_api::promote_auto_stt_dictionary_keyword(&repo_root, promote_keyword)
    })
    .await
    {
        Ok(promoted) => promoted,
        Err(error) => return error_response(error),
    };

    if !promoted {
        return error_response(AppError::not_found(format!(
            "auto dictionary keyword not found: {keyword}"
        )));
    }

    let repo_root = state.repo_root.clone();
    match run_blocking_app(move || app_api::list_stt_dictionary_keywords(&repo_root)).await {
        Ok(keywords) => Json(DictionaryKeywordListResponse::from(keywords)).into_response(),
        Err(error) => error_response(error),
    }
}

pub(crate) async fn delete_auto_stt_dictionary_keyword(
    State(state): State<AppState>,
    AxumPath(keyword): AxumPath<String>,
) -> Response {
    let repo_root = state.repo_root.clone();
    let delete_keyword = keyword.clone();
    let deleted = match run_blocking_app(move || {
        app_api::delete_auto_stt_dictionary_keyword(&repo_root, delete_keyword)
    })
    .await
    {
        Ok(deleted) => deleted,
        Err(error) => return error_response(error),
    };

    if !deleted {
        return error_response(AppError::not_found(format!(
            "auto dictionary keyword not found: {keyword}"
        )));
    }

    let repo_root = state.repo_root.clone();
    match run_blocking_app(move || app_api::list_stt_dictionary_keywords(&repo_root)).await {
        Ok(keywords) => Json(DictionaryKeywordListResponse::from(keywords)).into_response(),
        Err(error) => error_response(error),
    }
}
