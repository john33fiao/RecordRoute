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

    assert!(html.contains("RecordRoute Web Console"));
    assert!(html.contains("id=\"settings-open-button\""));
    assert!(html.contains("data-tab=\"upload\""));
    assert!(html.contains("data-tab=\"jobs\""));
    assert!(html.contains("data-tab-panel=\"queue\""));
    assert!(html.contains("id=\"settings-modal\""));
    assert!(html.contains("id=\"upload-panel\""));
    assert!(html.contains("id=\"batch-panel\""));
    assert!(html.contains("id=\"jobs-panel\""));
    assert!(html.contains("id=\"selected-job-meta\""));
    assert!(html.contains("id=\"search-panel\""));
    assert!(html.contains("id=\"queue-panel\""));
    assert!(html.contains("id=\"dictionary-panel\""));
    assert!(html.contains("id=\"system-server-status\""));
    assert!(html.contains("id=\"dictionary-form\""));
    assert!(html.contains("id=\"dictionary-list\""));
    assert!(html.contains("id=\"dictionary-refresh-button\""));
    assert!(html.contains("id=\"queue-pause-button\""));
    assert!(html.contains("id=\"queue-cancel-button\""));
    assert!(html.contains("id=\"batch-delete-target\""));
    assert!(html.contains("id=\"batch-delete-button\""));
    assert!(html.contains("id=\"batch-delete-modal\""));
    assert!(html.contains("id=\"batch-delete-confirm-button\""));
    assert!(html.contains(".qta,audio/*"));
    assert!(html.contains("성을 회의록으로, STT·요약·임베딩·RAG 질의 지원까지 일원화."));
    assert!(html.contains(
        "오디오-텍스트 변환 정확도를 높이기 위해, 주로 사용되는 키워드를 관리할 수 있습니다."
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
        "const TAB_KEYS = [\"upload\", \"jobs\", \"search\", \"queue\", \"dictionary\"];"
    ));
    assert!(js.contains("async function refreshJobs"));
    assert!(js.contains("async function refreshQueue"));
    assert!(js.contains("function renderTabs()"));
    assert!(js.contains("function openSettingsModal()"));
    assert!(js.contains("function renderSettingsModal()"));
    assert!(js.contains("async function onQueuePauseToggle"));
    assert!(js.contains("async function onQueueCancelPending"));
    assert!(js.contains("async function refreshDictionary"));
    assert!(js.contains("window.addEventListener(\"hashchange\", onHashChange);"));
    assert!(js.contains("pendingDictionaryRefreshJobId"));
    assert!(js.contains("queue-pause-button"));
    assert!(js.contains("queue-cancel-button"));
    assert!(js.contains("const BATCH_DELETE_TARGETS = ["));
    assert!(js.contains("async function openBatchDeleteModal"));
    assert!(js.contains("async function onBatchDeleteSubmit"));
    assert!(js.contains("function renderBatchDeleteModal"));
    assert!(js.contains("/jobs/completed"));
    assert!(js.contains("batch-delete-button"));
    assert!(js.contains("const QUEUE_COLLAPSE_THRESHOLD = 10;"));
    assert!(js.contains("각 파일이 ffmpeg 큐에 등록되었습니다."));
    assert!(js.contains("setActiveTab(\"jobs\");"));
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
    assert!(css.contains(".dictionary-form"));
    assert!(css.contains(".dictionary-chip"));
    assert!(css.contains(".dictionary-group"));
    assert!(css.contains(".dictionary-promote-button"));
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
