use super::super::router_with_repo_root;
use super::super::types::{DictionaryKeywordListResponse, ErrorResponse};
use super::support::{get_request, post_json_request, read_json, temp_workspace};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use tower::util::ServiceExt;

#[tokio::test(flavor = "multi_thread")]
async fn dictionary_keyword_routes_support_crud() {
    let app = router_with_repo_root(temp_workspace());

    let initial = app
        .clone()
        .oneshot(get_request("/dictionary/keywords"))
        .await
        .expect("initial dictionary response");
    assert_eq!(initial.status(), StatusCode::OK);
    let initial_body: DictionaryKeywordListResponse = read_json(initial).await;
    assert!(initial_body.keywords.is_empty());

    let created = app
        .clone()
        .oneshot(post_json_request(
            "/dictionary/keywords",
            &serde_json::json!({ "keyword": "  RecordRoute  " }),
        ))
        .await
        .expect("create dictionary response");
    assert_eq!(created.status(), StatusCode::OK);
    let created_body: DictionaryKeywordListResponse = read_json(created).await;
    assert_eq!(created_body.keywords, vec!["RecordRoute".to_string()]);

    let deleted = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/dictionary/keywords/RecordRoute")
                .body(Body::empty())
                .expect("delete request"),
        )
        .await
        .expect("delete dictionary response");
    assert_eq!(deleted.status(), StatusCode::OK);
    let deleted_body: DictionaryKeywordListResponse = read_json(deleted).await;
    assert!(deleted_body.keywords.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_dictionary_keyword_returns_404_when_missing() {
    let app = router_with_repo_root(temp_workspace());
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/dictionary/keywords/not-found")
                .body(Body::empty())
                .expect("delete request"),
        )
        .await
        .expect("delete dictionary response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: ErrorResponse = read_json(response).await;
    assert!(body.message.contains("dictionary keyword not found"));
}
