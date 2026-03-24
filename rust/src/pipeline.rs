use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::json;
use tokio::fs;
use tokio::process::Command;

use crate::models::{Job, ProcessingStep};
use crate::sidecar_clients::{EmbeddingClient, SummaryClient, TranscriptionClient};
use crate::storage::{LocalFileStore, RecordingRepository};

#[derive(Debug)]
pub struct PipelineError {
    pub step: ProcessingStep,
    pub source: anyhow::Error,
}

impl PipelineError {
    fn new(step: ProcessingStep, source: anyhow::Error) -> Self {
        Self { step, source }
    }
}

pub struct PipelineProcessor {
    repo: Arc<dyn RecordingRepository>,
    file_store: Arc<LocalFileStore>,
    transcription_client: Arc<dyn TranscriptionClient>,
    summary_client: Arc<dyn SummaryClient>,
    embedding_client: Arc<dyn EmbeddingClient>,
    ffmpeg_bin: String,
}

impl PipelineProcessor {
    pub fn new(
        repo: Arc<dyn RecordingRepository>,
        file_store: Arc<LocalFileStore>,
        transcription_client: Arc<dyn TranscriptionClient>,
        summary_client: Arc<dyn SummaryClient>,
        embedding_client: Arc<dyn EmbeddingClient>,
        ffmpeg_bin: String,
    ) -> Self {
        Self {
            repo,
            file_store,
            transcription_client,
            summary_client,
            embedding_client,
            ffmpeg_bin,
        }
    }

    pub async fn process_job(&self, job: Job) -> Result<(), PipelineError> {
        let recording = self
            .repo
            .get_recording(job.recording_id)
            .await
            .map_err(|error| PipelineError::new(ProcessingStep::UploadSaved, error))?
            .ok_or_else(|| PipelineError::new(ProcessingStep::UploadSaved, anyhow!("recording not found")))?;

        let original_path = self.file_store.absolute_path_from_rel(&recording.original_rel_path);
        if !fs::try_exists(&original_path)
            .await
            .map_err(|error| PipelineError::new(ProcessingStep::UploadSaved, error.into()))?
        {
            return Err(PipelineError::new(
                ProcessingStep::UploadSaved,
                anyhow!("original upload is missing at {}", original_path.display()),
            ));
        }

        let wav_path = self.file_store.wav_abs_path(recording.id);
        let wav_rel_path = self
            .file_store
            .relative_path(&wav_path)
            .map_err(|error| PipelineError::new(ProcessingStep::WavReady, error))?;

        let wav_ready = if let Some(existing) = recording.wav_rel_path.as_deref() {
            let existing_abs = self.file_store.absolute_path_from_rel(existing);
            fs::try_exists(existing_abs)
                .await
                .map_err(|error| PipelineError::new(ProcessingStep::WavReady, error.into()))?
        } else {
            false
        };

        if !wav_ready {
            self.convert_to_wav(&original_path, &wav_path)
                .await
                .map_err(|error| PipelineError::new(ProcessingStep::WavReady, error))?;
            self.repo
                .mark_wav_ready(recording.id, job.id, &wav_rel_path)
                .await
                .map_err(|error| PipelineError::new(ProcessingStep::WavReady, error))?;
        }

        let transcript = if let Some(existing) = recording.transcript.clone() {
            existing
        } else {
            let wav_bytes = fs::read(&wav_path)
                .await
                .with_context(|| format!("failed to read wav file {}", wav_path.display()))
                .map_err(|error| PipelineError::new(ProcessingStep::Transcribed, error))?;
            let result = self
                .transcription_client
                .transcribe(wav_bytes)
                .await
                .map_err(|error| PipelineError::new(ProcessingStep::Transcribed, error))?;
            self.file_store
                .write_json_artifact(recording.id, "whisper-vjson.json", &result.raw_response)
                .await
                .map_err(|error| PipelineError::new(ProcessingStep::Transcribed, error))?;
            self.repo
                .save_transcription(recording.id, job.id, result.language.as_deref(), &result.text)
                .await
                .map_err(|error| PipelineError::new(ProcessingStep::Transcribed, error))?;
            result.text
        };

        let (summary, canonical_text) = if let (Some(summary), Some(canonical_text)) = (
            recording.summary.clone(),
            recording.summary_canonical_text.clone(),
        ) {
            (summary, canonical_text)
        } else {
            let summary = self
                .summary_client
                .summarize(&transcript)
                .await
                .map_err(|error| PipelineError::new(ProcessingStep::Summarized, error))?;
            let canonical_text = summary.canonical_text();
            self.file_store
                .write_json_artifact(recording.id, "summary.json", &json!(&summary))
                .await
                .map_err(|error| PipelineError::new(ProcessingStep::Summarized, error))?;
            self.repo
                .save_summary(recording.id, job.id, &summary, &canonical_text)
                .await
                .map_err(|error| PipelineError::new(ProcessingStep::Summarized, error))?;
            (summary, canonical_text)
        };

        let embedding = self
            .embedding_client
            .embed(&canonical_text)
            .await
            .map_err(|error| PipelineError::new(ProcessingStep::Embedded, error))?;
        self.file_store
            .write_json_artifact(
                recording.id,
                "embedding.json",
                &json!({
                    "dimensions": embedding.len(),
                    "summary_title": summary.title.clone(),
                    "embedding": embedding,
                }),
            )
            .await
            .map_err(|error| PipelineError::new(ProcessingStep::Embedded, error))?;
        self.repo
            .save_embedding(recording.id, &embedding)
            .await
            .map_err(|error| PipelineError::new(ProcessingStep::Embedded, error))?;
        self.repo
            .complete_job(recording.id, job.id)
            .await
            .map_err(|error| PipelineError::new(ProcessingStep::Embedded, error))?;

        Ok(())
    }

    async fn convert_to_wav(&self, input_path: &Path, output_path: &Path) -> Result<()> {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let output = Command::new(&self.ffmpeg_bin)
            .arg("-y")
            .arg("-i")
            .arg(input_path)
            .arg("-vn")
            .arg("-acodec")
            .arg("pcm_s16le")
            .arg("-ar")
            .arg("16000")
            .arg("-ac")
            .arg("1")
            .arg(output_path)
            .output()
            .await
            .with_context(|| format!("failed to launch ffmpeg binary `{}`", self.ffmpeg_bin))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("ffmpeg conversion failed: {stderr}");
        }

        Ok(())
    }
}

