use super::super::router_with_repo_root;
use super::support::{get_request, temp_workspace};
use axum::http::StatusCode;
use http_body_util::BodyExt;
use tower::util::ServiceExt;

#[tokio::test(flavor = "multi_thread")]
async fn get_root_serves_html_shell_with_expected_sections() {
    let app = router_with_repo_root(temp_workspace());
    let response = app.oneshot(get_request("/")).await.expect("root response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/html; charset=utf-8")
    );

    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect html body")
        .to_bytes();
    let html = String::from_utf8(body.to_vec()).expect("utf-8 html");

    assert!(html.contains("id=\"upload-panel\""));
    assert!(html.contains("id=\"jobs-panel\""));
    assert!(html.contains("id=\"search-panel\""));
    assert!(html.contains("id=\"queue-panel\""));
    assert!(html.contains("id=\"dictionary-form\""));
    assert!(html.contains("id=\"dictionary-list\""));
    assert!(html.contains(
        "사용자가 직접 등록한 키워드만 STT 실행 시 Whisper 초기 프롬프트에 자동 주입됩니다."
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_app_js_serves_script_asset() {
    let app = router_with_repo_root(temp_workspace());
    let response = app
        .oneshot(get_request("/app.js"))
        .await
        .expect("js response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/javascript; charset=utf-8")
    );

    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect js body")
        .to_bytes();
    let js = String::from_utf8(body.to_vec()).expect("utf-8 js");

    assert!(js.contains("const state ="));
    assert!(js.contains("async function refreshJobs"));
    assert!(js.contains("async function refreshQueue"));
    assert!(js.contains("async function refreshDictionary"));
    assert!(js.contains("function renderQueueBoard"));
    assert!(js.contains("function renderDictionary"));
    assert!(js.contains("function renderUserDictionaryChip"));
    assert!(js.contains("data-dictionary-action=\"promote-auto\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_app_css_serves_stylesheet_asset() {
    let app = router_with_repo_root(temp_workspace());
    let response = app
        .oneshot(get_request("/app.css"))
        .await
        .expect("css response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/css; charset=utf-8")
    );

    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect css body")
        .to_bytes();
    let css = String::from_utf8(body.to_vec()).expect("utf-8 css");

    assert!(css.contains(".panel"));
    assert!(css.contains(".jobs-list"));
    assert!(css.contains(".queue-board"));
    assert!(css.contains(".dictionary-form"));
    assert!(css.contains(".dictionary-chip"));
    assert!(css.contains(".dictionary-group"));
    assert!(css.contains(".dictionary-promote-button"));
}
