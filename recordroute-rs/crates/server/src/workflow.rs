use recordroute_common::{AppConfig, Result};
use recordroute_llm::Summarizer;
use recordroute_stt::{TranscriptionOptions, WhisperEngine};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::history::HistoryManager;
use crate::job_manager::JobManager;

/// Workflow execution steps
#[derive(Debug, Clone, Copy)]
pub enum WorkflowStep {
    AudioExtraction,
    Transcription,
    Summarization,
    Embedding,
}

/// Workflow execution options
#[derive(Debug, Clone)]
pub struct WorkflowOptions {
    pub run_stt: bool,
    pub run_summarize: bool,
    pub run_embed: bool,
    pub stt_model: Option<String>,
    pub summary_model: Option<String>,
    pub language: Option<String>,
}

/// Workflow execution result
#[derive(Debug)]
pub struct WorkflowResult {
    pub transcript_path: Option<PathBuf>,
    pub summary_path: Option<PathBuf>,
    pub embedding_id: Option<String>,
}

/// Workflow executor that orchestrates STT, LLM, and embedding tasks
pub struct WorkflowExecutor {
    whisper: Arc<WhisperEngine>,
    summarizer: Arc<Summarizer>,
    config: AppConfig,
    history: Arc<RwLock<HistoryManager>>,
    job_manager: Arc<JobManager>,
}

impl WorkflowExecutor {
    /// Create new workflow executor
    pub fn new(
        whisper: Arc<WhisperEngine>,
        summarizer: Arc<Summarizer>,
        config: AppConfig,
        history: Arc<RwLock<HistoryManager>>,
        job_manager: Arc<JobManager>,
    ) -> Self {
        Self {
            whisper,
            summarizer,
            config,
            history,
            job_manager,
        }
    }

    /// Execute workflow for a file
    pub async fn execute(
        &self,
        file_uuid: &str,
        file_path: &Path,
        options: WorkflowOptions,
        task_id: &str,
    ) -> Result<WorkflowResult> {
        let mut result = WorkflowResult {
            transcript_path: None,
            summary_path: None,
            embedding_id: None,
        };

        // Phase 1: STT (Speech-to-Text)
        if options.run_stt {
            info!("Starting STT workflow for file: {}", file_uuid);
            self.job_manager
                .update_progress(task_id, 10, "Starting transcription...".to_string())
                .await;

            let transcript_path = self.run_stt_workflow(file_uuid, file_path, &options, task_id).await?;
            result.transcript_path = Some(transcript_path.clone());

            // Update history
            self.history.write().await.update_record(file_uuid, |record| {
                record.stt_done = true;
                record.stt_path = Some(transcript_path.to_string_lossy().to_string());
            })?;

            self.job_manager
                .update_progress(task_id, 50, "Transcription completed".to_string())
                .await;
        }

        // Phase 2: Summarization
        if options.run_summarize {
            info!("Starting summarization workflow for file: {}", file_uuid);

            // Need transcript text
            let transcript_text = if let Some(ref transcript_path) = result.transcript_path {
                tokio::fs::read_to_string(transcript_path).await?
            } else {
                // Try to load existing transcript
                let transcript_file = self.config.db_base_path
                    .join("whisper_output")
                    .join(format!("{}.txt", file_uuid));

                if transcript_file.exists() {
                    tokio::fs::read_to_string(&transcript_file).await?
                } else {
                    warn!("No transcript found for summarization");
                    self.job_manager
                        .update_progress(task_id, 70, "Summarization skipped (no transcript)".to_string())
                        .await;
                    String::new()
                }
            };

            if !transcript_text.is_empty() {
                let summary_path = self
                    .run_summarize_workflow(file_uuid, &transcript_text, &options, task_id)
                    .await?;
                result.summary_path = Some(summary_path.clone());

                // Update history
                self.history.write().await.update_record(file_uuid, |record| {
                    record.summarize_done = true;
                    record.summary_path = Some(summary_path.to_string_lossy().to_string());
                })?;

                self.job_manager
                    .update_progress(task_id, 80, "Summarization completed".to_string())
                    .await;
            }
        }

        // Phase 3: Embedding (TODO: Implement when vector module is ready)
        if options.run_embed {
            warn!("Embedding not yet implemented (Phase 4)");
            self.job_manager
                .update_progress(task_id, 90, "Embedding skipped (not implemented)".to_string())
                .await;
        }

        self.job_manager
            .update_progress(task_id, 100, "Workflow completed".to_string())
            .await;

        Ok(result)
    }

    /// Execute STT workflow
    async fn run_stt_workflow(
        &self,
        file_uuid: &str,
        file_path: &Path,
        options: &WorkflowOptions,
        task_id: &str,
    ) -> Result<PathBuf> {
        // Prepare transcription options
        let stt_options = TranscriptionOptions {
            language: options.language.clone(),
            initial_prompt: None,
            temperature: 0.0,
            filter_fillers: true,
            min_segment_length: 3,
            normalize_punctuation: true,
            ..Default::default()
        };

        self.job_manager
            .update_progress(task_id, 20, "Loading audio file...".to_string())
            .await;

        // Run transcription
        info!("Transcribing file: {:?}", file_path);
        let transcript = self.whisper.transcribe(file_path, &stt_options)?;

        self.job_manager
            .update_progress(task_id, 40, "Writing transcription results...".to_string())
            .await;

        // Save transcript to file
        let output_dir = self.config.db_base_path.join("whisper_output");
        tokio::fs::create_dir_all(&output_dir).await?;

        let transcript_file = output_dir.join(format!("{}.txt", file_uuid));
        tokio::fs::write(&transcript_file, &transcript.text).await?;

        // Save segments as JSON
        let segments_file = output_dir.join(format!("{}_segments.json", file_uuid));
        let segments_json = serde_json::to_string_pretty(&transcript.segments)?;
        tokio::fs::write(&segments_file, segments_json).await?;

        info!(
            "Transcription saved to: {:?}, Language: {}",
            transcript_file, transcript.language
        );

        Ok(transcript_file)
    }

    /// Execute summarization workflow
    async fn run_summarize_workflow(
        &self,
        file_uuid: &str,
        transcript_text: &str,
        _options: &WorkflowOptions,
        task_id: &str,
    ) -> Result<PathBuf> {
        self.job_manager
            .update_progress(task_id, 60, "Generating summary...".to_string())
            .await;

        // Run summarization
        info!("Summarizing transcript - Length: {} chars", transcript_text.len());
        let summary = self.summarizer.summarize(transcript_text).await?;

        self.job_manager
            .update_progress(task_id, 75, "Writing summary...".to_string())
            .await;

        // Save summary to file
        let output_dir = self.config.db_base_path.join("whisper_output");
        tokio::fs::create_dir_all(&output_dir).await?;

        let summary_file = output_dir.join(format!("{}_summary.txt", file_uuid));
        tokio::fs::write(&summary_file, &summary.text).await?;

        // Save one-line summary separately
        let oneline_file = output_dir.join(format!("{}_oneline.txt", file_uuid));
        tokio::fs::write(&oneline_file, &summary.one_line).await?;

        // Update history with one-line summary
        self.history.write().await.update_record(file_uuid, |record| {
            record.one_line_summary = Some(summary.one_line.clone());
        })?;

        info!(
            "Summary saved to: {:?}, One-line: {}",
            summary_file, summary.one_line
        );

        Ok(summary_file)
    }
}
