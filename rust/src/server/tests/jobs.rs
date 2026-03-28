use super::super::router_with_repo_root;
use super::super::router_with_repo_root_and_upload_limits;
use super::super::types::{
    BatchQueueSubmissionResponse, ErrorResponse, JobListResponse, JobStatusResponse,
    JobSubmissionResponse,
};
use super::support::*;
use crate::index::{IndexStore, JobOutputs, JobRecord, JobStatus, TaskType};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use std::fs;
use std::path::{Path, PathBuf};
use tower::util::ServiceExt;

#[tokio::test(flavor = "multi_thread")]
async fn post_jobs_batch_process_enqueues_unfinished_pipeline_tasks() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let job = JobRecord::new(
        "job-batch-queued".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
        PathBuf::from("/tmp/job-batch-queued.wav"),
        store.job_dir("job-batch-queued"),
    );
    store.insert_job(job).expect("insert job");

    let app = router_with_repo_root(repo_root.clone());
    let response = app
        .clone()
        .oneshot(post_empty_request("/jobs/batch-process"))
        .await
        .expect("batch process response");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body: BatchQueueSubmissionResponse = read_json(response).await;
    assert_eq!(body.total_jobs, 1);
    assert_eq!(body.ffmpeg_queued, 1);
    assert_eq!(body.stt_queued, 0);
    assert_eq!(body.summary_queued, 0);
    assert_eq!(body.embedding_queued, 0);
}
#[tokio::test(flavor = "multi_thread")]
async fn post_jobs_returns_accepted_then_job_transitions_to_completed() {
    let repo_root = temp_workspace();
    let scripts_dir = repo_root.join("scripts");
    let build_bin = repo_root
        .join(".build/ffmpeg")
        .join(crate::ffmpeg::target_dir_name())
        .join("install/bin");
    let input = repo_root.join("fixture.wav");
    let gate = repo_root.join("ffmpeg.gate");

    fs::create_dir_all(&scripts_dir).expect("scripts dir");
    fs::create_dir_all(&build_bin).expect("toolchain dir");
    write_build_script(&build_script_path(&repo_root, "ffmpeg"));
    write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
    write_blocking_ffmpeg(&fake_command_path(&build_bin, "ffmpeg"), &gate);
    write_test_wav(&input, 2);
    fs::write(&gate, "wait").expect("gate");

    let app = router_with_repo_root(repo_root.clone());

    let response = app
        .clone()
        .oneshot(post_json_request(
            "/jobs",
            &serde_json::json!({ "input_path": input }),
        ))
        .await
        .expect("post jobs response");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let submitted: JobSubmissionResponse = read_json(response).await;
    assert_eq!(submitted.status, JobStatus::Queued);
    assert!(!submitted.reused);
    assert!(!submitted.deduplicated);
    assert_eq!(
        submitted.queue.as_ref().expect("queue info").category,
        crate::index::QueueCategory::Ffmpeg
    );

    let running: JobRecord = read_json(
        app.clone()
            .oneshot(get_request(&format!("/jobs/{}", submitted.job_id)))
            .await
            .expect("get job response"),
    )
    .await;
    assert!(matches!(
        running.status,
        JobStatus::Queued | JobStatus::Running
    ));

    fs::remove_file(&gate).expect("remove gate");
    let completed = wait_for_job_completion(&app, &submitted.job_id).await;

    assert_eq!(completed.status, JobStatus::Completed);
    assert_eq!(completed.probe.channels, Some(2));
    assert!(
        Path::new(
            completed
                .outputs
                .merged_mono_wav
                .as_deref()
                .expect("merged output"),
        )
        .is_file()
    );
    assert_eq!(completed.outputs.split_mono_wavs.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn post_jobs_upload_accepts_file_and_stores_content_hashed_path() {
    let repo_root = temp_workspace();
    let scripts_dir = repo_root.join("scripts");
    let build_bin = repo_root
        .join(".build/ffmpeg")
        .join(crate::ffmpeg::target_dir_name())
        .join("install/bin");
    let gate = repo_root.join("ffmpeg.gate");

    fs::create_dir_all(&scripts_dir).expect("scripts dir");
    fs::create_dir_all(&build_bin).expect("toolchain dir");
    write_build_script(&build_script_path(&repo_root, "ffmpeg"));
    write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 1, Some("mono"));
    write_blocking_ffmpeg(&fake_command_path(&build_bin, "ffmpeg"), &gate);
    fs::write(&gate, "wait").expect("gate");

    let app = router_with_repo_root(repo_root.clone());
    let audio_bytes = b"upload-audio-bytes";
    let response = app
        .clone()
        .oneshot(post_upload_request(
            "/jobs/upload",
            "sample.wav",
            audio_bytes,
        ))
        .await
        .expect("upload response");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let submitted: JobSubmissionResponse = read_json(response).await;
    assert!(!submitted.reused);
    assert!(!submitted.deduplicated);
    assert!(submitted.source_path.ends_with(".bin"));
    assert!(Path::new(&submitted.source_path).is_file());
    assert_eq!(
        fs::read(&submitted.source_path).expect("uploaded path"),
        audio_bytes
    );

    fs::remove_file(&gate).expect("remove gate");
    let completed = wait_for_job_completion(&app, &submitted.job_id).await;
    assert_eq!(completed.status, JobStatus::Completed);
}

#[tokio::test(flavor = "multi_thread")]
async fn post_jobs_returns_400_for_invalid_input_path() {
    let repo_root = temp_workspace();
    let missing_input = repo_root.join("missing.wav");
    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(post_json_request(
            "/jobs",
            &serde_json::json!({ "input_path": missing_input }),
        ))
        .await
        .expect("post jobs invalid path response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: ErrorResponse = read_json(response).await;
    assert!(body.message.contains("input file not found:"));
}

#[tokio::test(flavor = "multi_thread")]
async fn post_jobs_upload_returns_400_for_missing_file_field() {
    let repo_root = temp_workspace();
    let app = router_with_repo_root(repo_root);
    let request = Request::builder()
        .method(Method::POST)
        .uri("/jobs/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={UPLOAD_BOUNDARY}"),
        )
        .body(Body::from(format!(
            "--{UPLOAD_BOUNDARY}\r\nContent-Disposition: form-data; name=\"note\"\r\n\r\nmissing file\r\n--{UPLOAD_BOUNDARY}--\r\n"
        )))
        .expect("upload request");

    let response = app
        .oneshot(request)
        .await
        .expect("upload missing file response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: ErrorResponse = read_json(response).await;
    assert_eq!(body.message, "multipart field 'file' is required");
}

#[tokio::test(flavor = "multi_thread")]
async fn post_jobs_upload_returns_400_for_duplicate_file_field() {
    let repo_root = temp_workspace();
    let app = router_with_repo_root(repo_root);
    let mut body = Vec::new();

    for (filename, bytes) in [
        ("first.wav", b"one".as_slice()),
        ("second.wav", b"two".as_slice()),
    ] {
        body.extend_from_slice(format!("--{UPLOAD_BOUNDARY}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
                .as_bytes(),
        );
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{UPLOAD_BOUNDARY}--\r\n").as_bytes());

    let request = Request::builder()
        .method(Method::POST)
        .uri("/jobs/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={UPLOAD_BOUNDARY}"),
        )
        .body(Body::from(body))
        .expect("duplicate upload request");

    let response = app
        .oneshot(request)
        .await
        .expect("duplicate upload response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: ErrorResponse = read_json(response).await;
    assert_eq!(body.message, "multipart field 'file' must appear only once");
}

#[tokio::test(flavor = "multi_thread")]
async fn post_jobs_upload_returns_400_for_empty_file() {
    let repo_root = temp_workspace();
    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(post_upload_request("/jobs/upload", "empty.wav", b""))
        .await
        .expect("empty upload response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: ErrorResponse = read_json(response).await;
    assert_eq!(body.message, "uploaded file is empty");
}

#[tokio::test(flavor = "multi_thread")]
async fn post_jobs_upload_returns_413_and_cleans_temp_files_when_file_exceeds_limit() {
    let repo_root = temp_workspace();
    let app = router_with_repo_root_and_upload_limits(
        repo_root.clone(),
        super::super::upload::UploadLimits::new(16, 4096),
    );
    let oversized = vec![b'a'; 17];
    let response = app
        .oneshot(post_upload_request(
            "/jobs/upload",
            "too-big.wav",
            &oversized,
        ))
        .await
        .expect("oversized upload response");

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let body: ErrorResponse = read_json(response).await;
    assert_eq!(body.message, super::super::upload::upload_limit_message(16));

    let temp_dir = repo_root.join("db/uploads/.tmp");
    if temp_dir.is_dir() {
        let entries = fs::read_dir(&temp_dir)
            .expect("temp dir")
            .collect::<Result<Vec<_>, _>>()
            .expect("temp dir entries");
        assert!(entries.is_empty());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn post_jobs_upload_returns_400_and_cleans_temp_files_for_malformed_multipart() {
    let repo_root = temp_workspace();
    let app = router_with_repo_root(repo_root.clone());
    let body = format!(
        "--{UPLOAD_BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"sample.wav\"\r\nContent-Type: application/octet-stream\r\n\r\nhello\r\n--{UPLOAD_BOUNDARY}\r\nContent-Disposition: form-data; name=\"note\"\r\n"
    );
    let request = Request::builder()
        .method(Method::POST)
        .uri("/jobs/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={UPLOAD_BOUNDARY}"),
        )
        .body(Body::from(body))
        .expect("malformed upload request");

    let response = app
        .oneshot(request)
        .await
        .expect("malformed upload response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: ErrorResponse = read_json(response).await;
    assert_eq!(body.message, "invalid multipart body");

    let temp_dir = repo_root.join("db/uploads/.tmp");
    if temp_dir.is_dir() {
        let entries = fs::read_dir(&temp_dir)
            .expect("temp dir")
            .collect::<Result<Vec<_>, _>>()
            .expect("temp dir entries");
        assert!(entries.is_empty());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn post_jobs_upload_reuses_completed_job_for_same_content() {
    let repo_root = temp_workspace();
    let scripts_dir = repo_root.join("scripts");
    let build_bin = repo_root
        .join(".build/ffmpeg")
        .join(crate::ffmpeg::target_dir_name())
        .join("install/bin");

    fs::create_dir_all(&scripts_dir).expect("scripts dir");
    fs::create_dir_all(&build_bin).expect("toolchain dir");
    write_build_script(&build_script_path(&repo_root, "ffmpeg"));
    write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
    write_fake_ffmpeg(&fake_command_path(&build_bin, "ffmpeg"));

    let app = router_with_repo_root(repo_root.clone());
    let audio_bytes = b"same-content";

    let first_response = app
        .clone()
        .oneshot(post_upload_request(
            "/jobs/upload",
            "first.wav",
            audio_bytes,
        ))
        .await
        .expect("first upload response");
    assert_eq!(first_response.status(), StatusCode::ACCEPTED);
    let first: JobSubmissionResponse = read_json(first_response).await;
    let completed = wait_for_job_completion(&app, &first.job_id).await;
    assert_eq!(completed.status, JobStatus::Completed);

    fs::remove_file(fake_command_path(&build_bin, "ffmpeg")).expect("remove ffmpeg");
    fs::remove_file(fake_command_path(&build_bin, "ffprobe")).expect("remove ffprobe");

    let second_response = app
        .oneshot(post_upload_request(
            "/jobs/upload",
            "second.wav",
            audio_bytes,
        ))
        .await
        .expect("second upload response");

    assert_eq!(second_response.status(), StatusCode::OK);
    let second: JobSubmissionResponse = read_json(second_response).await;
    assert_eq!(first.job_id, second.job_id);
    assert!(second.reused);
    assert!(!second.deduplicated);
}

#[tokio::test(flavor = "multi_thread")]
async fn post_jobs_deduplicates_running_job() {
    let repo_root = temp_workspace();
    let scripts_dir = repo_root.join("scripts");
    let build_bin = repo_root
        .join(".build/ffmpeg")
        .join(crate::ffmpeg::target_dir_name())
        .join("install/bin");
    let input = repo_root.join("fixture.wav");
    let gate = repo_root.join("ffmpeg.gate");

    fs::create_dir_all(&scripts_dir).expect("scripts dir");
    fs::create_dir_all(&build_bin).expect("toolchain dir");
    write_build_script(&build_script_path(&repo_root, "ffmpeg"));
    write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 1, Some("mono"));
    write_blocking_ffmpeg(&fake_command_path(&build_bin, "ffmpeg"), &gate);
    write_test_wav(&input, 1);
    fs::write(&gate, "wait").expect("gate");

    let app = router_with_repo_root(repo_root.clone());

    let first: JobSubmissionResponse = read_json(
        app.clone()
            .oneshot(post_json_request(
                "/jobs",
                &serde_json::json!({ "input_path": input }),
            ))
            .await
            .expect("first response"),
    )
    .await;
    let second: JobSubmissionResponse = read_json(
        app.clone()
            .oneshot(post_json_request(
                "/jobs",
                &serde_json::json!({ "input_path": input }),
            ))
            .await
            .expect("second response"),
    )
    .await;

    assert_eq!(first.job_id, second.job_id);
    assert!(matches!(
        second.status,
        JobStatus::Queued | JobStatus::Running
    ));
    assert!(!second.reused);
    assert!(second.deduplicated);

    fs::remove_file(&gate).expect("remove gate");
    let completed = wait_for_job_completion(&app, &first.job_id).await;
    assert_eq!(completed.status, JobStatus::Completed);
}

#[tokio::test(flavor = "multi_thread")]
async fn post_jobs_reuses_completed_job() {
    let repo_root = temp_workspace();
    let scripts_dir = repo_root.join("scripts");
    let build_bin = repo_root
        .join(".build/ffmpeg")
        .join(crate::ffmpeg::target_dir_name())
        .join("install/bin");
    let input = repo_root.join("fixture.wav");

    fs::create_dir_all(&scripts_dir).expect("scripts dir");
    fs::create_dir_all(&build_bin).expect("toolchain dir");
    write_build_script(&build_script_path(&repo_root, "ffmpeg"));
    write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
    write_fake_ffmpeg(&fake_command_path(&build_bin, "ffmpeg"));
    write_test_wav(&input, 2);

    crate::app::run_with_repo_root(&repo_root, &input).expect("seed completed job");

    fs::remove_file(fake_command_path(&build_bin, "ffmpeg")).expect("remove ffmpeg");
    fs::remove_file(fake_command_path(&build_bin, "ffprobe")).expect("remove ffprobe");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(post_json_request(
            "/jobs",
            &serde_json::json!({ "input_path": input }),
        ))
        .await
        .expect("reuse response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: JobSubmissionResponse = read_json(response).await;
    assert_eq!(body.status, JobStatus::Completed);
    assert!(body.reused);
    assert!(!body.deduplicated);
    assert!(body.outputs.merged_mono_wav.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn get_jobs_returns_latest_first() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);

    store
        .insert_job(JobRecord::new(
            "job-1".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/first.wav"),
            store.job_dir("job-1"),
        ))
        .expect("insert first job");
    store
        .insert_job(JobRecord::new(
            "job-2".to_string(),
            "2026-01-01T00:00:01Z".to_string(),
            PathBuf::from("/tmp/second.wav"),
            store.job_dir("job-2"),
        ))
        .expect("insert second job");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/jobs"))
        .await
        .expect("get jobs response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: JobListResponse = read_json(response).await;
    assert_eq!(body.jobs.len(), 2);
    assert_eq!(body.jobs[0].job_id, "job-2");
    assert_eq!(body.jobs[1].job_id, "job-1");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_jobs_completed_returns_only_completed_jobs() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);

    let mut completed_job = JobRecord::new(
        "job-completed".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
        PathBuf::from("/tmp/completed.wav"),
        store.job_dir("job-completed"),
    );
    completed_job
        .mark_completed(
            "2026-01-01T00:00:01Z".to_string(),
            JobOutputs {
                merged_mono_wav: Some("/tmp/completed_mono.wav".to_string()),
                split_mono_wavs: Vec::new(),
            },
        )
        .expect("mark completed job");
    store
        .insert_job(completed_job)
        .expect("insert completed job");
    store
        .insert_job(JobRecord::new(
            "job-running".to_string(),
            "2026-01-01T00:00:02Z".to_string(),
            PathBuf::from("/tmp/running.wav"),
            store.job_dir("job-running"),
        ))
        .expect("insert running job");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/jobs/completed"))
        .await
        .expect("get completed jobs response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: JobListResponse = read_json(response).await;
    assert_eq!(body.jobs.len(), 1);
    assert_eq!(body.jobs[0].job_id, "job-completed");
    assert_eq!(body.jobs[0].status, JobStatus::Completed);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_jobs_by_source_filters_jobs() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);

    store
        .insert_job(JobRecord::new(
            "job-target-1".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/target.wav"),
            store.job_dir("job-target-1"),
        ))
        .expect("insert target job 1");
    store
        .insert_job(JobRecord::new(
            "job-other".to_string(),
            "2026-01-01T00:00:01Z".to_string(),
            PathBuf::from("/tmp/other.wav"),
            store.job_dir("job-other"),
        ))
        .expect("insert other job");
    store
        .insert_job(JobRecord::new(
            "job-target-2".to_string(),
            "2026-01-01T00:00:02Z".to_string(),
            PathBuf::from("/tmp/target.wav"),
            store.job_dir("job-target-2"),
        ))
        .expect("insert target job 2");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/jobs/by-source?source_path=/tmp/target.wav"))
        .await
        .expect("get jobs by source response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: JobListResponse = read_json(response).await;
    assert_eq!(body.jobs.len(), 2);
    assert_eq!(body.jobs[0].job_id, "job-target-2");
    assert_eq!(body.jobs[1].job_id, "job-target-1");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_jobs_by_source_requires_source_path_query() {
    let repo_root = temp_workspace();
    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/jobs/by-source"))
        .await
        .expect("get jobs by source without query response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: ErrorResponse = read_json(response).await;
    assert_eq!(body.message, "source_path query is required");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_job_status_returns_integrated_stage_status() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);

    let mut job = JobRecord::new(
        "job-status".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
        PathBuf::from("/tmp/status.wav"),
        store.job_dir("job-status"),
    );
    job.upsert_running_task(TaskType::Stt, "2026-01-01T00:00:01Z".to_string());
    job.complete_task(TaskType::Stt, "2026-01-01T00:00:02Z".to_string())
        .expect("complete stt task");
    job.upsert_running_task(TaskType::Summary, "2026-01-01T00:00:03Z".to_string());
    job.status = JobStatus::Running;
    store.insert_job(job).expect("insert status job");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/jobs/job-status/status"))
        .await
        .expect("get job status response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: JobStatusResponse = read_json(response).await;
    assert_eq!(body.job_id, "job-status");
    assert_eq!(body.job_status, JobStatus::Running);
    assert_eq!(body.tasks.len(), 3);
    assert!(
        body.tasks
            .iter()
            .any(|task| task.task_type == TaskType::Ffmpeg)
    );
    assert!(
        body.tasks
            .iter()
            .any(|task| task.task_type == TaskType::Stt)
    );
    assert!(
        body.tasks
            .iter()
            .any(|task| task.task_type == TaskType::Summary)
    );
    assert_eq!(body.error_message, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_job_status_returns_404_for_missing_job() {
    let repo_root = temp_workspace();
    let app = router_with_repo_root(repo_root);

    let response = app
        .oneshot(get_request("/jobs/missing/status"))
        .await
        .expect("missing status response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: ErrorResponse = read_json(response).await;
    assert_eq!(body.code, "404");
    assert!(body.message.contains("job not found: missing"));
}

#[tokio::test(flavor = "multi_thread")]
async fn post_jobs_marks_failed_job_and_preserves_error_message() {
    let repo_root = temp_workspace();
    let scripts_dir = repo_root.join("scripts");
    let build_bin = repo_root
        .join(".build/ffmpeg")
        .join(crate::ffmpeg::target_dir_name())
        .join("install/bin");
    let input = repo_root.join("fixture.wav");

    fs::create_dir_all(&scripts_dir).expect("scripts dir");
    fs::create_dir_all(&build_bin).expect("toolchain dir");
    write_build_script(&build_script_path(&repo_root, "ffmpeg"));
    write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
    write_failing_ffmpeg(&fake_command_path(&build_bin, "ffmpeg"));
    write_test_wav(&input, 2);

    let app = router_with_repo_root(repo_root.clone());
    let submission: JobSubmissionResponse = read_json(
        app.clone()
            .oneshot(post_json_request(
                "/jobs",
                &serde_json::json!({ "input_path": input }),
            ))
            .await
            .expect("submit response"),
    )
    .await;

    let failed = wait_for_job_terminal_state(&app, &submission.job_id).await;
    assert_eq!(failed.status, JobStatus::Failed);
    assert!(
        failed
            .error_message
            .as_deref()
            .expect("error message")
            .contains("ffmpeg conversion failed")
    );

    let job_dir = PathBuf::from(&failed.job_dir);
    assert!(!job_dir.join("channel_01.wav").exists());
    assert!(!job_dir.join("channel_02.wav").exists());
    assert!(!job_dir.join("mono_mix.wav").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn get_job_status_sanitizes_setup_related_task_errors() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);

    let mut job = JobRecord::new(
        "job-setup-error".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
        PathBuf::from("/tmp/setup-error.wav"),
        store.job_dir("job-setup-error"),
    );
    job.upsert_running_task(TaskType::Summary, "2026-01-01T00:00:01Z".to_string());
    job.fail_task(
        TaskType::Summary,
        "2026-01-01T00:00:02Z".to_string(),
        "local llama toolchain not found. Build it first with /tmp/build_llama.sh".to_string(),
    )
    .expect("fail summary task");
    store.insert_job(job).expect("insert job");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/jobs/job-setup-error/status"))
        .await
        .expect("get job status response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: JobStatusResponse = read_json(response).await;
    let summary_task = body
        .tasks
        .iter()
        .find(|task| task.task_type == TaskType::Summary)
        .expect("summary task");
    assert_eq!(
        summary_task.last_error.as_deref(),
        Some("환경 준비가 필요합니다. setup을 다시 실행하세요."),
    );
}
