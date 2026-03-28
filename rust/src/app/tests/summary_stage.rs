use super::super::*;
use super::support::*;
use crate::index::{IndexStore, QueuePayload};
use crate::test_support::{
    env_lock, mark_job_completed_with_audio, seed_summary, seed_transcripts, test_job,
};
use std::fs;
use std::io::Cursor;
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
    let store = IndexStore::new(&repo_root);
    let llama_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let llama_log = repo_root.join("llama-args.log");
    let embedding_log = repo_root.join("llama-embedding.log");
    let prompt_capture = repo_root.join("llama-prompt.txt");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&llama_bin).expect("llama bin");

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

    let mut selected_job = test_job(
        "job-1",
        "2026-01-01T00:00:00Z",
        "sources/job-1/source.wav",
        "hash-job-1",
        "input.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut selected_job,
        "2026-01-01T00:00:01Z",
        &["channel_01.wav", "channel_02.wav", "mono_mix.wav"],
    )
    .expect("mark selected completed");
    store.insert_job(selected_job).expect("insert selected job");
    seed_transcripts(
        &store,
        "job-1",
        &[
            ("channel_01", "화자 A가 일정과 비용을 설명했다."),
            ("channel_02", "화자 B가 일정과 비용을 다시 확인했다."),
            (
                "mono_mix",
                "전체 대화에서 다음 주 방문과 견적 검토가 언급되었다.",
            ),
        ],
    )
    .expect("seed transcripts");

    let mut ignored_job = test_job(
        "job-2",
        "2026-01-01T00:00:01Z",
        "sources/job-2/source.wav",
        "hash-job-2",
        "other.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut ignored_job,
        "2026-01-01T00:00:02Z",
        &["mono_mix.wav"],
    )
    .expect("mark ignored completed");
    store.insert_job(ignored_job).expect("insert ignored job");

    let mut reader = Cursor::new(b"1\n".to_vec());
    let mut output = Vec::new();

    let summary =
        run_summary_with_repo_root(&repo_root, &mut reader, &mut output).expect("summary run");

    unsafe { std::env::remove_var("HF_TOKEN") };

    assert_eq!(summary.job_id, "job-1");
    assert_eq!(
        summary.summary_file,
        repo_root.join("db/audio-spool/summary/job-1/result.md")
    );
    assert!(summary.summary_dir.is_dir());
    assert_eq!(
        fs::read_to_string(&summary.summary_file).expect("summary file"),
        "synthetic summary"
    );
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
fn run_summary_reuses_existing_summary_record_without_llama_toolchain() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let mut job = test_job(
        "job-1",
        "2026-01-01T00:00:00Z",
        "sources/job-1/source.wav",
        "hash-job-1",
        "input.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut job,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark completed");
    store.insert_job(job).expect("insert selected job");
    seed_transcripts(&store, "job-1", &[("mono_mix", "현장 방문 일정을 논의했다.")])
        .expect("seed transcript");
    seed_summary(&store, "job-1", "existing summary").expect("seed summary");

    let mut reader = Cursor::new(b"1\n".to_vec());
    let mut output = Vec::new();

    let summary =
        run_summary_with_repo_root(&repo_root, &mut reader, &mut output).expect("summary run");

    assert_eq!(summary.job_id, "job-1");
    assert_eq!(
        summary.summary_dir,
        repo_root.join("db/audio-spool/summary/job-1")
    );
    assert_eq!(
        summary.summary_file,
        repo_root.join("db/audio-spool/summary/job-1/result.md")
    );
    assert_eq!(
        fs::read_to_string(&summary.summary_file).expect("summary file"),
        "existing summary"
    );
    assert!(!summary.summary_dir.join(".input.prompt.txt").exists());
}

#[test]
fn run_summary_uses_local_model_path_when_file_exists() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let repo_root = temp_workspace();
    let llama_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let llama_log = repo_root.join("llama-local.log");
    let embedding_log = repo_root.join("llama-local-embedding.log");
    let prompt_capture = repo_root.join("llama-local-prompt.txt");
    let model_path = repo_root.join("models/llama/custom.gguf");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&llama_bin).expect("llama bin");
    fs::create_dir_all(model_path.parent().expect("parent")).expect("models dir");
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
    let mut job = test_job(
        "job-1",
        "2026-01-01T00:00:00Z",
        "sources/job-1/source.wav",
        "hash-job-1",
        "input.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut job,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark completed");
    store.insert_job(job).expect("insert selected job");
    seed_transcripts(&store, "job-1", &[("mono_mix", "현장 방문 일정을 논의했다.")])
        .expect("seed transcript");

    let mut reader = Cursor::new(b"1\n".to_vec());
    let mut output = Vec::new();

    let summary =
        run_summary_with_repo_root(&repo_root, &mut reader, &mut output).expect("summary run");

    unsafe { std::env::remove_var("RECORDROUTE_LLAMA_MODEL") };

    assert!(summary.summary_file.is_file());
    let llama_log = fs::read_to_string(llama_log).expect("llama log");
    assert!(llama_log.contains("-m"));
    assert!(llama_log.contains(model_path.to_string_lossy().as_ref()));
    assert!(!llama_log.contains("-hf"));

    let embedding_log = fs::read_to_string(embedding_log).expect("embedding log");
    assert!(embedding_log.contains("-hf"));
}

#[test]
fn submit_summary_upgrades_queued_request_to_force_regenerate() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let mut job = test_job(
        "job-1",
        "2026-01-01T00:00:00Z",
        "sources/job-1/source.wav",
        "hash-job-1",
        "input.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut job,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark completed");
    store.insert_job(job).expect("insert job");
    seed_transcripts(&store, "job-1", &[("mono_mix", "transcript")]).expect("seed transcript");

    let first = submit_summary_job(&repo_root, "job-1", false).expect("initial summary submit");
    assert!(first.should_execute());

    let upgraded = submit_summary_job(&repo_root, "job-1", true).expect("upgrade summary submit");
    assert!(upgraded.should_execute());
    assert!(!upgraded.deduplicated());

    let snapshot = queue_snapshot(&repo_root).expect("queue snapshot");
    let entry = snapshot.pending_batches[0].entries[0].clone();
    assert!(matches!(
        entry.payload,
        QueuePayload::Summary {
            force_regenerate: true
        }
    ));

    let deduplicated =
        submit_summary_job(&repo_root, "job-1", false).expect("compatible summary submit");
    assert!(deduplicated.deduplicated());
}
