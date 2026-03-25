use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

const SERVER_BIND: &str = "127.0.0.1:38080";

#[derive(Debug, Clone, Deserialize)]
pub struct PingRequest {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PingResponse {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone)]
struct AppState {
    invalid_request_body: PingResponse,
}

pub fn router() -> Router {
    Router::new()
        .route("/server/ping", post(post_server_ping))
        .with_state(AppState {
            invalid_request_body: PingResponse {
                code: "400".to_string(),
                message: "invalid request body".to_string(),
            },
        })
}

pub async fn serve() -> Result<(), String> {
    let listener = tokio::net::TcpListener::bind(SERVER_BIND)
        .await
        .map_err(|error| format!("failed to bind {SERVER_BIND}: {error}"))?;

    axum::serve(listener, router())
        .await
        .map_err(|error| format!("server error: {error}"))
}

async fn post_server_ping(
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use http_body_util::BodyExt;
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn ping_returns_expected_success_payload() {
        let app = router();
        let request = Request::builder()
            .method(Method::POST)
            .uri("/server/ping")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"code":"100","message":"ping test"}"#))
            .expect("request");

        let response = app.oneshot(request).await.expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect")
            .to_bytes();
        assert_eq!(
            std::str::from_utf8(&body).expect("utf8"),
            r#"{"code":"200","message":"Welcome... The message you sent - ping test"}"#
        );
    }

    #[tokio::test]
    async fn ping_returns_400_for_malformed_json() {
        let app = router();
        let request = Request::builder()
            .method(Method::POST)
            .uri("/server/ping")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"code":"100","message":"ping test""#))
            .expect("request");

        let response = app.oneshot(request).await.expect("response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect")
            .to_bytes();
        assert_eq!(
            std::str::from_utf8(&body).expect("utf8"),
            r#"{"code":"400","message":"invalid request body"}"#
        );
    }

    #[tokio::test]
    async fn ping_returns_400_when_message_is_missing() {
        let app = router();
        let request = Request::builder()
            .method(Method::POST)
            .uri("/server/ping")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"code":"100"}"#))
            .expect("request");

        let response = app.oneshot(request).await.expect("response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect")
            .to_bytes();
        assert_eq!(
            std::str::from_utf8(&body).expect("utf8"),
            r#"{"code":"400","message":"invalid request body"}"#
        );
    }

    #[tokio::test]
    async fn ping_returns_400_when_code_is_missing() {
        let app = router();
        let request = Request::builder()
            .method(Method::POST)
            .uri("/server/ping")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"message":"ping test"}"#))
            .expect("request");

        let response = app.oneshot(request).await.expect("response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect")
            .to_bytes();
        assert_eq!(
            std::str::from_utf8(&body).expect("utf8"),
            r#"{"code":"400","message":"invalid request body"}"#
        );
    }

    #[tokio::test]
    async fn ping_rejects_get_method() {
        let app = router();
        let request = Request::builder()
            .method(Method::GET)
            .uri("/server/ping")
            .body(Body::empty())
            .expect("request");

        let response = app.oneshot(request).await.expect("response");

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}
