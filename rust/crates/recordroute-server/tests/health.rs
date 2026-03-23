use axum::body::Body;
use axum::http::{Request, StatusCode};
use recordroute_core::config::AppConfig;
use recordroute_server::{app, AppState};
use tower::util::ServiceExt;

#[tokio::test]
async fn health_endpoint_returns_expected_contract() {
    let app = app(AppState::from_config(AppConfig::from_env_iter([
        ("DB_FOLDER_PATH", "DB"),
        ("MODEL_ROOT_PATH", "models"),
    ])));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
    assert_eq!(json["db_root"], "DB");
    assert_eq!(json["model_root"], "models");
}
