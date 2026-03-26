use super::super::router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
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
