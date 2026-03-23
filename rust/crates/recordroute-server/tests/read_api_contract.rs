use std::fs;
use std::path::{Path, PathBuf};

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use recordroute_core::config::AppConfig;
use recordroute_server::{app, AppState, TaskProgressSnapshot};
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::util::ServiceExt;

#[tokio::test]
async fn history_contract_matches_fixture() {
    let test_app = TestApp::new();
    let fixture = load_http_fixture("get_history_list.json");

    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/history")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), status_from_fixture(&fixture));
    let body = to_json_body(response).await;
    assert_eq!(body, fixture["expected_body"]);
}

#[tokio::test]
async fn tasks_contract_matches_fixture() {
    let test_app = TestApp::new();
    let fixture = load_http_fixture("get_tasks_memory_registry.json");

    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/tasks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), status_from_fixture(&fixture));
    let body = to_json_body(response).await;
    assert_eq!(body, fixture["expected_body"]);
}

#[tokio::test]
async fn progress_contract_matches_fixture() {
    let test_app = TestApp::new();
    let fixture = load_http_fixture("get_progress_task.json");

    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(fixture["path"].as_str().unwrap())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), status_from_fixture(&fixture));
    let body = to_json_body(response).await;
    assert_eq!(body, fixture["expected_body"]);
}

#[tokio::test]
async fn segments_contract_matches_fixture() {
    let test_app = TestApp::new();
    let fixture = load_http_fixture("get_segments_sidecar.json");

    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(fixture["path"].as_str().unwrap())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), status_from_fixture(&fixture));
    let body = to_json_body(response).await;
    assert_eq!(body, fixture["expected_body"]);
}

#[tokio::test]
async fn download_contract_matches_fixture() {
    let test_app = TestApp::new();
    let fixture = load_http_fixture("get_download_file.json");

    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(fixture["path"].as_str().unwrap())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), status_from_fixture(&fixture));
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        fixture["expected_body"]["content_type"].as_str().unwrap()
    );
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_DISPOSITION)
            .unwrap()
            .to_str()
            .unwrap(),
        fixture["expected_body"]["content_disposition"]
            .as_str()
            .unwrap()
    );

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(bytes.as_ref(), b"fake-mp3");
}

#[tokio::test]
async fn missing_segments_sidecar_returns_empty_array() {
    let test_app = TestApp::new();

    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/segments/DB/uploads/__UPLOAD_ID__/missing-sidecar.m4a")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(to_json_body(response).await, Value::Array(Vec::new()));
}

#[tokio::test]
async fn progress_not_found_returns_404_error_envelope() {
    let test_app = TestApp::new();

    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/progress/missing-task")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = to_json_body(response).await;
    assert_eq!(body["success"], Value::Bool(false));
    assert_eq!(
        body["error"]["code"],
        Value::String("task_not_found".to_string())
    );
}

#[tokio::test]
async fn empty_task_registry_returns_empty_array() {
    let temp_dir = TempDir::new().unwrap();
    let db_root = temp_dir.path().join("DB");
    let model_root = temp_dir.path().join("models");
    fs::create_dir_all(&db_root).unwrap();
    fs::create_dir_all(&model_root).unwrap();

    let app = app(AppState::from_config(AppConfig::from_env_iter([
        ("DB_FOLDER_PATH", db_root.to_string_lossy().to_string()),
        ("MODEL_ROOT_PATH", model_root.to_string_lossy().to_string()),
    ])));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/tasks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(to_json_body(response).await, Value::Array(Vec::new()));
}

struct TestApp {
    _temp_dir: TempDir,
    app: axum::Router,
}

impl TestApp {
    fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let db_root = temp_dir.path().join("DB");
        let model_root = temp_dir.path().join("models");
        fs::create_dir_all(db_root.join("uploads").join("__UPLOAD_ID__")).unwrap();
        fs::create_dir_all(&model_root).unwrap();

        let history_path = db_root.join("upload_history.json");
        let registry_path = db_root.join("file_registry.json");
        let audio_path = db_root
            .join("uploads")
            .join("__UPLOAD_ID__")
            .join("meeting.m4a");
        let missing_sidecar_audio = db_root
            .join("uploads")
            .join("__UPLOAD_ID__")
            .join("missing-sidecar.m4a");
        let download_path = db_root
            .join("uploads")
            .join("__UPLOAD_ID__")
            .join("meeting.mp3");

        fs::write(&audio_path, b"fake-audio").unwrap();
        fs::write(&missing_sidecar_audio, b"fake-audio").unwrap();
        fs::write(&download_path, b"fake-mp3").unwrap();
        fs::write(
            audio_path.with_extension("segments.json"),
            serde_json::to_vec_pretty(&json!({
                "segments": [
                    { "start": 0.0, "end": 2.4, "text": "안녕하세요", "speaker": "SPEAKER_00" },
                    { "start": 2.4, "end": 5.0, "text": "회의를 시작하겠습니다.", "speaker": "SPEAKER_01" }
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        fs::write(
            &history_path,
            serde_json::to_vec_pretty(&json!([
                {
                    "id": "__RECORD_ID__",
                    "timestamp": "__TIMESTAMP__",
                    "filename": "meeting.m4a",
                    "title_summary": "회의 요약",
                    "file_type": "audio",
                    "duration": "00:05:00",
                    "file_hash": "sha256:meeting",
                    "file_path": "DB/uploads/__UPLOAD_ID__/meeting.m4a",
                    "completed_tasks": {
                        "stt": true,
                        "embedding": false,
                        "summary": true
                    },
                    "download_links": {
                        "stt": "/download/__UUID_OR_PATH__"
                    },
                    "info": {
                        "source": "fixture"
                    },
                    "deleted": false,
                    "deleted_at": null,
                    "deleted_assets": {}
                }
            ]))
            .unwrap(),
        )
        .unwrap();

        fs::write(
            &registry_path,
            serde_json::to_vec_pretty(&json!({
                "__FILE_ID__": {
                    "file_uuid": "__FILE_ID__",
                    "file_path": "DB/uploads/__UPLOAD_ID__/meeting.m4a",
                    "record_id": "__RECORD_ID__",
                    "task_type": "stt",
                    "original_filename": "meeting.m4a"
                },
                "__UUID_OR_PATH__": {
                    "file_uuid": "__UUID_OR_PATH__",
                    "file_path": "DB/uploads/__UPLOAD_ID__/meeting.mp3",
                    "record_id": "__RECORD_ID__",
                    "task_type": "stt",
                    "original_filename": "meeting.mp3"
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let config = AppConfig::from_env_iter([
            ("DB_FOLDER_PATH", db_root.to_string_lossy().to_string()),
            ("MODEL_ROOT_PATH", model_root.to_string_lossy().to_string()),
        ]);
        let app = app(AppState::with_task_snapshots(
            config,
            [TaskProgressSnapshot {
                task_id: "__TASK_ID__".to_string(),
                message: "Summarizing transcript".to_string(),
                stage: Some("summary".to_string()),
                progress_percent: Some(66),
                eta_seconds: Some(12),
                error_code: None,
                retryable: None,
                failed_step: None,
                error: None,
            }],
        ));

        Self {
            _temp_dir: temp_dir,
            app,
        }
    }
}

fn load_http_fixture(name: &str) -> Value {
    let fixture_path = fixture_root().join("http").join(name);
    serde_json::from_slice(&fs::read(fixture_path).unwrap()).unwrap()
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures")
        .join("contracts")
}

fn status_from_fixture(fixture: &Value) -> StatusCode {
    StatusCode::from_u16(fixture["expected_status"].as_u64().unwrap() as u16).unwrap()
}

async fn to_json_body(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}
