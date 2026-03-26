use super::types::{AppState, PingRequest, PingResponse};
use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub(crate) async fn post_server_ping(
    State(state): State<AppState>,
    payload: Result<Json<PingRequest>, JsonRejection>,
) -> Response {
    match payload {
        Ok(Json(request)) => {
            let _request_code = request.code;
            let response = PingResponse {
                code: "200".to_string(),
                message: format!("Welcome... The message you sent - {}", request.message),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(state.invalid_request_body.clone()),
        )
            .into_response(),
    }
}
