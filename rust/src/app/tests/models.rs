use super::super::*;
use super::support::*;
use crate::index::{IndexStore, ModelKind, ModelPreparationStatus};
use crate::test_support::env_lock;
use std::fs;
use std::thread;
#[test]
fn prepare_llama_model_downloads_default_hugging_face_repo() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        std::env::remove_var("RECORDROUTE_LLAMA_MODEL");
        std::env::set_var("HF_TOKEN", "prepare-token");
    }

    let repo_root = temp_workspace();
    let llama_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let llama_log = repo_root.join("llama-prepare.log");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&llama_bin).expect("llama bin");

    write_build_script(&build_script_path(&repo_root, "llama"));
    write_fake_llama_cli(
        &fake_command_path(&llama_bin, "llama-cli"),
        &llama_log,
        &repo_root.join("unused-prompt.txt"),
        false,
    );
    write_platform_script(
        &fake_command_path(&llama_bin, "llama-embedding"),
        "#!/bin/sh\nexit 0\n",
        "@echo off\nexit /b 0\n",
    );

    prepare_llama_model_with_repo_root(&repo_root).expect("prepare llama model");

    unsafe {
        std::env::remove_var("RECORDROUTE_LLAMA_MODEL");
        std::env::remove_var("HF_TOKEN");
    }

    let cached_model = repo_root
        .join("models/llama/hf")
        .join("ggml-org__gemma-3-4b-it-GGUF.gguf");
    let cached_embedding_model = repo_root
        .join("models/llama/hf")
        .join("Qwen__Qwen3-Embedding-4B-GGUF.gguf");
    assert!(cached_model.is_file());
    assert!(cached_embedding_model.is_file());

    let llama_log = fs::read_to_string(llama_log).expect("llama log");
    assert!(llama_log.contains("-hf"));
    assert!(llama_log.contains("ggml-org/gemma-3-4b-it-GGUF"));
    assert!(llama_log.contains("/exit"));
    assert!(llama_log.contains("HF_TOKEN=prepare-token"));
    assert!(llama_log.contains("LLAMA_CACHE="));
}

#[test]
fn submit_model_preparation_replaces_stale_running_record() {
    let repo_root = temp_workspace();
    let whisper_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let stale_heartbeat = "2020-01-01T00:00:00Z".to_string();
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(repo_root.join("modules/whisper.cpp/models")).expect("download dir");
    fs::create_dir_all(&whisper_bin).expect("whisper bin");

    write_build_script(&build_script_path(&repo_root, "whisper"));
    write_fake_whisper_cli(
        &fake_command_path(&whisper_bin, "whisper-cli"),
        &repo_root.join("whisper.log"),
        false,
    );
    write_fake_download_script(
        &whisper_download_script_path(&repo_root),
        &repo_root.join("download.log"),
    );

    let store = IndexStore::new(&repo_root);
    store
        .update_model_preparation(ModelKind::Whisper, |record| {
            record.mark_running(stale_heartbeat.clone());
        })
        .expect("seed running preparation");

    let submission =
        submit_model_preparation(&repo_root, ModelKind::Whisper).expect("submit model preparation");

    assert!(submission.should_execute());
    assert_eq!(
        submission.preparation.status,
        ModelPreparationStatus::Running
    );
    assert_ne!(
        submission.preparation.started_at.as_deref(),
        Some(stale_heartbeat.as_str())
    );
}

#[test]
fn ensure_model_prepared_waits_for_running_preparation_without_duplicate_download() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let repo_root = temp_workspace();
    let whisper_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(repo_root.join("models/whisper")).expect("models dir");
    fs::create_dir_all(&whisper_bin).expect("whisper bin");

    write_build_script(&build_script_path(&repo_root, "whisper"));
    write_fake_whisper_cli(
        &fake_command_path(&whisper_bin, "whisper-cli"),
        &repo_root.join("whisper.log"),
        false,
    );
    let running_started_at = now_rfc3339().expect("running timestamp");
    IndexStore::new(&repo_root)
        .update_model_preparation(ModelKind::Whisper, |record| {
            record.mark_running(running_started_at.clone());
        })
        .expect("seed running preparation");

    let waiting_repo_root = repo_root.clone();
    let waiter = thread::spawn(move || {
        ensure_model_prepared(&waiting_repo_root, ModelKind::Whisper)
            .expect("waiting model preparation")
    });

    thread::sleep(std::time::Duration::from_millis(100));

    let during_wait = IndexStore::new(&repo_root)
        .model_preparation(ModelKind::Whisper)
        .expect("running preparation");
    assert_eq!(during_wait.status, ModelPreparationStatus::Running);
    assert_eq!(
        during_wait.started_at.as_deref(),
        Some(running_started_at.as_str())
    );

    fs::write(repo_root.join("models/whisper/ggml-base.bin"), "model").expect("model");
    IndexStore::new(&repo_root)
        .update_model_preparation(ModelKind::Whisper, |record| {
            record.mark_completed("2026-01-01T00:00:01Z".to_string());
        })
        .expect("complete preparation");

    let completed = waiter.join().expect("waiter join");

    assert!(repo_root.join("models/whisper/ggml-base.bin").is_file());
    assert_eq!(completed.status, ModelPreparationStatus::Completed);
    assert_eq!(
        completed.started_at.as_deref(),
        Some(running_started_at.as_str())
    );
}

#[test]
fn prepare_models_with_repo_root_prepares_whisper_and_llama_models() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        std::env::remove_var("RECORDROUTE_LLAMA_MODEL");
        std::env::remove_var("RECORDROUTE_LLAMA_EMBEDDING_MODEL");
        std::env::remove_var("RECORDROUTE_WHISPER_MODEL_SOURCE_DIR");
    }

    let repo_root = temp_workspace();
    let whisper_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let llama_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let whisper_source_dir = repo_root.join("whisper-source");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&whisper_source_dir).expect("source dir");
    fs::create_dir_all(&whisper_bin).expect("whisper bin");
    fs::create_dir_all(&llama_bin).expect("llama bin");
    fs::write(whisper_source_dir.join("ggml-base.bin"), "model").expect("source model");

    write_build_script(&build_script_path(&repo_root, "whisper"));
    write_build_script(&build_script_path(&repo_root, "llama"));
    write_fake_whisper_cli(
        &fake_command_path(&whisper_bin, "whisper-cli"),
        &repo_root.join("whisper.log"),
        false,
    );
    write_fake_llama_cli(
        &fake_command_path(&llama_bin, "llama-cli"),
        &repo_root.join("llama.log"),
        &repo_root.join("unused-prompt.txt"),
        false,
    );
    write_platform_script(
        &fake_command_path(&llama_bin, "llama-embedding"),
        "#!/bin/sh\nexit 0\n",
        "@echo off\nexit /b 0\n",
    );
    unsafe { std::env::set_var("RECORDROUTE_WHISPER_MODEL_SOURCE_DIR", &whisper_source_dir) };

    prepare_models_with_repo_root(&repo_root).expect("prepare models");

    unsafe { std::env::remove_var("RECORDROUTE_WHISPER_MODEL_SOURCE_DIR") };

    assert!(repo_root.join("models/whisper/ggml-base.bin").is_file());
    assert!(
        repo_root
            .join("models/llama/hf/ggml-org__gemma-3-4b-it-GGUF.gguf")
            .is_file()
    );
    assert!(
        repo_root
            .join("models/llama/hf/Qwen__Qwen3-Embedding-4B-GGUF.gguf")
            .is_file()
    );
}
