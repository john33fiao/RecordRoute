use super::types::{AppState, build_stats_overview_response};
use super::{error_response, run_blocking};
use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};

pub(crate) async fn get_stats_overview(State(state): State<AppState>) -> Response {
    let repo_root = state.repo_root.clone();
    match run_blocking(move || crate::app::collect_stats_overview(&repo_root)).await {
        Ok(overview) => Json(build_stats_overview_response(overview)).into_response(),
        Err(error) => error_response(crate::error::AppError::internal(error)),
    }
}
