use super::super::*;
use super::support::*;
use crate::index::{IndexStore, JobOutputs, JobRecord, QueuePayload, TaskType};
use crate::test_support::env_lock;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

#[test]
fn run_stt_reports_current_storage_model_when_no_candidates_exist() {
    let repo_root = temp_workspace();
    let mut reader = Cursor::new(Vec::<u8>::new());
    let mut output = Vec::new();

    let error = run_stt_with_repo_root(&repo_root, &mut reader, &mut output).expect_err("stt");

    assert_eq!(
        error,
        "no completed jobs with supported audio artifacts found"
    );
    assert!(!error.contains("db/index.json"));
}

#[test]
fn run_stt_processes_audio_files_in_selected_job_dir() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let selected_job_dir = store.job_dir("job-1");
    let ignored_job_dir = store.job_dir("job-2");
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
    assert!(whisper_log.contains("ko"));
    assert!(whisper_log.contains("channel_01.wav"));
    assert!(whisper_log.contains("mono_mix.wav"));

    let persisted_job = store.find_job("job-1").expect("find job").expect("job");
    assert!(persisted_job.task(TaskType::Summary).is_none());
}

#[test]
fn run_stt_retries_invalid_folder_selection() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let selected_job_dir = store.job_dir("job-1");
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
#[test]
fn execute_stt_job_deletes_failed_transcript_and_batch_requeues_job() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe { std::env::remove_var("RECORDROUTE_WHISPER_LANGUAGE") };

    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let job_dir = store.job_dir("job-1");
    let whisper_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let source_dir = repo_root.join("whisper-source");
    fs::create_dir_all(&job_dir).expect("job dir");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&source_dir).expect("source dir");
    fs::create_dir_all(&whisper_bin).expect("whisper bin");

    write_build_script(&build_script_path(&repo_root, "whisper"));
    write_platform_script(
        &fake_command_path(&whisper_bin, "whisper-cli"),
        "#!/bin/sh\nout=''\nnext=''\nfor arg in \"$@\"; do\n  if [ \"$next\" = 'of' ]; then\n    out=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -of)\n      next='of'\n      ;;\n  esac\ndone\nmkdir -p \"$(dirname \"$out\")\"\ncase \"$out\" in\n  *channel_02)\n    printf '\\377\\376A' > \"$out.txt\"\n    ;;\n  *)\n    printf 'good transcript' > \"$out.txt\"\n    ;;\nesac\n",
        "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nset \"out=\"\nset \"next=\"\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\nif /I \"!next!\"==\"of\" (\n  set \"out=!arg!\"\n  set \"next=\"\n) else if /I \"!arg!\"==\"-of\" (\n  set \"next=of\"\n)\nshift\ngoto loop\n:done\nif defined out (\n  for %%I in (\"!out!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n  set \"target=!out!.txt\"\n  echo !target! | findstr /I /C:\"channel_02\" >nul\n  if not errorlevel 1 (\n    powershell -NoProfile -Command \"[System.IO.File]::WriteAllBytes('!target!',[byte[]](0xFF,0xFE,0x41))\" >nul\n  ) else (\n    > \"!target!\" <nul set /p =good transcript\n  )\n)\nexit /b 0\n",
    );
    fs::write(source_dir.join("ggml-base.bin"), "model").expect("source model");
    unsafe { std::env::set_var("RECORDROUTE_WHISPER_MODEL_SOURCE_DIR", &source_dir) };

    let mut job = JobRecord::new(
        "job-1".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
        PathBuf::from("/tmp/input.wav"),
        job_dir,
    );
    super::super::super::test_support::mark_job_completed_with_audio(
        &store,
        &mut job,
        "2026-01-01T00:00:01Z",
        &["channel_01.wav", "channel_02.wav"],
    )
    .expect("mark completed");
    job.enqueue_task(TaskType::Stt, "2026-01-01T00:00:02Z".to_string());
    store.insert_job(job).expect("insert job");

    let error = super::super::stt_stage::execute_stt_job(
        &repo_root,
        "job-1",
        &[
            PathBuf::from("channel_01.wav"),
            PathBuf::from("channel_02.wav"),
        ],
        "ko",
        &[],
    )
    .expect_err("stt should fail");
    unsafe { std::env::remove_var("RECORDROUTE_WHISPER_MODEL_SOURCE_DIR") };

    assert!(error.contains("channel_02"));

    let stt_dir = repo_root.join("db/audio-spool/stt/job-1");
    assert!(stt_dir.join("channel_01.txt").is_file());
    assert!(!stt_dir.join("channel_02.txt").exists());
    assert!(
        store
            .find_transcript("job-1", "channel_01")
            .expect("find transcript")
            .is_some()
    );
    assert!(
        store
            .find_transcript("job-1", "channel_02")
            .expect("find transcript")
            .is_none()
    );

    let persisted_job = store.find_job("job-1").expect("find job").expect("job");
    let task = persisted_job.task(TaskType::Stt).expect("stt task");
    assert_eq!(task.status, crate::index::TaskStatus::Failed);
    assert!(
        task.last_error
            .as_deref()
            .unwrap_or_default()
            .contains("channel_02")
    );

    let batch =
        submit_batch_pipeline_jobs(&repo_root, BatchProcessTarget::Stt).expect("batch submit");
    assert_eq!(batch.stt_queued, 1);

    let entry = store
        .with_index_read(|index| {
            Ok(super::super::queue::find_task_entry(
                index,
                "job-1",
                TaskType::Stt,
            ))
        })
        .expect("read queue")
        .expect("queue entry");
    match entry.payload {
        QueuePayload::Stt { audio_files, .. } => {
            assert_eq!(audio_files, vec!["channel_01.wav", "channel_02.wav"]);
        }
        other => panic!("expected stt payload, got {other:?}"),
    }
}

#[test]
fn submit_stt_rejects_conflicting_inflight_subset_request() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let job_dir = store.job_dir("job-1");
    fs::create_dir_all(&job_dir).expect("job dir");
    fs::write(job_dir.join("mono_mix.wav"), "audio").expect("mono mix");
    fs::write(job_dir.join("channel_01.wav"), "audio").expect("channel");

    let mut job = JobRecord::new(
        "job-1".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
        PathBuf::from("/tmp/input.wav"),
        job_dir,
    );
    job.mark_completed("2026-01-01T00:00:01Z".to_string(), JobOutputs::default())
        .expect("mark completed");
    store.insert_job(job).expect("insert job");

    let first = submit_stt_job(
        &repo_root,
        "job-1",
        Some(vec!["mono_mix.wav".to_string()]),
        Vec::new(),
    )
    .expect("first stt submit");
    assert!(first.should_execute());

    let error = submit_stt_job(
        &repo_root,
        "job-1",
        Some(vec!["channel_01.wav".to_string()]),
        Vec::new(),
    )
    .expect_err("conflicting inflight subset should fail");
    assert!(error.to_string().contains("different options"));
}

#[test]
fn submit_stt_merges_persisted_dictionary_keywords_into_queue_payload() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let job_dir = store.job_dir("job-1");
    fs::create_dir_all(&job_dir).expect("job dir");
    fs::write(job_dir.join("mono_mix.wav"), "audio").expect("mono mix");
    store
        .upsert_stt_dictionary_keyword("RecordRoute", crate::index::DictionaryKeywordSource::User)
        .expect("insert dictionary keyword");
    store
        .upsert_stt_dictionary_keyword("배포", crate::index::DictionaryKeywordSource::Auto)
        .expect("insert auto dictionary keyword");

    let mut job = JobRecord::new(
        "job-1".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
        PathBuf::from("/tmp/input.wav"),
        job_dir,
    );
    job.mark_completed("2026-01-01T00:00:01Z".to_string(), JobOutputs::default())
        .expect("mark completed");
    store.insert_job(job).expect("insert job");

    let submission =
        submit_stt_job(&repo_root, "job-1", None, vec!["Dooray".to_string()]).expect("submit");

    assert!(submission.should_execute());

    let entry = store
        .with_index_read(|index| {
            Ok(super::super::queue::find_task_entry(
                index,
                "job-1",
                TaskType::Stt,
            ))
        })
        .expect("read queue")
        .expect("queue entry");

    match entry.payload {
        QueuePayload::Stt { keywords, .. } => {
            assert_eq!(
                keywords,
                vec!["RecordRoute".to_string(), "Dooray".to_string()]
            );
        }
        other => panic!("expected stt payload, got {other:?}"),
    }
}

#[test]
fn submit_stt_reuses_existing_transcripts_when_profile_matches_completed_task() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe { std::env::remove_var("RECORDROUTE_WHISPER_LANGUAGE") };

    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let mut job = super::super::super::test_support::test_job(
        "job-1",
        "2026-01-01T00:00:00Z",
        "sources/job-1/source.wav",
        "hash-job-1",
        "input.wav",
    );
    super::super::super::test_support::mark_job_completed_with_audio(
        &store,
        &mut job,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark completed");
    job.enqueue_task(TaskType::Stt, "2026-01-01T00:00:02Z".to_string());
    job.set_task_request_fingerprint(
        TaskType::Stt,
        QueuePayload::Stt {
            audio_files: vec!["mono_mix.wav".to_string()],
            language: "ko".to_string(),
            keywords: Vec::new(),
        }
        .request_fingerprint(),
    );
    job.complete_task(TaskType::Stt, "2026-01-01T00:00:03Z".to_string())
        .expect("complete stt");
    store.insert_job(job).expect("insert job");
    super::super::super::test_support::seed_transcripts(&store, "job-1", &[("mono_mix", "text")])
        .expect("seed transcript");

    let submission = submit_stt_job(&repo_root, "job-1", None, Vec::new()).expect("submit stt");

    assert!(submission.reused());
}

#[test]
fn submit_stt_submits_when_keywords_change_completed_task_profile() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe { std::env::remove_var("RECORDROUTE_WHISPER_LANGUAGE") };

    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let mut job = super::super::super::test_support::test_job(
        "job-1",
        "2026-01-01T00:00:00Z",
        "sources/job-1/source.wav",
        "hash-job-1",
        "input.wav",
    );
    super::super::super::test_support::mark_job_completed_with_audio(
        &store,
        &mut job,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark completed");
    job.enqueue_task(TaskType::Stt, "2026-01-01T00:00:02Z".to_string());
    job.set_task_request_fingerprint(
        TaskType::Stt,
        QueuePayload::Stt {
            audio_files: vec!["mono_mix.wav".to_string()],
            language: "ko".to_string(),
            keywords: Vec::new(),
        }
        .request_fingerprint(),
    );
    job.complete_task(TaskType::Stt, "2026-01-01T00:00:03Z".to_string())
        .expect("complete stt");
    store.insert_job(job).expect("insert job");
    super::super::super::test_support::seed_transcripts(&store, "job-1", &[("mono_mix", "text")])
        .expect("seed transcript");

    let submission =
        submit_stt_job(&repo_root, "job-1", None, vec!["Acme".to_string()]).expect("submit stt");

    assert!(submission.should_execute());
    assert!(!submission.reused());
}
