use super::super::router_with_repo_root;
use super::super::types::{DictionaryKeywordListResponse, ErrorResponse};
use super::support::{
    get_request, post_empty_request, post_json_request, read_json, temp_workspace,
};
use crate::index::{DictionaryKeywordSource, IndexStore};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use tower::util::ServiceExt;

#[tokio::test(flavor = "multi_thread")]
async fn dictionary_keyword_routes_support_crud() {
    let repo_root = temp_workspace();
    let app = router_with_repo_root(repo_root);

    let initial = app
        .clone()
        .oneshot(get_request("/dictionary/keywords"))
        .await
        .expect("initial dictionary response");
    assert_eq!(initial.status(), StatusCode::OK);
    let initial_body: DictionaryKeywordListResponse = read_json(initial).await;
    assert!(initial_body.user_keywords.is_empty());
    assert!(initial_body.auto_keywords.is_empty());

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
    assert_eq!(created_body.user_keywords, vec!["RecordRoute".to_string()]);
    assert!(created_body.auto_keywords.is_empty());

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
    assert!(deleted_body.user_keywords.is_empty());
    assert!(deleted_body.auto_keywords.is_empty());
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

#[tokio::test(flavor = "multi_thread")]
async fn dictionary_keyword_routes_support_auto_promote_and_delete() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    store
        .upsert_stt_dictionary_keyword("회의록", DictionaryKeywordSource::Auto)
        .expect("seed auto keyword");
    store
        .upsert_stt_dictionary_keyword("배포", DictionaryKeywordSource::Auto)
        .expect("seed auto keyword");
    store
        .upsert_stt_dictionary_keyword("액션아이템", DictionaryKeywordSource::Auto)
        .expect("seed auto keyword");
    let app = router_with_repo_root(repo_root);

    let promoted = app
        .clone()
        .oneshot(post_empty_request(
            "/dictionary/keywords/auto/%ED%9A%8C%EC%9D%98%EB%A1%9D/promote",
        ))
        .await
        .expect("promote dictionary response");
    assert_eq!(promoted.status(), StatusCode::OK);
    let promoted_body: DictionaryKeywordListResponse = read_json(promoted).await;
    assert_eq!(promoted_body.user_keywords, vec!["회의록".to_string()]);
    assert_eq!(
        sorted_strings(promoted_body.auto_keywords),
        sorted_strings(vec!["배포".to_string(), "액션아이템".to_string()])
    );

    let deleted = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/dictionary/keywords/auto/%EB%B0%B0%ED%8F%AC")
                .body(Body::empty())
                .expect("delete auto request"),
        )
        .await
        .expect("delete auto dictionary response");
    assert_eq!(deleted.status(), StatusCode::OK);
    let deleted_body: DictionaryKeywordListResponse = read_json(deleted).await;
    assert_eq!(deleted_body.user_keywords, vec!["회의록".to_string()]);
    assert_eq!(deleted_body.auto_keywords, vec!["액션아이템".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_all_auto_dictionary_keywords_returns_updated_keywords() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    store
        .upsert_stt_dictionary_keyword("RecordRoute", DictionaryKeywordSource::User)
        .expect("seed user keyword");
    store
        .upsert_stt_dictionary_keyword("회의록", DictionaryKeywordSource::Auto)
        .expect("seed auto keyword");
    store
        .upsert_stt_dictionary_keyword("배포", DictionaryKeywordSource::Auto)
        .expect("seed auto keyword");
    let app = router_with_repo_root(repo_root);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/dictionary/keywords/auto")
                .body(Body::empty())
                .expect("bulk delete auto request"),
        )
        .await
        .expect("bulk delete auto dictionary response");
    assert_eq!(response.status(), StatusCode::OK);

    let body: DictionaryKeywordListResponse = read_json(response).await;
    assert_eq!(body.user_keywords, vec!["RecordRoute".to_string()]);
    assert!(body.auto_keywords.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_all_auto_dictionary_keywords_is_idempotent_when_empty() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    store
        .upsert_stt_dictionary_keyword("RecordRoute", DictionaryKeywordSource::User)
        .expect("seed user keyword");
    let app = router_with_repo_root(repo_root);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/dictionary/keywords/auto")
                .body(Body::empty())
                .expect("bulk delete auto request"),
        )
        .await
        .expect("bulk delete auto dictionary response");
    assert_eq!(response.status(), StatusCode::OK);

    let body: DictionaryKeywordListResponse = read_json(response).await;
    assert_eq!(body.user_keywords, vec!["RecordRoute".to_string()]);
    assert!(body.auto_keywords.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn adding_existing_auto_keyword_promotes_it_to_user() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    store
        .upsert_stt_dictionary_keyword("회의록", DictionaryKeywordSource::Auto)
        .expect("seed auto keyword");
    store
        .upsert_stt_dictionary_keyword("배포", DictionaryKeywordSource::Auto)
        .expect("seed auto keyword");
    store
        .upsert_stt_dictionary_keyword("액션아이템", DictionaryKeywordSource::Auto)
        .expect("seed auto keyword");
    let app = router_with_repo_root(repo_root);

    let response = app
        .oneshot(post_json_request(
            "/dictionary/keywords",
            &serde_json::json!({ "keyword": "  회의록  " }),
        ))
        .await
        .expect("promote by create response");
    assert_eq!(response.status(), StatusCode::OK);

    let body: DictionaryKeywordListResponse = read_json(response).await;
    assert_eq!(body.user_keywords, vec!["회의록".to_string()]);
    assert_eq!(
        sorted_strings(body.auto_keywords),
        sorted_strings(vec!["배포".to_string(), "액션아이템".to_string()])
    );
}

fn sorted_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}
