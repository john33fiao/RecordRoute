use super::super::router_with_repo_root;
use super::support::{get_request, temp_workspace};
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
    assert!(html.contains("Statistics"));
    assert!(html.contains("upload-submit-button"));
    assert!(html.contains("upload-queue-pause-button"));
    assert!(html.contains("data-queue-pause-toggle"));
    assert!(html.contains("\u{C131}\u{C744} \u{D68C}\u{C758}\u{B85D}\u{C73C}\u{B85C}, STT\u{00B7}\u{C694}\u{C57D}\u{00B7}\u{C784}\u{BCA0}\u{B529}\u{00B7}RAG \u{C9C8}\u{C758} \u{C9C0}\u{C6D0}\u{AE4C}\u{C9C0} \u{C77C}\u{C6D0}\u{D654}."));
    assert!(html.contains(
        "\u{C624}\u{B514}\u{C624}-\u{D14D}\u{C2A4}\u{D2B8} \u{BCC0}\u{D658} \u{C815}\u{D655}\u{B3C4}\u{B97C} \u{B192}\u{C774}\u{AE30} \u{C704}\u{D574}, \u{C8FC}\u{B85C} \u{C0AC}\u{C6A9}\u{B418}\u{B294} \u{D0A4}\u{C6CC}\u{B4DC}\u{B97C} \u{AD00}\u{B9AC}\u{D560} \u{C218} \u{C788}\u{C2B5}\u{B2C8}\u{B2E4}."
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
    assert!(js.contains(
        "const TAB_KEYS = [\"upload\", \"jobs\", \"stats\", \"search\", \"queue\", \"dictionary\"];"
    ));
    assert!(js.contains("async function refreshJobs"));
    assert!(js.contains("async function refreshStats"));
    assert!(js.contains("async function refreshQueue"));
    assert!(js.contains("function renderTabs()"));
    assert!(js.contains("function openSettingsModal()"));
    assert!(js.contains("function renderSettingsModal()"));
    assert!(js.contains("async function onQueuePauseToggle"));
    assert!(js.contains("async function onQueueCancelPending"));
    assert!(js.contains("async function refreshDictionary"));
    assert!(js.contains("window.addEventListener(\"hashchange\", onHashChange);"));
    assert!(js.contains("pendingDictionaryRefreshJobId"));
    assert!(js.contains("queue-cancel-button"));
    assert!(js.contains("document.querySelectorAll(\"[data-queue-pause-toggle]\")"));
    assert!(js.contains("queuePauseButtons"));
    assert!(js.contains("dataset.messageTarget"));
    assert!(js.contains("const BATCH_DELETE_TARGETS = ["));
    assert!(js.contains("async function openBatchDeleteModal"));
    assert!(js.contains("async function onBatchDeleteSubmit"));
    assert!(js.contains("function renderBatchDeleteModal"));
    assert!(js.contains("function renderStats()"));
    assert!(js.contains("/stats/overview"));
    assert!(js.contains("stats-refresh-button"));
    assert!(js.contains("/jobs/completed"));
    assert!(js.contains("batch-delete-button"));
    assert!(js.contains("const QUEUE_COLLAPSE_THRESHOLD = 10;"));
    assert!(js.contains("\u{AC01} \u{D30C}\u{C77C}\u{C774} ffmpeg \u{D050}\u{C5D0} \u{B4F1}\u{B85D}\u{B418}\u{C5C8}\u{C2B5}\u{B2C8}\u{B2E4}."));
    assert!(js.contains("setActiveTab(\"jobs\");"));
    assert!(js.contains("function onQueueBoardClick"));
    assert!(js.contains("function renderQueueBoard"));
    assert!(js.contains("function renderQueueControls"));
    assert!(js.contains("data-queue-toggle"));
    assert!(js.contains("function renderDictionary"));
    assert!(js.contains("function renderUserDictionaryChip"));
    assert!(js.contains("data-dictionary-action=\"promote-auto\""));
    assert!(js.contains("data-dictionary-action=\"delete-auto-all\""));
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

    assert!(css.contains(".tab-shell"));
    assert!(css.contains(".tab-button"));
    assert!(css.contains(".surface"));
    assert!(css.contains(".upload-grid"));
    assert!(css.contains(".jobs-layout"));
    assert!(css.contains(".job-stage-strip"));
    assert!(css.contains(".jobs-list"));
    assert!(css.contains(".queue-board"));
    assert!(css.contains(".section-actions"));
    assert!(css.contains(".danger-button"));
    assert!(css.contains(".batch-controls-stack"));
    assert!(css.contains(".batch-controls-danger"));
    assert!(css.contains(".settings-summary-card"));
    assert!(css.contains(".modal-dialog-large"));
    assert!(css.contains(".queue-column-toggle"));
    assert!(css.contains(".stats-shell"));
    assert!(css.contains(".stats-kpi-grid"));
    assert!(css.contains(".stats-stage-grid"));
    assert!(css.contains(".stats-stage-card"));
    assert!(css.contains(".dictionary-form"));
    assert!(css.contains(".dictionary-chip"));
    assert!(css.contains(".dictionary-group"));
    assert!(css.contains(".dictionary-promote-button"));
    assert!(css.contains(".dictionary-group-head-row"));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_new_root_returns_not_found() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _env_guard = EnvVarGuard::capture(crate::server::QUEUE_START_PAUSED_ENV_VAR);
    unsafe { std::env::set_var(crate::server::QUEUE_START_PAUSED_ENV_VAR, "1") };
    let app = router_with_repo_root(temp_workspace());
    let response = app
        .oneshot(get_request("/new"))
        .await
        .expect("new root response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
