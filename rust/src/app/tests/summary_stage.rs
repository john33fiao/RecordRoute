use super::super::*;
use super::support::*;
use crate::index::{IndexStore, JobRecord};
use crate::test_support::env_lock;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
#[test]
fn run_summary_processes_transcript_files_in_selected_job_dir() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        std::env::remove_var("RECORDROUTE_LLAMA_MODEL");
        std::env::set_var("HF_TOKEN", "summary-token");
    }

    let repo_root = temp_workspace();
    let selected_job_dir = repo_root.join("db/job-1");
    let ignored_job_dir = repo_root.join("db/job-2");
    let llama_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let llama_log = repo_root.join("llama-args.log");
    let embedding_log = repo_root.join("llama-embedding.log");
    let prompt_capture = repo_root.join("llama-prompt.txt");
    fs::create_dir_all(selected_job_dir.join("stt")).expect("selected stt dir");
    fs::create_dir_all(ignored_job_dir.join("stt")).expect("ignored stt dir");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&llama_bin).expect("llama bin");

    fs::write(
        selected_job_dir.join("stt/channel_01.txt"),
        "화자 A가 일정과 비용을 설명했다.",
    )
    .expect("channel 01");
    fs::write(
        selected_job_dir.join("stt/channel_02.txt"),
        "화자 B가 일정과 비용을 다시 확인했다.",
    )
    .expect("channel 02");
    fs::write(
        selected_job_dir.join("stt/mono_mix.txt"),
        "전체 대화에서 다음 주 방문과 견적 검토가 언급되었다.",
    )
    .expect("mono mix");
    fs::write(ignored_job_dir.join("stt/notes.md"), "ignore").expect("ignored");

    write_build_script(&build_script_path(&repo_root, "llama"));
    write_fake_llama_cli(
        &fake_command_path(&llama_bin, "llama-cli"),
        &llama_log,
        &prompt_capture,
        false,
    );
    write_fake_llama_embedding(
        &fake_command_path(&llama_bin, "llama-embedding"),
        &embedding_log,
    );

    let store = IndexStore::new(&repo_root);
    store
        .insert_job(JobRecord::new(
            "job-1".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/input.wav"),
            selected_job_dir.clone(),
        ))
        .expect("insert selected job");
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

    let summary =
        run_summary_with_repo_root(&repo_root, &mut reader, &mut output).expect("summary run");

    unsafe { std::env::remove_var("HF_TOKEN") };

    assert_eq!(summary.job_id, "job-1");
    assert_eq!(
        summary.summary_file,
        selected_job_dir.join("summary/result.md")
    );
    assert!(summary.summary_dir.is_dir());
    assert_eq!(
        fs::read_to_string(&summary.summary_file).expect("summary file"),
        "synthetic summary"
    );
    assert!(summary.summary_dir.join("embedding.json").is_file());
    assert!(!summary.summary_dir.join(".input.prompt.txt").exists());

    let prompt = fs::read_to_string(prompt_capture).expect("prompt capture");
    assert!(prompt.contains("[channel_01.txt]"));
    assert!(prompt.contains("[channel_02.txt]"));
    assert!(prompt.contains("[mono_mix.txt]"));
    assert!(prompt.contains("Markdown"));
    assert!(prompt.contains("## 요약"));
    assert!(prompt.contains("통화, 회의, 음성 메모"));
    assert!(prompt.contains("화자 A가 일정과 비용을 설명했다."));

    let llama_log = fs::read_to_string(llama_log).expect("llama log");
    assert!(llama_log.contains("--single-turn"));
    assert!(llama_log.contains("-hf"));
    assert!(llama_log.contains("ggml-org/gemma-3-4b-it-GGUF"));
    assert!(llama_log.contains("HF_TOKEN=summary-token"));

    let embedding_log = fs::read_to_string(embedding_log).expect("embedding log");
    assert!(embedding_log.contains("-hf"));
    assert!(embedding_log.contains("Qwen/Qwen3-Embedding-4B-GGUF"));
}

#[test]
fn run_summary_reuses_existing_summary_file_without_llama_toolchain() {
    let repo_root = temp_workspace();
    let selected_job_dir = repo_root.join("db/job-1");
    let existing_summary = selected_job_dir.join("summary/input.md");
    fs::create_dir_all(selected_job_dir.join("stt")).expect("selected stt dir");
    fs::create_dir_all(existing_summary.parent().expect("summary dir")).expect("summary dir");

    fs::write(
        selected_job_dir.join("stt/mono_mix.txt"),
        "현장 방문 일정을 논의했다.",
    )
    .expect("mono mix");
    fs::write(&existing_summary, "existing summary").expect("existing summary");

    let store = IndexStore::new(&repo_root);
    store
        .insert_job(JobRecord::new(
            "job-1".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/input.wav"),
            selected_job_dir.clone(),
        ))
        .expect("insert selected job");

    let mut reader = Cursor::new(b"1\n".to_vec());
    let mut output = Vec::new();

    let summary =
        run_summary_with_repo_root(&repo_root, &mut reader, &mut output).expect("summary run");

    assert_eq!(summary.job_id, "job-1");
    assert_eq!(summary.summary_dir, selected_job_dir.join("summary"));
    assert_eq!(
        summary.summary_file,
        selected_job_dir.join("summary/result.md")
    );
    assert!(!existing_summary.exists());
    assert_eq!(
        fs::read_to_string(&summary.summary_file).expect("summary file"),
        "existing summary"
    );
    assert!(!summary.summary_dir.join(".input.prompt.txt").exists());
}

#[test]
fn run_summary_reuses_existing_result_txt_without_llama_toolchain() {
    let repo_root = temp_workspace();
    let selected_job_dir = repo_root.join("db/job-1");
    let existing_summary = selected_job_dir.join("summary/result.txt");
    fs::create_dir_all(selected_job_dir.join("stt")).expect("selected stt dir");
    fs::create_dir_all(existing_summary.parent().expect("summary dir")).expect("summary dir");

    fs::write(
        selected_job_dir.join("stt/mono_mix.txt"),
        "현장 방문 일정을 논의했다.",
    )
    .expect("mono mix");
    fs::write(&existing_summary, "legacy txt summary").expect("existing summary");

    let store = IndexStore::new(&repo_root);
    store
        .insert_job(JobRecord::new(
            "job-1".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/input.wav"),
            selected_job_dir.clone(),
        ))
        .expect("insert selected job");

    let mut reader = Cursor::new(b"1\n".to_vec());
    let mut output = Vec::new();

    let summary =
        run_summary_with_repo_root(&repo_root, &mut reader, &mut output).expect("summary run");

    assert_eq!(summary.job_id, "job-1");
    assert_eq!(
        summary.summary_file,
        selected_job_dir.join("summary/result.md")
    );
    assert!(!existing_summary.exists());
    assert_eq!(
        fs::read_to_string(&summary.summary_file).expect("summary file"),
        "legacy txt summary"
    );
}

#[test]
fn run_summary_uses_local_model_path_when_file_exists() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let repo_root = temp_workspace();
    let selected_job_dir = repo_root.join("db/job-1");
    let llama_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let llama_log = repo_root.join("llama-local.log");
    let embedding_log = repo_root.join("llama-local-embedding.log");
    let prompt_capture = repo_root.join("llama-local-prompt.txt");
    let model_path = repo_root.join("models/llama/custom.gguf");
    fs::create_dir_all(selected_job_dir.join("stt")).expect("selected stt dir");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&llama_bin).expect("llama bin");
    fs::create_dir_all(model_path.parent().expect("parent")).expect("models dir");

    fs::write(
        selected_job_dir.join("stt/mono_mix.txt"),
        "현장 방문 일정을 논의했다.",
    )
    .expect("mono mix");
    fs::write(&model_path, "model").expect("model");

    write_build_script(&build_script_path(&repo_root, "llama"));
    write_fake_llama_cli(
        &fake_command_path(&llama_bin, "llama-cli"),
        &llama_log,
        &prompt_capture,
        false,
    );
    write_fake_llama_embedding(
        &fake_command_path(&llama_bin, "llama-embedding"),
        &embedding_log,
    );

    unsafe { std::env::set_var("RECORDROUTE_LLAMA_MODEL", "models/llama/custom.gguf") };

    let store = IndexStore::new(&repo_root);
    store
        .insert_job(JobRecord::new(
            "job-1".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/input.wav"),
            selected_job_dir,
        ))
        .expect("insert selected job");

    let mut reader = Cursor::new(b"1\n".to_vec());
    let mut output = Vec::new();

    let summary =
        run_summary_with_repo_root(&repo_root, &mut reader, &mut output).expect("summary run");

    unsafe { std::env::remove_var("RECORDROUTE_LLAMA_MODEL") };

    assert!(summary.summary_file.is_file());
    assert!(summary.summary_dir.join("embedding.json").is_file());
    let llama_log = fs::read_to_string(llama_log).expect("llama log");
    assert!(llama_log.contains("-m"));
    assert!(llama_log.contains(model_path.to_string_lossy().as_ref()));
    assert!(!llama_log.contains("-hf"));

    let embedding_log = fs::read_to_string(embedding_log).expect("embedding log");
    assert!(embedding_log.contains("-hf"));
}
