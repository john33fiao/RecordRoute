use super::super::router_with_repo_root;
use super::super::types::{
    ErrorResponse, ModelPrepareResponse, ModelStatusResponse, SystemStatusResponse,
};
use super::support::*;
use crate::index::{IndexStore, ModelKind, ModelPreparationStatus};
use crate::test_support::env_lock;
use axum::http::StatusCode;
use std::fs;
use tower::util::ServiceExt;
#[tokio::test(flavor = "multi_thread")]
async fn get_system_status_reports_available_toolchains_and_model_readiness() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        std::env::remove_var(crate::llama::MODEL_ENV_VAR);
        std::env::remove_var(crate::llama::EMBEDDING_MODEL_ENV_VAR);
    }

    let repo_root = temp_workspace();
    let scripts_dir = repo_root.join("scripts");
    let ffmpeg_bin = repo_root
        .join(".build/ffmpeg")
        .join(crate::ffmpeg::target_dir_name())
        .join("install/bin");
    let whisper_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let llama_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");

    fs::create_dir_all(&scripts_dir).expect("scripts dir");
    fs::create_dir_all(&ffmpeg_bin).expect("ffmpeg bin");
    fs::create_dir_all(&whisper_bin).expect("whisper bin");
    fs::create_dir_all(&llama_bin).expect("llama bin");
    fs::create_dir_all(repo_root.join("models/whisper")).expect("whisper model dir");
    fs::create_dir_all(repo_root.join("models/llama/hf")).expect("llama model dir");

    write_build_script(&build_script_path(&repo_root, "ffmpeg"));
    write_build_script(&build_script_path(&repo_root, "whisper"));
    write_build_script(&build_script_path(&repo_root, "llama"));
    write_simple_command(&fake_command_path(&ffmpeg_bin, "ffmpeg"));
    write_simple_command(&fake_command_path(&ffmpeg_bin, "ffprobe"));
    write_simple_command(&fake_command_path(&whisper_bin, "whisper-cli"));
    write_simple_command(&fake_command_path(&llama_bin, "llama-cli"));
    write_simple_command(&fake_command_path(&llama_bin, "llama-embedding"));

    fs::write(repo_root.join("models/whisper/ggml-base.bin"), "model").expect("whisper model");
    fs::write(
        repo_root.join("models/llama/hf/ggml-org__gemma-3-4b-it-GGUF.gguf"),
        "cached llama model",
    )
    .expect("llama model");
    fs::write(
        repo_root.join("models/llama/hf/Qwen__Qwen3-Embedding-4B-GGUF.gguf"),
        "cached llama embedding model",
    )
    .expect("llama embedding model");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/system/status"))
        .await
        .expect("system status response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: SystemStatusResponse = read_json(response).await;
    assert!(body.ffmpeg_available);
    assert!(body.whisper_available);
    assert!(body.llama_available);
    assert!(body.whisper_model_ready);
    assert!(body.llama_model_ready);
    assert!(body.llama_embedding_model_ready);
    assert!(body.errors.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn get_system_status_reports_missing_toolchains_and_models_in_errors() {
    let repo_root = temp_workspace();
    let scripts_dir = repo_root.join("scripts");
    let ffmpeg_bin = repo_root
        .join(".build/ffmpeg")
        .join(crate::ffmpeg::target_dir_name())
        .join("install/bin");

    fs::create_dir_all(&scripts_dir).expect("scripts dir");
    fs::create_dir_all(&ffmpeg_bin).expect("ffmpeg bin");

    write_build_script(&build_script_path(&repo_root, "ffmpeg"));
    write_simple_command(&fake_command_path(&ffmpeg_bin, "ffmpeg"));
    write_simple_command(&fake_command_path(&ffmpeg_bin, "ffprobe"));

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/system/status"))
        .await
        .expect("system status response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: SystemStatusResponse = read_json(response).await;
    assert!(body.ffmpeg_available);
    assert!(!body.whisper_available);
    assert!(!body.llama_available);
    assert!(!body.whisper_model_ready);
    assert!(!body.llama_model_ready);
    assert!(!body.errors.is_empty());
    assert!(body.errors.iter().all(|message| message.contains("setup")));
    assert!(
        body.errors
            .iter()
            .all(|message| !message.contains("build_whisper"))
    );
    assert!(
        body.errors
            .iter()
            .all(|message| !message.contains("build_llama"))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn post_prepare_whisper_returns_ok_when_model_is_already_ready() {
    let repo_root = temp_workspace();
    let whisper_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(repo_root.join("modules/whisper.cpp/models")).expect("download dir");
    fs::create_dir_all(repo_root.join("models/whisper")).expect("model dir");
    fs::create_dir_all(&whisper_bin).expect("whisper bin");

    write_build_script(&build_script_path(&repo_root, "whisper"));
    write_simple_command(&fake_command_path(&whisper_bin, "whisper-cli"));
    fs::write(repo_root.join("models/whisper/ggml-base.bin"), "model").expect("model");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(post_empty_request("/models/whisper/prepare"))
        .await
        .expect("prepare response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: ModelPrepareResponse = read_json(response).await;
    assert_eq!(body.model, ModelKind::Whisper);
    assert!(body.ready);
    assert!(body.already_ready);
    assert!(!body.deduplicated);
    assert_eq!(body.preparation.status, ModelPreparationStatus::Completed);
}

#[tokio::test(flavor = "multi_thread")]
async fn post_prepare_whisper_runs_background_download_and_updates_model_status() {
    let repo_root = temp_workspace();
    let whisper_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(repo_root.join("modules/whisper.cpp/models")).expect("download dir");
    fs::create_dir_all(&whisper_bin).expect("whisper bin");

    write_build_script(&build_script_path(&repo_root, "whisper"));
    write_simple_command(&fake_command_path(&whisper_bin, "whisper-cli"));
    write_fake_download_script(
        &whisper_download_script_path(&repo_root),
        &repo_root.join("download.log"),
    );

    let app = router_with_repo_root(repo_root.clone());
    let response = app
        .clone()
        .oneshot(post_empty_request("/models/whisper/prepare"))
        .await
        .expect("prepare response");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body: ModelPrepareResponse = read_json(response).await;
    assert_eq!(body.model, ModelKind::Whisper);
    assert!(!body.ready);
    assert!(!body.already_ready);
    assert!(!body.deduplicated);
    assert_eq!(body.preparation.status, ModelPreparationStatus::Running);

    let status = wait_for_model_preparation_state(&app, ModelKind::Whisper).await;
    assert!(status.available);
    assert!(status.ready);
    assert_eq!(status.preparation.status, ModelPreparationStatus::Completed);
    assert!(repo_root.join("models/whisper/ggml-base.bin").is_file());
}

#[tokio::test(flavor = "multi_thread")]
async fn post_prepare_llama_runs_background_download_and_updates_model_status() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        std::env::remove_var(crate::llama::MODEL_ENV_VAR);
        std::env::remove_var(crate::llama::EMBEDDING_MODEL_ENV_VAR);
    }

    let repo_root = temp_workspace();
    let llama_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&llama_bin).expect("llama bin");

    write_build_script(&build_script_path(&repo_root, "llama"));
    write_fake_llama_download_command(
        &fake_command_path(&llama_bin, "llama-cli"),
        &repo_root.join("llama-download.log"),
    );
    write_simple_command(&fake_command_path(&llama_bin, "llama-embedding"));

    let app = router_with_repo_root(repo_root.clone());
    let response = app
        .clone()
        .oneshot(post_empty_request("/models/llama/prepare"))
        .await
        .expect("prepare response");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body: ModelPrepareResponse = read_json(response).await;
    assert_eq!(body.model, ModelKind::Llama);
    assert!(!body.ready);
    assert!(!body.already_ready);
    assert!(!body.deduplicated);
    assert_eq!(body.preparation.status, ModelPreparationStatus::Running);

    let status = wait_for_model_preparation_state(&app, ModelKind::Llama).await;
    assert!(status.available);
    assert!(status.ready);
    assert!(status.embedding_ready);
    assert_eq!(status.preparation.status, ModelPreparationStatus::Completed);

    let log = fs::read_to_string(repo_root.join("llama-download.log")).expect("llama download log");
    assert!(log.contains("-hf"));
    assert!(log.contains("LLAMA_CACHE="));
}
#[tokio::test(flavor = "multi_thread")]
async fn post_prepare_llama_deduplicates_running_preparation() {
    let repo_root = temp_workspace();
    let llama_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&llama_bin).expect("llama bin");

    write_build_script(&build_script_path(&repo_root, "llama"));
    write_simple_command(&fake_command_path(&llama_bin, "llama-cli"));
    write_simple_command(&fake_command_path(&llama_bin, "llama-embedding"));
    IndexStore::new(&repo_root)
        .update_model_preparation(ModelKind::Llama, |record| {
            record.mark_running(crate::app::now_rfc3339().expect("timestamp"));
        })
        .expect("seed running preparation");
    IndexStore::new(&repo_root)
        .update_llama_embedding_preparation(|record| {
            record.mark_running(crate::app::now_rfc3339().expect("timestamp"));
        })
        .expect("seed running embedding preparation");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(post_empty_request("/models/llama/prepare"))
        .await
        .expect("prepare response");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body: ModelPrepareResponse = read_json(response).await;
    assert_eq!(body.model, ModelKind::Llama);
    assert!(!body.ready);
    assert!(!body.already_ready);
    assert!(body.deduplicated);
    assert_eq!(body.preparation.status, ModelPreparationStatus::Running);
}

#[tokio::test(flavor = "multi_thread")]
async fn post_prepare_whisper_returns_503_for_invalid_configuration() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe { std::env::set_var(crate::whisper::MODEL_ENV_VAR, "models/whisper/custom.bin") };

    let repo_root = temp_workspace();
    let whisper_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(repo_root.join("modules/whisper.cpp/models")).expect("download dir");
    fs::create_dir_all(&whisper_bin).expect("whisper bin");
    write_build_script(&build_script_path(&repo_root, "whisper"));
    write_simple_command(&fake_command_path(&whisper_bin, "whisper-cli"));

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(post_empty_request("/models/whisper/prepare"))
        .await
        .expect("prepare response");

    unsafe { std::env::remove_var(crate::whisper::MODEL_ENV_VAR) };

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: ErrorResponse = read_json(response).await;
    assert!(body.message.contains("setup"));
    assert!(!body.message.contains("models/whisper/custom.bin"));
}

#[tokio::test(flavor = "multi_thread")]
async fn post_prepare_llama_marks_failed_status_when_download_fails() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        std::env::remove_var(crate::llama::MODEL_ENV_VAR);
        std::env::remove_var(crate::llama::EMBEDDING_MODEL_ENV_VAR);
    }

    let repo_root = temp_workspace();
    let llama_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&llama_bin).expect("llama bin");
    write_build_script(&build_script_path(&repo_root, "llama"));
    write_failing_llama_cli(&fake_command_path(&llama_bin, "llama-cli"));
    write_simple_command(&fake_command_path(&llama_bin, "llama-embedding"));

    let app = router_with_repo_root(repo_root);
    let response = app
        .clone()
        .oneshot(post_empty_request("/models/llama/prepare"))
        .await
        .expect("prepare response");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body: ModelPrepareResponse = read_json(response).await;
    assert_eq!(body.model, ModelKind::Llama);
    assert_eq!(body.preparation.status, ModelPreparationStatus::Running);

    let status = wait_for_model_preparation_state(&app, ModelKind::Llama).await;
    assert!(status.available);
    assert!(!status.ready);
    assert_eq!(status.preparation.status, ModelPreparationStatus::Failed);
    assert_eq!(
        status.error.as_deref(),
        Some("환경 준비가 필요합니다. setup을 다시 실행하세요."),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn get_models_status_sanitizes_setup_errors() {
    let repo_root = temp_workspace();
    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/models/status"))
        .await
        .expect("models status response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: ModelStatusResponse = read_json(response).await;
    assert_eq!(
        body.whisper.error.as_deref(),
        Some("환경 준비가 필요합니다. setup을 다시 실행하세요."),
    );
    assert_eq!(
        body.llama.error.as_deref(),
        Some("환경 준비가 필요합니다. setup을 다시 실행하세요."),
    );
    assert_eq!(
        body.llama.embedding_error.as_deref(),
        Some("환경 준비가 필요합니다. setup을 다시 실행하세요."),
    );
}
