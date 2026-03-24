use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::models::{Job, NewRecording, ProcessingStatus, RecordingBundle, RecordingListItem, SearchResult};
use crate::AppState;

pub fn router(state: AppState) -> Router {
    let max_upload_size = state.config.max_upload_size_bytes;

    Router::new()
        .route("/v1/recordings", post(upload_recording).get(list_recordings))
        .route("/v1/recordings/:recording_id", get(get_recording))
        .route("/v1/jobs/:job_id", get(get_job))
        .route("/v1/search", get(search_recordings))
        .layer(DefaultBodyLimit::max(max_upload_size))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[derive(Debug, Serialize)]
struct UploadResponse {
    recording_id: Uuid,
    job_id: Uuid,
    status: ProcessingStatus,
    step: crate::models::ProcessingStep,
}

#[derive(Debug, Serialize)]
struct RecordingListResponse {
    items: Vec<RecordingListItem>,
}

#[derive(Debug, Serialize)]
struct SearchResponse {
    items: Vec<SearchResult>,
}

#[derive(Debug, Deserialize)]
struct RecordingListQuery {
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    query: String,
    limit: Option<usize>,
}

async fn upload_recording(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> ApiResult<Json<UploadResponse>> {
    let recording_id = Uuid::new_v4();
    let mut upload = None;

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::bad_request(format!("invalid multipart body: {error}")))?
    {
        let Some(file_name) = field.file_name().map(str::to_owned) else {
            continue;
        };
        let content_type = field.content_type().map(str::to_string);
        let saved = state
            .file_store
            .save_upload_field(
                recording_id,
                &file_name,
                content_type.as_deref(),
                &mut field,
                state.config.max_upload_size_bytes,
            )
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;

        upload = Some((file_name, content_type, saved));
        break;
    }

    let Some((file_name, content_type, saved)) = upload else {
        return Err(ApiError::bad_request("multipart payload must include a file field"));
    };

    let result = state
        .repo
        .insert_recording_with_job(NewRecording {
            id: recording_id,
            original_filename: file_name,
            original_content_type: content_type,
            file_size_bytes: saved.file_size_bytes,
            original_rel_path: saved.relative_path,
        })
        .await;

    let (_, job) = match result {
        Ok(value) => value,
        Err(error) => {
            let _ = state.file_store.remove_recording_dir(recording_id).await;
            return Err(ApiError::internal(error));
        }
    };

    state.worker_notify.notify_one();

    Ok(Json(UploadResponse {
        recording_id,
        job_id: job.id,
        status: job.status,
        step: job.step,
    }))
}

async fn get_job(State(state): State<AppState>, Path(job_id): Path<Uuid>) -> ApiResult<Json<Job>> {
    let job = state
        .repo
        .get_job(job_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found(format!("job `{job_id}` was not found")))?;
    Ok(Json(job))
}

async fn get_recording(
    State(state): State<AppState>,
    Path(recording_id): Path<Uuid>,
) -> ApiResult<Json<RecordingBundle>> {
    let recording = state
        .repo
        .get_recording_bundle(recording_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found(format!("recording `{recording_id}` was not found")))?;
    Ok(Json(recording))
}

async fn list_recordings(
    State(state): State<AppState>,
    Query(query): Query<RecordingListQuery>,
) -> ApiResult<Json<RecordingListResponse>> {
    let status = match query.status {
        Some(raw) => Some(
            raw.parse::<ProcessingStatus>()
                .map_err(|error| ApiError::bad_request(format!("invalid status filter: {error}")))?,
        ),
        None => None,
    };

    let items = state
        .repo
        .list_recordings(status)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(RecordingListResponse { items }))
}

async fn search_recordings(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Json<SearchResponse>> {
    let normalized_query = query.query.trim();
    if normalized_query.is_empty() {
        return Err(ApiError::bad_request("query must not be empty"));
    }

    let requested_limit = query.limit.unwrap_or(10);
    let limit = requested_limit.min(state.config.max_search_limit) as i64;

    let query_embedding = match state.embedding_client.embed(normalized_query).await {
        Ok(embedding) => Some(embedding),
        Err(error) => {
            tracing::warn!("embedding search fallback to keyword-only mode: {error}");
            None
        }
    };

    let items = state
        .repo
        .search_recordings(normalized_query, query_embedding.as_deref(), limit)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(SearchResponse { items }))
}
