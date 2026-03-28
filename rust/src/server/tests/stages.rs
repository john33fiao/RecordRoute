use super::super::resolve_stt_subset;
use super::super::router_with_repo_root;
use super::super::types::{
    ErrorResponse, SttProgressResponse, SttRequest, SttTranscriptListResponse, SttTranscriptText,
    SummaryTextResponse,
};
use super::support::*;
use crate::index::{IndexStore, TaskType};
use crate::test_support::{
    mark_job_completed_with_audio, seed_summary, seed_transcripts, test_job,
};
use axum::http::StatusCode;
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
    let mut job = test_job(
        job_id,
        "2026-01-01T00:00:00Z",
        "sources/job-stt-texts/source.wav",
        "hash-stt-texts",
        "stt.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut job,
        "2026-01-01T00:00:01Z",
        &["channel_01.wav", "mono_mix.wav"],
    )
    .expect("mark completed");
    store.insert_job(job).expect("insert job");
    seed_transcripts(
        &store,
        job_id,
        &[("channel_01", "channel transcript"), ("mono_mix", "mono transcript")],
    )
    .expect("seed transcripts");

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
    let mut job = test_job(
        job_id,
        "2026-01-01T00:00:00Z",
        "sources/job-stt-progress/source.wav",
        "hash-stt-progress",
        "stt-progress.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut job,
        "2026-01-01T00:00:01Z",
        &["channel_01.wav", "channel_02.wav", "mono_mix.wav"],
    )
    .expect("mark completed");
    job.upsert_running_task(TaskType::Stt, "2026-01-01T00:00:10Z".to_string());
    store.insert_job(job).expect("insert job");
    seed_transcripts(&store, job_id, &[("channel_01", "channel transcript")]).expect("seed transcript");

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
    let mut job = test_job(
        job_id,
        "2026-01-01T00:00:00Z",
        "sources/job-stt-text/source.wav",
        "hash-stt-text",
        "stt-single.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut job,
        "2026-01-01T00:00:01Z",
        &["channel_01.wav"],
    )
    .expect("mark completed");
    store.insert_job(job).expect("insert job");
    seed_transcripts(&store, job_id, &[("channel_01", "single transcript")]).expect("seed transcript");

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
    let mut job = test_job(
        job_id,
        "2026-01-01T00:00:00Z",
        "sources/job-stt-missing/source.wav",
        "hash-stt-missing",
        "stt-missing.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut job,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark completed");
    store.insert_job(job).expect("insert job");

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
async fn get_summary_text_returns_result_md_as_json() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let job_id = "job-summary-text";
    let mut job = test_job(
        job_id,
        "2026-01-01T00:00:00Z",
        "sources/job-summary-text/source.wav",
        "hash-summary-text",
        "summary.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut job,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark completed");
    store.insert_job(job).expect("insert job");
    seed_summary(&store, job_id, "summary body").expect("seed summary");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/jobs/job-summary-text/summary/text"))
        .await
        .expect("summary text response");

    assert_eq!(response.status(), StatusCode::OK);
    let body: SummaryTextResponse = read_json(response).await;
    assert_eq!(body.job_id, job_id);
    assert_eq!(body.file_name, "result.md");
    assert_eq!(body.text, "summary body");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_summary_text_returns_404_when_result_md_missing() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let job_id = "job-summary-missing";
    let mut job = test_job(
        job_id,
        "2026-01-01T00:00:00Z",
        "sources/job-summary-missing/source.wav",
        "hash-summary-missing",
        "summary-missing.wav",
    );
    mark_job_completed_with_audio(
        &store,
        &mut job,
        "2026-01-01T00:00:01Z",
        &["mono_mix.wav"],
    )
    .expect("mark completed");
    store.insert_job(job).expect("insert job");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(get_request("/jobs/job-summary-missing/summary/text"))
        .await
        .expect("summary text missing response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: ErrorResponse = read_json(response).await;
    assert!(body.message.contains("summary not found for job:"));
}

#[tokio::test(flavor = "multi_thread")]
async fn post_summary_rejects_request_before_stt_completion() {
    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let job_id = "job-summary-before-stt";

    store
        .insert_job(test_job(
            job_id,
            "2026-01-01T00:00:00Z",
            "sources/job-summary-before-stt/source.wav",
            "hash-summary-before-stt",
            "summary-before-stt.wav",
        ))
        .expect("insert job");

    let app = router_with_repo_root(repo_root);
    let response = app
        .oneshot(post_json_request(
            &format!("/jobs/{job_id}/summary"),
            &serde_json::json!({ "force_regenerate": true }),
        ))
        .await
        .expect("summary submit response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: ErrorResponse = read_json(response).await;
    assert!(
        body.message
            .contains("stt must be completed before summary")
    );
}
