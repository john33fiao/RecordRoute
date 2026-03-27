use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexFile {
    pub version: u32,
    #[serde(default)]
    pub model_preparations: ModelPreparations,
    pub jobs: Vec<JobRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobRecord {
    pub job_id: String,
    pub status: JobStatus,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub source_path: String,
    pub source_file_name: String,
    pub job_dir: String,
    pub probe: JobProbe,
    pub split_strategy: SplitStrategy,
    pub outputs: JobOutputs,
    pub error_message: Option<String>,
    #[serde(default)]
    pub summary_embedding: Option<SummaryEmbeddingRecord>,
    #[serde(default)]
    pub tasks: Vec<TaskRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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
pub enum TaskStatus {
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
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskRecord {
    #[serde(default = "default_task_id")]
    pub task_id: String,
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub last_error: Option<String>,
    pub retry_count: u32,
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
    pub(crate) fn empty() -> Self {
        Self {
            version: 3,
            model_preparations: ModelPreparations::default(),
            jobs: Vec::new(),
        }
    }
}

impl JobRecord {
    pub fn new(job_id: String, started_at: String, source_path: PathBuf, job_dir: PathBuf) -> Self {
        let source_file_name = source_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| source_path.to_string_lossy().into_owned());

        Self {
            job_id,
            status: JobStatus::Running,
            started_at: started_at.clone(),
            finished_at: None,
            source_path: source_path.to_string_lossy().into_owned(),
            source_file_name,
            job_dir: job_dir.to_string_lossy().into_owned(),
            probe: JobProbe::default(),
            split_strategy: SplitStrategy::PerChannelPlusMergedMono,
            outputs: JobOutputs::default(),
            error_message: None,
            summary_embedding: None,
            tasks: vec![TaskRecord::new(
                TaskType::Ffmpeg,
                TaskStatus::Running,
                started_at,
            )],
        }
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
        self.update_existing_task(
            TaskType::Ffmpeg,
            TaskStatus::Completed,
            Some(finished_at),
            None,
        )
    }

    pub fn mark_failed(
        &mut self,
        finished_at: String,
        error_message: String,
    ) -> Result<(), String> {
        self.status = JobStatus::Failed;
        self.finished_at = Some(finished_at.clone());
        self.error_message = Some(error_message.clone());
        self.update_existing_task(
            TaskType::Ffmpeg,
            TaskStatus::Failed,
            Some(finished_at),
            Some(error_message),
        )
    }

    pub fn upsert_running_task(&mut self, task_type: TaskType, started_at: String) {
        let retry_count = self
            .task(task_type)
            .map(|task| task.retry_count.saturating_add(1))
            .unwrap_or(0);
        self.set_task(TaskRecord {
            task_id: build_task_id(),
            task_type,
            status: TaskStatus::Running,
            started_at,
            finished_at: None,
            last_error: None,
            retry_count,
        });
    }

    pub fn complete_task(
        &mut self,
        task_type: TaskType,
        finished_at: String,
    ) -> Result<(), String> {
        self.update_existing_task(task_type, TaskStatus::Completed, Some(finished_at), None)
    }

    pub fn fail_task(
        &mut self,
        task_type: TaskType,
        finished_at: String,
        error: String,
    ) -> Result<(), String> {
        self.update_existing_task(
            task_type,
            TaskStatus::Failed,
            Some(finished_at),
            Some(error),
        )
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
        status: TaskStatus,
        finished_at: Option<String>,
        last_error: Option<String>,
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

        task.status = status;
        task.finished_at = finished_at;
        task.last_error = last_error;
        Ok(())
    }
}

impl TaskRecord {
    pub fn new(task_type: TaskType, status: TaskStatus, started_at: String) -> Self {
        Self {
            task_id: build_task_id(),
            task_type,
            status,
            started_at,
            finished_at: None,
            last_error: None,
            retry_count: 0,
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

impl JobOutputs {
    pub(crate) fn has_reusable_files(&self) -> bool {
        let Some(merged_mono_wav) = self.merged_mono_wav.as_deref() else {
            return false;
        };
        if self.split_mono_wavs.is_empty() {
            return false;
        }

        Path::new(merged_mono_wav).is_file()
            && self
                .split_mono_wavs
                .iter()
                .all(|output| Path::new(&output.path).is_file())
    }
}

fn build_task_id() -> String {
    Uuid::now_v7().to_string()
}

fn default_task_id() -> String {
    String::new()
}
