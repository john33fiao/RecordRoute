use recordroute_common::{AppConfig, RecordRouteError, Result};
use recordroute_llm::Summarizer;
use recordroute_stt::{TranscriptionOptions, WhisperEngine};
use recordroute_vector::{VectorMetadata, VectorSearchEngine};
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
    vector_search: Arc<VectorSearchEngine>,
    config: AppConfig,
    history: Arc<RwLock<HistoryManager>>,
    job_manager: Arc<JobManager>,
}

impl WorkflowExecutor {
    /// Create new workflow executor
    pub fn new(
        whisper: Arc<WhisperEngine>,
        summarizer: Arc<Summarizer>,
        vector_search: Arc<VectorSearchEngine>,
        config: AppConfig,
        history: Arc<RwLock<HistoryManager>>,
        job_manager: Arc<JobManager>,
    ) -> Self {
        Self {
            whisper,
            summarizer,
            vector_search,
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

        // Phase 3: Embedding
        if options.run_embed {
            info!("Starting embedding workflow for file: {}", file_uuid);

            // Need transcript or summary text
            let text_to_embed = if let Some(ref summary_path) = result.summary_path {
                tokio::fs::read_to_string(summary_path).await?
            } else if let Some(ref transcript_path) = result.transcript_path {
                tokio::fs::read_to_string(transcript_path).await?
            } else {
                // Try to load existing files
                let summary_file = self.config.db_base_path
                    .join("whisper_output")
                    .join(format!("{}_summary.txt", file_uuid));

                let transcript_file = self.config.db_base_path
                    .join("whisper_output")
                    .join(format!("{}.txt", file_uuid));

                if summary_file.exists() {
                    tokio::fs::read_to_string(&summary_file).await?
                } else if transcript_file.exists() {
                    tokio::fs::read_to_string(&transcript_file).await?
                } else {
                    warn!("No text found for embedding");
                    self.job_manager
                        .update_progress(task_id, 90, "Embedding skipped (no text)".to_string())
                        .await;
                    String::new()
                }
            };

            if !text_to_embed.is_empty() {
                let embedding_id = self
                    .run_embed_workflow(file_uuid, &text_to_embed, file_path, &options, task_id)
                    .await?;
                result.embedding_id = Some(embedding_id);

                // Update history
                self.history.write().await.update_record(file_uuid, |record| {
                    record.embed_done = true;
                })?;

                self.job_manager
                    .update_progress(task_id, 95, "Embedding completed".to_string())
                    .await;
            }
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

        // Run transcription in a blocking task (Whisper is CPU-intensive)
        info!("Transcribing file: {:?}", file_path);
        let whisper = Arc::clone(&self.whisper);
        let file_path_clone = file_path.to_path_buf();
        let stt_options_clone = stt_options.clone();

        let transcript = tokio::task::spawn_blocking(move || {
            whisper.transcribe(&file_path_clone, &stt_options_clone)
        })
        .await
        .map_err(|e| RecordRouteError::stt(format!("Transcription task panicked: {}", e)))??;

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

    /// Execute embedding workflow
    async fn run_embed_workflow(
        &self,
        file_uuid: &str,
        text: &str,
        file_path: &Path,
        _options: &WorkflowOptions,
        task_id: &str,
    ) -> Result<String> {
        self.job_manager
            .update_progress(task_id, 85, "Generating embedding...".to_string())
            .await;

        // Get filename and paths for metadata
        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let transcript_path = self.config.db_base_path
            .join("whisper_output")
            .join(format!("{}.txt", file_uuid));

        let summary_path = self.config.db_base_path
            .join("whisper_output")
            .join(format!("{}_summary.txt", file_uuid));

        let one_line_file = self.config.db_base_path
            .join("whisper_output")
            .join(format!("{}_oneline.txt", file_uuid));

        let one_line_summary = if one_line_file.exists() {
            Some(tokio::fs::read_to_string(&one_line_file).await?)
        } else {
            None
        };

        // Create metadata
        let metadata = VectorMetadata {
            filename,
            file_path: file_path.to_string_lossy().to_string(),
            transcript_path: if transcript_path.exists() {
                Some(transcript_path.to_string_lossy().to_string())
            } else {
                None
            },
            summary_path: if summary_path.exists() {
                Some(summary_path.to_string_lossy().to_string())
            } else {
                None
            },
            one_line_summary,
            timestamp: Some(chrono::Utc::now()),
            tags: Vec::new(),
        };

        // Add to vector index
        self.vector_search
            .add_document(file_uuid, text, metadata)
            .await?;

        info!("Embedding added to vector index: {}", file_uuid);

        Ok(file_uuid.to_string())
    }
}
