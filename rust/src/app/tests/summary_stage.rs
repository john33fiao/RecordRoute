use super::super::*;
use super::support::*;
use crate::index::{DictionaryKeywordSource, IndexStore, QueuePayload, TaskStatus, TaskType};
use crate::test_support::{
    env_lock, mark_job_completed_with_audio, seed_summary, seed_summary_with_one_line,
    seed_transcripts, test_job,
};
use std::fs;
use std::io::Cursor;

#[test]
fn run_summary_reports_current_storage_model_when_no_candidates_exist() {
    let repo_root = temp_workspace();
    let mut reader = Cursor::new(Vec::<u8>::new());
    let mut output = Vec::new();

    let error =
        run_summary_with_repo_root(&repo_root, &mut reader, &mut output).expect_err("summary");

    assert_eq!(error, "no completed jobs with transcription data found");
    assert!(!error.contains("db/index.json"));
}

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

    let persisted_summary = store
        .get_summary("job-1")
        .expect("persisted summary read")
        .expect("persisted summary");
    assert_eq!(persisted_summary.text, "synthetic summary");
    assert_eq!(
        persisted_summary.one_line_summary.as_deref(),
        Some("synthetic one-line summary")
    );
    let stored_keywords = store
        .list_stt_dictionary_keywords()
        .expect("stored auto keywords");
    assert!(stored_keywords.user_keywords.is_empty());
    assert_eq!(
        sorted_strings(stored_keywords.auto_keywords),
        sorted_strings(vec![
            "회의".to_string(),
            "일정".to_string(),
            "견적".to_string(),
        ])
    );

    let prompt = fs::read_to_string(prompt_capture).expect("prompt capture");
    assert!(prompt.contains("[channel_01.txt]"));
    assert!(prompt.contains("[channel_02.txt]"));
    assert!(prompt.contains("[mono_mix.txt]"));
    assert!(prompt.contains("Markdown"));
    assert!(prompt.contains("## 요약"));
    assert!(prompt.contains("통화, 회의, 음성 메모"));
    assert!(prompt.contains("화자 A가 일정과 비용을 설명했다."));
    assert!(prompt.contains("한줄 별칭"));

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
fn run_summary_skips_existing_dictionary_entries_when_storing_keywords() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        std::env::remove_var("RECORDROUTE_LLAMA_MODEL");
    }

    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let llama_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let llama_log = repo_root.join("llama-keywords.log");
    let prompt_capture = repo_root.join("llama-keywords-prompts.txt");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&llama_bin).expect("llama bin");

    write_build_script(&build_script_path(&repo_root, "llama"));
    write_fake_llama_cli_with_keyword_output(
        &fake_command_path(&llama_bin, "llama-cli"),
        &llama_log,
        &prompt_capture,
        false,
        "회의\n회의\n일정\n신규키워드\n",
    );
    write_fake_llama_embedding(
        &fake_command_path(&llama_bin, "llama-embedding"),
        &repo_root.join("llama-keywords-embedding.log"),
    );

    store
        .upsert_stt_dictionary_keyword("회의", DictionaryKeywordSource::User)
        .expect("seed user keyword");
    store
        .upsert_stt_dictionary_keyword("일정", DictionaryKeywordSource::Auto)
        .expect("seed auto keyword");

    let mut job = test_job(
        "job-1",
        "2026-01-01T00:00:00Z",
        "sources/job-1/source.wav",
        "hash-job-1",
        "input.wav",
    );
    mark_job_completed_with_audio(&store, &mut job, "2026-01-01T00:00:01Z", &["mono_mix.wav"])
        .expect("mark completed");
    store.insert_job(job).expect("insert job");
    seed_transcripts(
        &store,
        "job-1",
        &[("mono_mix", "회의 일정과 견적 검토를 다시 잡기로 했다.")],
    )
    .expect("seed transcript");

    let mut reader = Cursor::new(b"1\n".to_vec());
    let mut output = Vec::new();
    run_summary_with_repo_root(&repo_root, &mut reader, &mut output).expect("summary run");

    let stored_keywords = store
        .list_stt_dictionary_keywords()
        .expect("stored keywords after summary");
    assert_eq!(stored_keywords.user_keywords, vec!["회의".to_string()]);
    assert_eq!(
        sorted_strings(stored_keywords.auto_keywords),
        sorted_strings(vec!["일정".to_string(), "신규키워드".to_string()])
    );
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
    mark_job_completed_with_audio(&store, &mut job, "2026-01-01T00:00:01Z", &["mono_mix.wav"])
        .expect("mark completed");
    store.insert_job(job).expect("insert selected job");
    seed_transcripts(
        &store,
        "job-1",
        &[("mono_mix", "현장 방문 일정을 논의했다.")],
    )
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

    let persisted_summary = store
        .get_summary("job-1")
        .expect("persisted summary read")
        .expect("persisted summary");
    assert_eq!(persisted_summary.text, "existing summary");
    assert_eq!(persisted_summary.one_line_summary, None);
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
    mark_job_completed_with_audio(&store, &mut job, "2026-01-01T00:00:01Z", &["mono_mix.wav"])
        .expect("mark completed");
    store.insert_job(job).expect("insert selected job");
    seed_transcripts(
        &store,
        "job-1",
        &[("mono_mix", "현장 방문 일정을 논의했다.")],
    )
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
    mark_job_completed_with_audio(&store, &mut job, "2026-01-01T00:00:01Z", &["mono_mix.wav"])
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

#[test]
fn force_regenerate_rebuilds_summary_and_one_line_summary_together() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        std::env::remove_var("RECORDROUTE_LLAMA_MODEL");
    }

    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let llama_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let llama_log = repo_root.join("llama-force.log");
    let prompt_capture = repo_root.join("llama-force-prompts.txt");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&llama_bin).expect("llama bin");

    write_build_script(&build_script_path(&repo_root, "llama"));
    write_fake_llama_cli(
        &fake_command_path(&llama_bin, "llama-cli"),
        &llama_log,
        &prompt_capture,
        false,
    );

    let mut job = test_job(
        "job-1",
        "2026-01-01T00:00:00Z",
        "sources/job-1/source.wav",
        "hash-job-1",
        "input.wav",
    );
    mark_job_completed_with_audio(&store, &mut job, "2026-01-01T00:00:01Z", &["mono_mix.wav"])
        .expect("mark completed");
    store.insert_job(job).expect("insert job");
    seed_transcripts(
        &store,
        "job-1",
        &[("mono_mix", "현장 일정과 견적 검토를 다시 잡기로 했다.")],
    )
    .expect("seed transcript");
    seed_summary_with_one_line(&store, "job-1", "old summary", Some("old one-line summary"))
        .expect("seed summary");

    let submission = submit_summary_job(&repo_root, "job-1", true).expect("submit summary");
    assert!(submission.should_execute());

    queue::dispatch_until_task_terminal(&repo_root, "job-1", TaskType::Summary)
        .expect("dispatch summary");

    let persisted_summary = store
        .get_summary("job-1")
        .expect("persisted summary read")
        .expect("persisted summary");
    assert_eq!(persisted_summary.text, "synthetic summary");
    assert_eq!(
        persisted_summary.one_line_summary.as_deref(),
        Some("synthetic one-line summary")
    );
}

#[test]
fn summary_keyword_generation_failure_rolls_back_summary_commit() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        std::env::remove_var("RECORDROUTE_LLAMA_MODEL");
    }

    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let llama_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let llama_log = repo_root.join("llama-keyword-failure.log");
    let prompt_capture = repo_root.join("llama-keyword-failure-prompts.txt");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&llama_bin).expect("llama bin");

    write_build_script(&build_script_path(&repo_root, "llama"));
    write_fake_llama_cli_with_keyword_output(
        &fake_command_path(&llama_bin, "llama-cli"),
        &llama_log,
        &prompt_capture,
        false,
        "## 키워드\n",
    );

    let mut job = test_job(
        "job-1",
        "2026-01-01T00:00:00Z",
        "sources/job-1/source.wav",
        "hash-job-1",
        "input.wav",
    );
    mark_job_completed_with_audio(&store, &mut job, "2026-01-01T00:00:01Z", &["mono_mix.wav"])
        .expect("mark completed");
    store.insert_job(job).expect("insert job");
    seed_transcripts(
        &store,
        "job-1",
        &[("mono_mix", "회의 일정과 견적 검토를 다시 잡기로 했다.")],
    )
    .expect("seed transcript");

    let submission = submit_summary_job(&repo_root, "job-1", false).expect("submit summary");
    assert!(submission.should_execute());
    let error = queue::dispatch_until_task_terminal(&repo_root, "job-1", TaskType::Summary)
        .expect_err("summary keyword extraction should fail");
    assert!(error.contains("generated summary keywords are empty"));

    let persisted_job = store
        .find_job("job-1")
        .expect("reload job")
        .expect("job should exist");
    assert_eq!(
        persisted_job
            .task(TaskType::Summary)
            .map(|task| task.status),
        Some(TaskStatus::Failed)
    );
    assert!(
        persisted_job.task(TaskType::Embedding).is_none(),
        "embedding follow-up should not be submitted after summary failure"
    );
    assert!(
        store.get_summary("job-1").expect("summary read").is_none(),
        "summary row should not be partially committed"
    );
    let stored_keywords = store
        .list_stt_dictionary_keywords()
        .expect("stored keywords after failure");
    assert!(stored_keywords.user_keywords.is_empty());
    assert!(stored_keywords.auto_keywords.is_empty());
}

fn sorted_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}
