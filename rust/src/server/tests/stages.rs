use super::super::resolve_stt_subset;
use super::super::router_with_repo_root;
use super::super::types::{
    ErrorResponse, SttProgressResponse, SttRequest, SttTranscriptListResponse, SttTranscriptText,
    SummaryTextResponse,
};
use super::support::*;
use crate::index::{IndexStore, JobRecord, TaskType};
use axum::http::StatusCode;
use std::fs;
use std::path::PathBuf;
use tower::util::ServiceExt;
#[test]
fn resolve_stt_subset_defaults_to_all_audio_files() {
    let subset = resolve_stt_subset(SttRequest {
        audio_files: Vec::new(),
        mono_mix_only: false,
    })
    .expect("subset");
    assert_eq!(subset, None);
}

#[test]
fn resolve_stt_subset_supports_mono_mix_only_mode() {
    let subset = resolve_stt_subset(SttRequest {
        audio_files: Vec::new(),
        mono_mix_only: true,
    })
    .expect("subset");
    assert_eq!(subset, Some(vec!["mono_mix.wav".to_string()]));
}

#[test]
fn resolve_stt_subset_rejects_mixed_modes() {
    let error = resolve_stt_subset(SttRequest {
        audio_files: vec!["channel_01.wav".to_string()],
        mono_mix_only: true,
    })
    .expect_err("mixed mode should fail");
    assert!(error.contains("audio_files cannot be combined with mono_mix_only"));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_stt_texts_returns_transcript_texts_as_json() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let job_id = "job-stt-texts";
    let job_dir = store.job_dir(job_id);
    fs::create_dir_all(job_dir.join("stt")).expect("stt dir");
    fs::write(job_dir.join("stt/channel_01.txt"), "channel transcript").expect("channel");
    fs::write(job_dir.join("stt/mono_mix.txt"), "mono transcript").expect("mono");

    store
        .insert_job(JobRecord::new(
            job_id.to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/stt.wav"),
            job_dir,
        ))
        .expect("insert job");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/jobs/job-stt-texts/stt/texts"))
        .await
        .expect("stt texts response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: SttTranscriptListResponse = read_json(response).await;
    assert_eq!(body.job_id, job_id);
    assert_eq!(body.transcripts.len(), 2);
    assert_eq!(body.transcripts[0].transcript_id, "channel_01");
    assert_eq!(body.transcripts[0].text, "channel transcript");
    assert_eq!(body.transcripts[1].transcript_id, "mono_mix");
    assert_eq!(body.transcripts[1].text, "mono transcript");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_stt_progress_returns_polling_snapshot() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let job_id = "job-stt-progress";
    let job_dir = store.job_dir(job_id);
    fs::create_dir_all(job_dir.join("stt")).expect("stt dir");
    fs::write(job_dir.join("channel_01.wav"), "audio-1").expect("audio-1");
    fs::write(job_dir.join("channel_02.wav"), "audio-2").expect("audio-2");
    fs::write(job_dir.join("mono_mix.wav"), "audio-3").expect("audio-3");
    fs::write(job_dir.join("stt/channel_01.txt"), "channel transcript").expect("transcript");

    let mut job = JobRecord::new(
        job_id.to_string(),
        "2026-01-01T00:00:00Z".to_string(),
        PathBuf::from("/tmp/stt-progress.wav"),
        job_dir,
    );
    job.upsert_running_task(TaskType::Stt, "2026-01-01T00:00:10Z".to_string());
    store.insert_job(job).expect("insert job");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/jobs/job-stt-progress/stt/progress"))
        .await
        .expect("stt progress response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: SttProgressResponse = read_json(response).await;
    assert_eq!(body.job_id, job_id);
    assert_eq!(body.phase, "running");
    assert_eq!(body.total_files, 3);
    assert_eq!(body.completed_files, 1);
    assert_eq!(body.progress_percent, 33);
    assert!(body.task.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn get_stt_text_returns_single_transcript_json() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let job_id = "job-stt-text";
    let job_dir = store.job_dir(job_id);
    fs::create_dir_all(job_dir.join("stt")).expect("stt dir");
    fs::write(job_dir.join("stt/channel_01.txt"), "single transcript").expect("channel");

    store
        .insert_job(JobRecord::new(
            job_id.to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/stt-single.wav"),
            job_dir,
        ))
        .expect("insert job");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/jobs/job-stt-text/stt/texts/channel_01"))
        .await
        .expect("stt text response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: SttTranscriptText = read_json(response).await;
    assert_eq!(body.transcript_id, "channel_01");
    assert_eq!(body.file_name, "channel_01.txt");
    assert_eq!(body.text, "single transcript");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_stt_text_returns_404_for_missing_transcript() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let job_id = "job-stt-missing";
    let job_dir = store.job_dir(job_id);
    fs::create_dir_all(job_dir.join("stt")).expect("stt dir");

    store
        .insert_job(JobRecord::new(
            job_id.to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/stt-missing.wav"),
            job_dir,
        ))
        .expect("insert job");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/jobs/job-stt-missing/stt/texts/not_found"))
        .await
        .expect("stt text missing response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: ErrorResponse = read_json(response).await;
    assert!(body.message.contains("transcript not found: not_found"));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_summary_text_returns_result_txt_as_json() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let job_id = "job-summary-text";
    let job_dir = store.job_dir(job_id);
    fs::create_dir_all(job_dir.join("summary")).expect("summary dir");
    fs::write(job_dir.join("summary/result.txt"), "summary body").expect("summary text");

    store
        .insert_job(JobRecord::new(
            job_id.to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/summary.wav"),
            job_dir,
        ))
        .expect("insert job");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/jobs/job-summary-text/summary/text"))
        .await
        .expect("summary text response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: SummaryTextResponse = read_json(response).await;
    assert_eq!(body.job_id, job_id);
    assert_eq!(body.file_name, "result.txt");
    assert_eq!(body.text, "summary body");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_summary_text_returns_404_when_result_txt_missing() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let job_id = "job-summary-missing";
    let job_dir = store.job_dir(job_id);
    fs::create_dir_all(job_dir.join("summary")).expect("summary dir");

    store
        .insert_job(JobRecord::new(
            job_id.to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            PathBuf::from("/tmp/summary-missing.wav"),
            job_dir,
        ))
        .expect("insert job");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/jobs/job-summary-missing/summary/text"))
        .await
        .expect("summary text missing response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: ErrorResponse = read_json(response).await;
    assert!(body.message.contains("summary not found:"));
}
