use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use chrono::Utc;
use record_route_api::api;
use record_route_api::config::AppConfig;
use record_route_api::models::{
    Job, NewRecording, ProcessingStatus, ProcessingStep, Recording, RecordingBundle,
    RecordingListItem, SearchResult, StructuredSummary,
};
use record_route_api::pipeline::PipelineProcessor;
use record_route_api::sidecar_clients::{EmbeddingClient, SummaryClient, TranscriptionClient, TranscriptionResult};
use record_route_api::storage::{LocalFileStore, RecordingRepository};
use record_route_api::AppState;
use tempfile::tempdir;
use tokio::sync::{Mutex, Notify};
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn upload_and_query_endpoints_work_with_mock_repository() {
    let temp = tempdir().unwrap();
    let file_store = Arc::new(LocalFileStore::new(temp.path().to_path_buf()));
    file_store.ensure_root().await.unwrap();

    let repo = Arc::new(MockRepository::default());
    let repo_dyn: Arc<dyn RecordingRepository> = repo.clone();
    let state = AppState {
        config: Arc::new(test_config(temp.path().to_path_buf())),
        repo: repo_dyn,
        file_store: Arc::clone(&file_store),
        transcription_client: Arc::new(FixedTranscriptionClient::success("unused")),
        summary_client: Arc::new(FixedSummaryClient),
        embedding_client: Arc::new(FixedEmbeddingClient::success(vec![0.9, 0.1])),
        worker_notify: Arc::new(Notify::new()),
    };

    let app = api::router(state.clone());
    let boundary = "test-boundary";
    let wav = tiny_wav_bytes();
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"sample.wav\"\r\n");
    body.extend_from_slice(b"Content-Type: audio/wav\r\n\r\n");
    body.extend_from_slice(&wav);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/recordings")
                .header("content-type", format!("multipart/form-data; boundary={boundary}"))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let recording_id = Uuid::parse_str(payload.get("recording_id").unwrap().as_str().unwrap()).unwrap();
    let job_id = Uuid::parse_str(payload.get("job_id").unwrap().as_str().unwrap()).unwrap();

    let response = app
        .clone()
        .oneshot(Request::builder().uri(format!("/v1/jobs/{job_id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .clone()
        .oneshot(Request::builder().uri("/v1/recordings").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let list_payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(list_payload.get("items").unwrap().as_array().unwrap().len(), 1);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/recordings/{recording_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    seed_searchable_recording(repo.as_ref(), recording_id).await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/search?query=meeting")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let search_payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert!(!search_payload.get("items").unwrap().as_array().unwrap().is_empty());
}

#[tokio::test]
async fn pipeline_processes_recording_and_marks_job_completed() {
    let temp = tempdir().unwrap();
    let file_store = Arc::new(LocalFileStore::new(temp.path().to_path_buf()));
    file_store.ensure_root().await.unwrap();
    let repo = Arc::new(MockRepository::default());

    let recording_id = Uuid::new_v4();
    let original_dir = temp.path().join(recording_id.to_string()).join("original");
    tokio::fs::create_dir_all(&original_dir).await.unwrap();
    let original_path = original_dir.join("input.wav");
    let wav = tiny_wav_bytes();
    tokio::fs::write(&original_path, &wav).await.unwrap();

    let (_, job) = repo
        .insert_recording_with_job(NewRecording {
            id: recording_id,
            original_filename: "input.wav".to_string(),
            original_content_type: Some("audio/wav".to_string()),
            file_size_bytes: wav.len() as i64,
            original_rel_path: format!("{recording_id}/original/input.wav"),
        })
        .await
        .unwrap();

    let processor = PipelineProcessor::new(
        repo.clone(),
        file_store.clone(),
        Arc::new(FixedTranscriptionClient::success("회의 내용을 정리해 주세요.")),
        Arc::new(FixedSummaryClient),
        Arc::new(FixedEmbeddingClient::success(vec![0.2, 0.8, 0.1])),
        "ffmpeg".to_string(),
    );

    processor.process_job(job.clone()).await.unwrap();

    let bundle = repo.get_recording_bundle(recording_id).await.unwrap().unwrap();
    assert_eq!(bundle.recording.status, ProcessingStatus::Completed);
    assert_eq!(bundle.recording.current_step, ProcessingStep::Embedded);
    assert!(bundle.recording.transcript.as_deref().unwrap().contains("회의"));
    assert!(bundle.recording.summary.is_some());
    assert_eq!(bundle.job.unwrap().status, ProcessingStatus::Completed);
    assert!(temp.path().join(recording_id.to_string()).join("wav").join("standard.wav").exists());
}

#[tokio::test]
async fn search_falls_back_to_keyword_only_when_embedding_client_fails() {
    let temp = tempdir().unwrap();
    let file_store = Arc::new(LocalFileStore::new(temp.path().to_path_buf()));
    file_store.ensure_root().await.unwrap();
    let repo = Arc::new(MockRepository::default());

    let recording_id = Uuid::new_v4();
    repo.insert_recording_with_job(NewRecording {
        id: recording_id,
        original_filename: "meeting.wav".to_string(),
        original_content_type: Some("audio/wav".to_string()),
        file_size_bytes: 10,
        original_rel_path: format!("{recording_id}/original/meeting.wav"),
    })
    .await
    .unwrap();
    let job = repo.get_job_by_recording(recording_id).await.unwrap();
    repo.save_transcription(recording_id, job.id, Some("ko"), "meeting transcript")
        .await
        .unwrap();

    let state = AppState {
        config: Arc::new(test_config(temp.path().to_path_buf())),
        repo: repo.clone(),
        file_store,
        transcription_client: Arc::new(FixedTranscriptionClient::success("unused")),
        summary_client: Arc::new(FixedSummaryClient),
        embedding_client: Arc::new(FixedEmbeddingClient::failure("embedding down")),
        worker_notify: Arc::new(Notify::new()),
    };

    let app = api::router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/search?query=meeting")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload.get("items").unwrap().as_array().unwrap().len(), 1);
}

fn test_config(storage_root: std::path::PathBuf) -> AppConfig {
    AppConfig {
        bind_addr: "127.0.0.1:3000".parse().unwrap(),
        app_db_path: storage_root.join("record-route.db"),
        app_storage_root: storage_root,
        ffmpeg_bin: "ffmpeg".to_string(),
        whisper_base_url: "http://127.0.0.1:8080".to_string(),
        llama_summary_base_url: "http://127.0.0.1:8081".to_string(),
        llama_embed_base_url: "http://127.0.0.1:8082".to_string(),
        whisper_model: "unused".to_string(),
        summary_model: "summary-model".to_string(),
        embed_model: "embedding-model".to_string(),
        sidecar_timeout: Duration::from_secs(5),
        worker_poll_interval: Duration::from_millis(50),
        max_upload_size_bytes: 1024 * 1024,
        max_job_attempts: 3,
        job_retry_backoff: Duration::from_secs(1),
        max_search_limit: 20,
    }
}

fn tiny_wav_bytes() -> Vec<u8> {
    let sample_rate = 16_000u32;
    let channels = 1u16;
    let bits_per_sample = 16u16;
    let samples = [0i16; 32];
    let data_len = (samples.len() * std::mem::size_of::<i16>()) as u32;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align = channels * bits_per_sample / 8;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

async fn seed_searchable_recording(repo: &MockRepository, recording_id: Uuid) {
    let job = repo.get_job_by_recording(recording_id).await.unwrap();
    repo.save_transcription(recording_id, job.id, Some("ko"), "meeting transcript")
        .await
        .unwrap();
    let summary = StructuredSummary {
        title: "Meeting".to_string(),
        abstract_text: "A brief meeting summary".to_string(),
        bullet_points: vec!["Action item one".to_string(), "Action item two".to_string()],
    };
    repo.save_summary(recording_id, job.id, &summary, &summary.canonical_text())
        .await
        .unwrap();
    repo.save_embedding(recording_id, &[0.9, 0.1]).await.unwrap();
    repo.complete_job(recording_id, job.id).await.unwrap();
}

#[derive(Default)]
struct MockRepository {
    inner: Mutex<MockState>,
}

#[derive(Default)]
struct MockState {
    recordings: HashMap<Uuid, Recording>,
    jobs: HashMap<Uuid, Job>,
    job_by_recording: HashMap<Uuid, Uuid>,
    embeddings: HashMap<Uuid, Vec<f32>>,
}

impl MockRepository {
    async fn get_job_by_recording(&self, recording_id: Uuid) -> Result<Job> {
        let inner = self.inner.lock().await;
        let job_id = inner
            .job_by_recording
            .get(&recording_id)
            .copied()
            .ok_or_else(|| anyhow!("missing job for recording"))?;
        inner.jobs
            .get(&job_id)
            .cloned()
            .ok_or_else(|| anyhow!("missing job row"))
    }
}

#[async_trait]
impl RecordingRepository for MockRepository {
    async fn insert_recording_with_job(&self, new_recording: NewRecording) -> Result<(Recording, Job)> {
        let now = Utc::now();
        let recording = Recording {
            id: new_recording.id,
            original_filename: new_recording.original_filename,
            original_content_type: new_recording.original_content_type,
            file_size_bytes: new_recording.file_size_bytes,
            original_rel_path: new_recording.original_rel_path,
            wav_rel_path: None,
            language: None,
            transcript: None,
            summary: None,
            summary_canonical_text: None,
            status: ProcessingStatus::Queued,
            current_step: ProcessingStep::UploadSaved,
            last_error: None,
            created_at: now,
            updated_at: now,
        };
        let job = Job {
            id: Uuid::new_v4(),
            recording_id: new_recording.id,
            status: ProcessingStatus::Queued,
            step: ProcessingStep::UploadSaved,
            attempt_count: 0,
            last_error: None,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            next_attempt_at: now,
        };

        let mut inner = self.inner.lock().await;
        inner.recordings.insert(recording.id, recording.clone());
        inner.job_by_recording.insert(recording.id, job.id);
        inner.jobs.insert(job.id, job.clone());
        Ok((recording, job))
    }

    async fn get_job(&self, job_id: Uuid) -> Result<Option<Job>> {
        Ok(self.inner.lock().await.jobs.get(&job_id).cloned())
    }

    async fn get_recording(&self, recording_id: Uuid) -> Result<Option<Recording>> {
        Ok(self.inner.lock().await.recordings.get(&recording_id).cloned())
    }

    async fn get_recording_bundle(&self, recording_id: Uuid) -> Result<Option<RecordingBundle>> {
        let inner = self.inner.lock().await;
        let Some(recording) = inner.recordings.get(&recording_id).cloned() else {
            return Ok(None);
        };
        let job = inner
            .job_by_recording
            .get(&recording_id)
            .and_then(|job_id| inner.jobs.get(job_id))
            .cloned();
        Ok(Some(RecordingBundle { recording, job }))
    }

    async fn list_recordings(&self, status: Option<ProcessingStatus>) -> Result<Vec<RecordingListItem>> {
        let mut items = self
            .inner
            .lock()
            .await
            .recordings
            .values()
            .filter(|recording| status.map(|status| recording.status == status).unwrap_or(true))
            .map(|recording| RecordingListItem {
                id: recording.id,
                original_filename: recording.original_filename.clone(),
                status: recording.status,
                current_step: recording.current_step,
                language: recording.language.clone(),
                has_transcript: recording.transcript.is_some(),
                has_summary: recording.summary.is_some(),
                last_error: recording.last_error.clone(),
                created_at: recording.created_at,
                updated_at: recording.updated_at,
            })
            .collect::<Vec<_>>();
        items.sort_by_key(|item| std::cmp::Reverse(item.created_at));
        Ok(items)
    }

    async fn claim_next_job(&self) -> Result<Option<Job>> {
        let mut inner = self.inner.lock().await;
        let now = Utc::now();
        let Some(job_id) = inner
            .jobs
            .values()
            .filter(|job| job.status == ProcessingStatus::Queued && job.next_attempt_at <= now)
            .min_by_key(|job| job.created_at)
            .map(|job| job.id)
        else {
            return Ok(None);
        };

        let recording_id;
        {
            let job = inner.jobs.get_mut(&job_id).unwrap();
            job.status = ProcessingStatus::Processing;
            job.updated_at = now;
            job.started_at = Some(job.started_at.unwrap_or(now));
            job.attempt_count += 1;
            recording_id = job.recording_id;
        }

        if let Some(recording) = inner.recordings.get_mut(&recording_id) {
            recording.status = ProcessingStatus::Processing;
            recording.updated_at = now;
        }

        Ok(inner.jobs.get(&job_id).cloned())
    }

    async fn mark_wav_ready(&self, recording_id: Uuid, job_id: Uuid, wav_rel_path: &str) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let now = Utc::now();
        if let Some(recording) = inner.recordings.get_mut(&recording_id) {
            recording.wav_rel_path = Some(wav_rel_path.to_string());
            recording.current_step = ProcessingStep::WavReady;
            recording.updated_at = now;
        }
        if let Some(job) = inner.jobs.get_mut(&job_id) {
            job.step = ProcessingStep::WavReady;
            job.updated_at = now;
        }
        Ok(())
    }

    async fn save_transcription(&self, recording_id: Uuid, job_id: Uuid, language: Option<&str>, transcript: &str) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let now = Utc::now();
        if let Some(recording) = inner.recordings.get_mut(&recording_id) {
            recording.language = language.map(str::to_string);
            recording.transcript = Some(transcript.to_string());
            recording.current_step = ProcessingStep::Transcribed;
            recording.updated_at = now;
        }
        if let Some(job) = inner.jobs.get_mut(&job_id) {
            job.step = ProcessingStep::Transcribed;
            job.updated_at = now;
        }
        Ok(())
    }

    async fn save_summary(&self, recording_id: Uuid, job_id: Uuid, summary: &StructuredSummary, canonical_text: &str) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let now = Utc::now();
        if let Some(recording) = inner.recordings.get_mut(&recording_id) {
            recording.summary = Some(summary.clone());
            recording.summary_canonical_text = Some(canonical_text.to_string());
            recording.current_step = ProcessingStep::Summarized;
            recording.updated_at = now;
        }
        if let Some(job) = inner.jobs.get_mut(&job_id) {
            job.step = ProcessingStep::Summarized;
            job.updated_at = now;
        }
        Ok(())
    }

    async fn save_embedding(&self, recording_id: Uuid, embedding: &[f32]) -> Result<()> {
        self.inner.lock().await.embeddings.insert(recording_id, embedding.to_vec());
        Ok(())
    }

    async fn complete_job(&self, recording_id: Uuid, job_id: Uuid) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let now = Utc::now();
        if let Some(recording) = inner.recordings.get_mut(&recording_id) {
            recording.status = ProcessingStatus::Completed;
            recording.current_step = ProcessingStep::Embedded;
            recording.updated_at = now;
        }
        if let Some(job) = inner.jobs.get_mut(&job_id) {
            job.status = ProcessingStatus::Completed;
            job.step = ProcessingStep::Embedded;
            job.updated_at = now;
            job.completed_at = Some(now);
        }
        Ok(())
    }

    async fn reschedule_or_fail_job(
        &self,
        job: &Job,
        step: ProcessingStep,
        error: &str,
        max_attempts: i32,
        retry_backoff: Duration,
    ) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let now = Utc::now();
        let status = if job.attempt_count >= max_attempts {
            ProcessingStatus::Failed
        } else {
            ProcessingStatus::Queued
        };

        if let Some(recording) = inner.recordings.get_mut(&job.recording_id) {
            recording.status = status;
            recording.current_step = step;
            recording.last_error = Some(error.to_string());
            recording.updated_at = now;
        }
        if let Some(stored_job) = inner.jobs.get_mut(&job.id) {
            stored_job.status = status;
            stored_job.step = step;
            stored_job.last_error = Some(error.to_string());
            stored_job.updated_at = now;
            stored_job.next_attempt_at = now + chrono::Duration::from_std(retry_backoff).unwrap();
        }
        Ok(())
    }

    async fn search_recordings(&self, query: &str, query_embedding: Option<&[f32]>, limit: i64) -> Result<Vec<SearchResult>> {
        let query_lower = query.to_lowercase();
        let inner = self.inner.lock().await;
        let mut items = inner
            .recordings
            .values()
            .filter_map(|recording| {
                let haystack = format!(
                    "{} {} {}",
                    recording.original_filename,
                    recording.summary_canonical_text.clone().unwrap_or_default(),
                    recording.transcript.clone().unwrap_or_default()
                )
                .to_lowercase();
                let keyword_hit = haystack.contains(&query_lower);
                let similarity_score = query_embedding.and_then(|query_embedding| {
                    inner
                        .embeddings
                        .get(&recording.id)
                        .map(|embedding| cosine_similarity(query_embedding, embedding))
                });
                if !keyword_hit && similarity_score.is_none() {
                    return None;
                }
                Some(SearchResult {
                    id: recording.id,
                    original_filename: recording.original_filename.clone(),
                    status: recording.status,
                    current_step: recording.current_step,
                    summary_excerpt: recording.summary_canonical_text.clone(),
                    transcript_excerpt: recording.transcript.clone(),
                    keyword_hit,
                    keyword_score: if keyword_hit { 1.0 } else { 0.0 },
                    similarity_score,
                    created_at: recording.created_at,
                })
            })
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            right
                .keyword_hit
                .cmp(&left.keyword_hit)
                .then_with(|| right.keyword_score.partial_cmp(&left.keyword_score).unwrap())
                .then_with(|| {
                    right
                        .similarity_score
                        .unwrap_or(-1.0)
                        .partial_cmp(&left.similarity_score.unwrap_or(-1.0))
                        .unwrap()
                })
        });
        items.truncate(limit as usize);
        Ok(items)
    }
}

struct FixedTranscriptionClient {
    result: std::result::Result<String, String>,
}

impl FixedTranscriptionClient {
    fn success(text: &str) -> Self {
        Self {
            result: Ok(text.to_string()),
        }
    }
}

#[async_trait]
impl TranscriptionClient for FixedTranscriptionClient {
    async fn transcribe(&self, _wav_bytes: Vec<u8>) -> Result<TranscriptionResult> {
        let text = self.result.clone().map_err(|error| anyhow!(error))?;
        Ok(TranscriptionResult {
            text: text.clone(),
            language: Some("ko".to_string()),
            raw_response: serde_json::json!({ "text": text }),
        })
    }
}

struct FixedSummaryClient;

#[async_trait]
impl SummaryClient for FixedSummaryClient {
    async fn summarize(&self, transcript: &str) -> Result<StructuredSummary> {
        Ok(StructuredSummary {
            title: "Summary".to_string(),
            abstract_text: format!("Transcript: {transcript}"),
            bullet_points: vec![
                "Point one".to_string(),
                "Point two".to_string(),
                "Point three".to_string(),
            ],
        })
    }
}

struct FixedEmbeddingClient {
    result: std::result::Result<Vec<f32>, String>,
}

impl FixedEmbeddingClient {
    fn success(vector: Vec<f32>) -> Self {
        Self { result: Ok(vector) }
    }

    fn failure(message: &str) -> Self {
        Self {
            result: Err(message.to_string()),
        }
    }
}

#[async_trait]
impl EmbeddingClient for FixedEmbeddingClient {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        self.result.clone().map_err(|error| anyhow!(error))
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let dot = left.iter().zip(right.iter()).map(|(a, b)| a * b).sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm * right_norm)
    }
}

