use super::super::*;
use super::support::*;
use crate::index::{IndexStore, JobOutputs, JobRecord};
use crate::test_support::env_lock;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
#[test]
fn run_stt_processes_audio_files_in_selected_job_dir() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let repo_root = temp_workspace();
    let selected_job_dir = repo_root.join("db/job-1");
    let ignored_job_dir = repo_root.join("db/job-2");
    let whisper_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let whisper_log = repo_root.join("whisper-args.log");
    let source_dir = repo_root.join("whisper-source");
    fs::create_dir_all(&selected_job_dir).expect("selected job dir");
    fs::create_dir_all(&ignored_job_dir).expect("ignored job dir");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&source_dir).expect("source dir");
    fs::create_dir_all(&whisper_bin).expect("whisper bin");

    write_test_wav(&selected_job_dir.join("channel_01.wav"), 1);
    write_test_wav(&selected_job_dir.join("mono_mix.wav"), 1);
    fs::write(selected_job_dir.join("notes.txt"), "ignore").expect("notes");
    fs::write(ignored_job_dir.join("not-audio.txt"), "ignore").expect("ignored");

    write_build_script(&build_script_path(&repo_root, "whisper"));
    write_fake_whisper_cli(
        &fake_command_path(&whisper_bin, "whisper-cli"),
        &whisper_log,
        false,
    );
    fs::write(source_dir.join("ggml-base.bin"), "model").expect("source model");
    unsafe { std::env::set_var("RECORDROUTE_WHISPER_MODEL_SOURCE_DIR", &source_dir) };

    let store = IndexStore::new(&repo_root);
    let mut selected_job = JobRecord::new(
        "job-1".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
        PathBuf::from("/tmp/input.wav"),
        selected_job_dir.clone(),
    );
    selected_job
        .mark_completed("2026-01-01T00:00:01Z".to_string(), JobOutputs::default())
        .expect("mark selected completed");
    store.insert_job(selected_job).expect("insert selected job");
    store
        .insert_job(JobRecord::new(
            "job-2".to_string(),
            "2026-01-01T00:00:01Z".to_string(),
            PathBuf::from("/tmp/other.wav"),
            ignored_job_dir,
        ))
        .expect("insert ignored job");

    let mut reader = Cursor::new(b"1\n".to_vec());
    let mut output = Vec::new();

    let summary = run_stt_with_repo_root(&repo_root, &mut reader, &mut output).expect("stt run");

    unsafe { std::env::remove_var("RECORDROUTE_WHISPER_MODEL_SOURCE_DIR") };

    assert_eq!(summary.job_id, "job-1");
    assert_eq!(summary.transcripts.len(), 2);
    assert!(summary.stt_dir.is_dir());
    assert!(summary.stt_dir.join("channel_01.txt").is_file());
    assert!(summary.stt_dir.join("mono_mix.txt").is_file());
    assert!(repo_root.join("models/whisper/ggml-base.bin").is_file());

    let whisper_log = fs::read_to_string(whisper_log).expect("whisper log");
    assert!(whisper_log.contains("-l"));
    assert!(whisper_log.contains("auto"));
    assert!(whisper_log.contains("channel_01.wav"));
    assert!(whisper_log.contains("mono_mix.wav"));
}

#[test]
fn run_stt_retries_invalid_folder_selection() {
    let repo_root = temp_workspace();
    let selected_job_dir = repo_root.join("db/job-1");
    let whisper_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    fs::create_dir_all(&selected_job_dir).expect("selected job dir");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(repo_root.join("models/whisper")).expect("models dir");
    fs::create_dir_all(&whisper_bin).expect("whisper bin");

    write_test_wav(&selected_job_dir.join("channel_01.wav"), 1);
    write_build_script(&build_script_path(&repo_root, "whisper"));
    write_fake_whisper_cli(
        &fake_command_path(&whisper_bin, "whisper-cli"),
        &repo_root.join("whisper.log"),
        false,
    );
    fs::write(repo_root.join("models/whisper/ggml-base.bin"), "model").expect("model");

    let store = IndexStore::new(&repo_root);
    let mut selected_job = JobRecord::new(
        "job-1".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
        PathBuf::from("/tmp/input.wav"),
        selected_job_dir,
    );
    selected_job
        .mark_completed("2026-01-01T00:00:01Z".to_string(), JobOutputs::default())
        .expect("mark selected completed");
    store.insert_job(selected_job).expect("insert selected job");

    let mut reader = Cursor::new(b"9\n1\n".to_vec());
    let mut output = Vec::new();

    let summary = run_stt_with_repo_root(&repo_root, &mut reader, &mut output).expect("stt run");

    assert_eq!(summary.job_id, "job-1");
    assert!(
        String::from_utf8(output)
            .expect("utf8")
            .contains("Invalid selection. Enter a number between 1 and 1.")
    );
}
