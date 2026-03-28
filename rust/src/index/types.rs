use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexFile {
    pub version: u32,
    #[serde(default)]
    pub model_preparations: ModelPreparations,
    #[serde(default)]
    pub task_queue: TaskQueueState,
    pub jobs: Vec<JobRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobRecord {
    pub job_id: String,
    pub status: JobStatus,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub source_ref: String,
    pub source_kind: SourceKind,
    pub source_content_sha256: String,
    pub source_file_name: String,
    pub probe: JobProbe,
    pub split_strategy: SplitStrategy,
    pub outputs: JobOutputs,
    pub error_message: Option<String>,
    #[serde(default)]
    pub summary_embedding: Option<SummaryEmbeddingRecord>,
    #[serde(default)]
    pub tasks: Vec<TaskRecord>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    LocalFile,
    Upload,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Ffmpeg,
    Stt,
    Summary,
    Embedding,
}

impl TaskType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ffmpeg => "ffmpeg",
            Self::Stt => "stt",
            Self::Summary => "summary",
            Self::Embedding => "embedding",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QueueCategory {
    Ffmpeg,
    Stt,
    Llm,
    Embed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveQueueBatch {
    pub category: QueueCategory,
    #[serde(default)]
    pub running: Option<QueueEntry>,
    #[serde(default)]
    pub entries: Vec<QueueEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueueBatch {
    pub category: QueueCategory,
    #[serde(default)]
    pub entries: Vec<QueueEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueueEntry {
    pub job_id: String,
    pub task_type: TaskType,
    pub category: QueueCategory,
    pub queued_at: String,
    pub payload: QueuePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QueuePayload {
    Ffmpeg {
        input_path: String,
    },
    Stt {
        #[serde(default)]
        audio_files: Vec<String>,
        #[serde(default = "default_stt_language")]
        language: String,
        #[serde(default)]
        keywords: Vec<String>,
    },
    Summary {
        force_regenerate: bool,
    },
    Embedding,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskQueueState {
    #[serde(default)]
    pub active_batch: Option<ActiveQueueBatch>,
    #[serde(default)]
    pub pending_batches: Vec<QueueBatch>,
    #[serde(default = "default_queue_burst_limit")]
    pub burst_limit: u32,
}

impl Default for TaskQueueState {
    fn default() -> Self {
        Self {
            active_batch: None,
            pending_batches: Vec::new(),
            burst_limit: default_queue_burst_limit(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Whisper,
    Llama,
}

impl ModelKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Whisper => "whisper",
            Self::Llama => "llama",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelPreparationStatus {
    Idle,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelPreparationRecord {
    pub status: ModelPreparationStatus,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub heartbeat_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl Default for ModelPreparationRecord {
    fn default() -> Self {
        Self {
            status: ModelPreparationStatus::Idle,
            started_at: None,
            finished_at: None,
            heartbeat_at: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ModelPreparations {
    #[serde(default)]
    pub whisper: ModelPreparationRecord,
    #[serde(default)]
    pub llama: ModelPreparationRecord,
    #[serde(default)]
    pub llama_embedding: ModelPreparationRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SummaryEmbeddingRecord {
    pub model_id: String,
    pub text_sha256: String,
    pub dimension: usize,
    pub normalized: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioArtifactRecord {
    pub job_id: String,
    pub logical_name: String,
    pub storage_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TranscriptRecord {
    pub job_id: String,
    pub transcript_id: String,
    pub file_name: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SummaryRecord {
    pub job_id: String,
    pub file_name: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SummaryEmbeddingVectorRecord {
    pub metadata: SummaryEmbeddingRecord,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskRecord {
    #[serde(default = "default_task_id")]
    pub task_id: String,
    pub task_type: TaskType,
    pub status: TaskStatus,
    #[serde(default)]
    pub queued_at: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub last_error: Option<String>,
    pub retry_count: u32,
    #[serde(default)]
    pub request_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SplitStrategy {
    PerChannelPlusMergedMono,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct JobProbe {
    pub channels: Option<u32>,
    pub channel_layout: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct JobOutputs {
    pub merged_mono_wav: Option<String>,
    #[serde(default)]
    pub split_mono_wavs: Vec<JobSplitOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobSplitOutput {
    pub channel_index: u32,
    pub path: String,
}

impl IndexFile {
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self {
            version: 4,
            model_preparations: ModelPreparations::default(),
            task_queue: TaskQueueState::default(),
            jobs: Vec::new(),
        }
    }
}

impl JobRecord {
    #[cfg(test)]
    pub fn new(
        job_id: String,
        started_at: String,
        source_path: PathBuf,
        _job_dir: PathBuf,
    ) -> Self {
        let source_file_name = source_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| source_path.to_string_lossy().into_owned());

        Self::new_with_source(
            job_id,
            started_at,
            source_path.to_string_lossy().into_owned(),
            SourceKind::LocalFile,
            String::new(),
            source_file_name,
        )
    }

    pub fn new_with_source(
        job_id: String,
        started_at: String,
        source_ref: String,
        source_kind: SourceKind,
        source_content_sha256: String,
        source_file_name: String,
    ) -> Self {
        Self {
            job_id,
            status: JobStatus::Queued,
            started_at: started_at.clone(),
            finished_at: None,
            source_ref,
            source_kind,
            source_content_sha256,
            source_file_name,
            probe: JobProbe::default(),
            split_strategy: SplitStrategy::PerChannelPlusMergedMono,
            outputs: JobOutputs::default(),
            error_message: None,
            summary_embedding: None,
            tasks: vec![TaskRecord::new_queued(TaskType::Ffmpeg, started_at)],
        }
    }

    pub fn mark_ffmpeg_running(&mut self, started_at: String) -> Result<(), String> {
        self.status = JobStatus::Running;
        self.finished_at = None;
        self.error_message = None;
        self.update_existing_task(TaskType::Ffmpeg, |task| task.mark_running(started_at))
    }

    pub fn mark_completed(
        &mut self,
        finished_at: String,
        outputs: JobOutputs,
    ) -> Result<(), String> {
        self.status = JobStatus::Completed;
        self.finished_at = Some(finished_at.clone());
        self.outputs = outputs;
        self.error_message = None;
        self.update_existing_task(TaskType::Ffmpeg, |task| task.mark_completed(finished_at))
    }

    pub fn mark_failed(
        &mut self,
        finished_at: String,
        error_message: String,
    ) -> Result<(), String> {
        self.status = JobStatus::Failed;
        self.finished_at = Some(finished_at.clone());
        self.error_message = Some(error_message.clone());
        self.update_existing_task(TaskType::Ffmpeg, |task| {
            task.mark_failed(finished_at, error_message)
        })
    }

    pub fn mark_ffmpeg_queued(&mut self, queued_at: String) {
        self.status = JobStatus::Queued;
        self.finished_at = None;
        self.error_message = None;
        self.enqueue_task(TaskType::Ffmpeg, queued_at);
    }

    pub fn enqueue_task(&mut self, task_type: TaskType, queued_at: String) {
        let retry_count = self
            .task(task_type)
            .map(|task| task.retry_count.saturating_add(1))
            .unwrap_or(0);
        self.set_task(TaskRecord::new_queued_with_retry(
            task_type,
            queued_at,
            retry_count,
        ));
    }

    pub fn set_task_request_fingerprint(
        &mut self,
        task_type: TaskType,
        request_fingerprint: Option<String>,
    ) {
        if let Some(task) = self
            .tasks
            .iter_mut()
            .find(|task| task.task_type == task_type)
        {
            task.request_fingerprint = request_fingerprint;
        }
    }

    #[cfg(test)]
    pub fn upsert_running_task(&mut self, task_type: TaskType, started_at: String) {
        let retry_count = self
            .task(task_type)
            .map(|task| task.retry_count.saturating_add(1))
            .unwrap_or(0);
        self.set_task(TaskRecord {
            task_id: build_task_id(),
            task_type,
            status: TaskStatus::Running,
            queued_at: None,
            started_at: Some(started_at),
            finished_at: None,
            last_error: None,
            retry_count,
            request_fingerprint: None,
        });
    }

    pub fn start_task(&mut self, task_type: TaskType, started_at: String) -> Result<(), String> {
        self.update_existing_task(task_type, |task| task.mark_running(started_at))
    }

    pub fn complete_task(
        &mut self,
        task_type: TaskType,
        finished_at: String,
    ) -> Result<(), String> {
        self.update_existing_task(task_type, |task| task.mark_completed(finished_at))
    }

    pub fn fail_task(
        &mut self,
        task_type: TaskType,
        finished_at: String,
        error: String,
    ) -> Result<(), String> {
        self.update_existing_task(task_type, |task| task.mark_failed(finished_at, error))
    }

    pub fn task(&self, task_type: TaskType) -> Option<&TaskRecord> {
        self.tasks.iter().find(|task| task.task_type == task_type)
    }

    fn set_task(&mut self, record: TaskRecord) {
        if let Some(existing) = self
            .tasks
            .iter_mut()
            .find(|task| task.task_type == record.task_type)
        {
            *existing = record;
        } else {
            self.tasks.push(record);
        }
    }

    fn update_existing_task(
        &mut self,
        task_type: TaskType,
        update: impl FnOnce(&mut TaskRecord),
    ) -> Result<(), String> {
        let Some(task) = self
            .tasks
            .iter_mut()
            .find(|task| task.task_type == task_type)
        else {
            return Err(format!(
                "task not found for job {}: {}",
                self.job_id,
                task_type.as_str()
            ));
        };

        update(task);
        Ok(())
    }
}

impl TaskRecord {
    pub fn new_queued(task_type: TaskType, queued_at: String) -> Self {
        Self::new_queued_with_retry(task_type, queued_at, 0)
    }

    pub fn new_queued_with_retry(task_type: TaskType, queued_at: String, retry_count: u32) -> Self {
        Self {
            task_id: build_task_id(),
            task_type,
            status: TaskStatus::Queued,
            queued_at: Some(queued_at),
            started_at: None,
            finished_at: None,
            last_error: None,
            retry_count,
            request_fingerprint: None,
        }
    }

    pub fn mark_running(&mut self, started_at: String) {
        self.status = TaskStatus::Running;
        self.started_at = Some(started_at);
        self.finished_at = None;
        self.last_error = None;
    }

    pub fn mark_completed(&mut self, finished_at: String) {
        self.status = TaskStatus::Completed;
        if self.started_at.is_none() {
            self.started_at = Some(finished_at.clone());
        }
        self.finished_at = Some(finished_at);
        self.last_error = None;
    }

    pub fn mark_failed(&mut self, finished_at: String, error: String) {
        self.status = TaskStatus::Failed;
        if self.started_at.is_none() {
            self.started_at = Some(finished_at.clone());
        }
        self.finished_at = Some(finished_at);
        self.last_error = Some(error);
    }
}

impl QueuePayload {
    pub fn request_fingerprint(&self) -> Option<String> {
        match self {
            Self::Stt {
                audio_files,
                language,
                keywords,
            } => serde_json::to_string(&(audio_files, language, keywords))
                .ok()
                .map(|raw| format!("stt:{raw}")),
            _ => None,
        }
    }
}

impl ModelPreparations {
    #[cfg(test)]
    pub fn record(&self, model: ModelKind) -> &ModelPreparationRecord {
        match model {
            ModelKind::Whisper => &self.whisper,
            ModelKind::Llama => &self.llama,
        }
    }

    pub(crate) fn record_mut(&mut self, model: ModelKind) -> &mut ModelPreparationRecord {
        match model {
            ModelKind::Whisper => &mut self.whisper,
            ModelKind::Llama => &mut self.llama,
        }
    }

    pub(crate) fn llama_embedding_mut(&mut self) -> &mut ModelPreparationRecord {
        &mut self.llama_embedding
    }
}

impl ModelPreparationRecord {
    pub fn mark_running(&mut self, started_at: String) {
        self.status = ModelPreparationStatus::Running;
        self.started_at = Some(started_at.clone());
        self.finished_at = None;
        self.heartbeat_at = Some(started_at);
        self.last_error = None;
    }

    pub fn mark_completed(&mut self, finished_at: String) {
        self.status = ModelPreparationStatus::Completed;
        if self.started_at.is_none() {
            self.started_at = Some(finished_at.clone());
        }
        self.finished_at = Some(finished_at.clone());
        self.heartbeat_at = Some(finished_at);
        self.last_error = None;
    }

    pub fn mark_failed(&mut self, finished_at: String, error: String) {
        self.status = ModelPreparationStatus::Failed;
        if self.started_at.is_none() {
            self.started_at = Some(finished_at.clone());
        }
        self.finished_at = Some(finished_at.clone());
        self.heartbeat_at = Some(finished_at);
        self.last_error = Some(error);
    }

    pub fn touch(&mut self, heartbeat_at: String) {
        if self.started_at.is_none() {
            self.started_at = Some(heartbeat_at.clone());
        }
        self.heartbeat_at = Some(heartbeat_at);
    }
}

fn build_task_id() -> String {
    Uuid::now_v7().to_string()
}

fn default_queue_burst_limit() -> u32 {
    3
}

fn default_task_id() -> String {
    String::new()
}

fn default_stt_language() -> String {
    "ko".to_string()
}
