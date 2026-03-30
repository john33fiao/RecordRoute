use super::super::router_with_repo_root;
use super::support::{get_request, seed_frontend_build, temp_workspace};
use crate::test_support::{EnvVarGuard, env_lock};
use axum::http::StatusCode;
use http_body_util::BodyExt;
use tower::util::ServiceExt;

#[tokio::test(flavor = "multi_thread")]
async fn get_root_serves_html_shell_with_expected_sections() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _env_guard = EnvVarGuard::capture(crate::server::QUEUE_START_PAUSED_ENV_VAR);
    unsafe { std::env::set_var(crate::server::QUEUE_START_PAUSED_ENV_VAR, "1") };
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
    assert!(html.contains("id=\"dictionary-panel\""));
    assert!(html.contains("id=\"dictionary-form\""));
    assert!(html.contains("id=\"dictionary-list\""));
    assert!(html.contains("id=\"dictionary-refresh-button\""));
    assert!(html.contains("id=\"queue-pause-button\""));
    assert!(html.contains("id=\"queue-cancel-button\""));
    assert!(html.contains(".qta,audio/*"));
    assert!(html.contains("업로드는 ffmpeg 큐에만 등록"));
    assert!(html.contains(
        "사용자가 직접 등록한 키워드만 STT 실행 시 Whisper 초기 프롬프트에 자동 주입됩니다."
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_app_js_serves_script_asset() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _env_guard = EnvVarGuard::capture(crate::server::QUEUE_START_PAUSED_ENV_VAR);
    unsafe { std::env::set_var(crate::server::QUEUE_START_PAUSED_ENV_VAR, "1") };
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
    assert!(js.contains("async function onQueuePauseToggle"));
    assert!(js.contains("async function onQueueCancelPending"));
    assert!(js.contains("async function refreshDictionary"));
    assert!(js.contains("dictionary-refresh-button"));
    assert!(js.contains("pendingDictionaryRefreshJobId"));
    assert!(js.contains("queue-pause-button"));
    assert!(js.contains("queue-cancel-button"));
    assert!(js.contains("const QUEUE_COLLAPSE_THRESHOLD = 10;"));
    assert!(js.contains("각 파일이 ffmpeg 큐에 등록되었습니다."));
    assert!(js.contains("일괄처리로 미리 등록된 후속 단계"));
    assert!(js.contains("function onQueueBoardClick"));
    assert!(js.contains("function renderQueueBoard"));
    assert!(js.contains("function renderQueueControls"));
    assert!(js.contains("data-queue-toggle"));
    assert!(js.contains("function renderDictionary"));
    assert!(js.contains("function renderUserDictionaryChip"));
    assert!(js.contains("data-dictionary-action=\"promote-auto\""));
    assert!(js.contains("\".qta\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_app_css_serves_stylesheet_asset() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _env_guard = EnvVarGuard::capture(crate::server::QUEUE_START_PAUSED_ENV_VAR);
    unsafe { std::env::set_var(crate::server::QUEUE_START_PAUSED_ENV_VAR, "1") };
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
    assert!(css.contains(".panel-actions"));
    assert!(css.contains(".warning-button"));
    assert!(css.contains("grid-template-rows: auto minmax(0, 1fr);"));
    assert!(css.contains(".queue-column-toggle"));
    assert!(css.contains(".dictionary-form"));
    assert!(css.contains(".dictionary-chip"));
    assert!(css.contains(".dictionary-group"));
    assert!(css.contains(".dictionary-promote-button"));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_new_root_serves_built_frontend_index() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _env_guard = EnvVarGuard::capture(crate::server::QUEUE_START_PAUSED_ENV_VAR);
    unsafe { std::env::set_var(crate::server::QUEUE_START_PAUSED_ENV_VAR, "1") };
    let repo_root = temp_workspace();
    seed_frontend_build(&repo_root);
    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/new"))
        .await
        .expect("new root response");

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

    assert!(html.contains("id=\"root\""));
    assert!(html.contains("/new/assets/app.js"));
    assert!(html.contains("/new/assets/app.css"));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_new_assets_serves_built_frontend_asset() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _env_guard = EnvVarGuard::capture(crate::server::QUEUE_START_PAUSED_ENV_VAR);
    unsafe { std::env::set_var(crate::server::QUEUE_START_PAUSED_ENV_VAR, "1") };
    let repo_root = temp_workspace();
    seed_frontend_build(&repo_root);
    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/new/assets/app.js"))
        .await
        .expect("new asset response");

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

    assert!(js.contains("new frontend asset"));
}
