use crate::app;
use crate::index::{
    DictionaryKeywords, JobOutputs, JobProbe, JobRecord, JobStatus, ModelKind,
    ModelPreparationRecord, QueueCategory, SourceKind, TaskRecord, TaskType,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PingRequest {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PingResponse {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ErrorResponse {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateJobRequest {
    pub input_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct JobsQuery {
    pub source_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SummaryRequest {
    #[serde(default)]
    pub force_regenerate: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SttRequest {
    #[serde(default)]
    pub audio_files: Vec<String>,
    #[serde(default)]
    pub mono_mix_only: bool,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DictionaryKeywordRequest {
    pub keyword: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct QueuePauseRequest {
    pub paused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct DictionaryKeywordListResponse {
    pub user_keywords: Vec<String>,
    pub auto_keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct TaskSubmissionResponse {
    pub job_id: String,
    pub task_type: TaskType,
    pub status: String,
    pub message: String,
    pub reused: bool,
    pub deduplicated: bool,
    pub queue: Option<QueueInfoResponse>,
    pub task: Option<TaskRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct FileListResponse {
    pub job_id: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct JobStatusResponse {
    pub job_id: String,
    pub job_status: JobStatus,
    pub tasks: Vec<TaskRecord>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SystemStatusResponse {
    pub ffmpeg_available: bool,
    pub whisper_available: bool,
    pub llama_available: bool,
    pub whisper_model_ready: bool,
    pub llama_model_ready: bool,
    pub llama_embedding_model_ready: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ModelStatusEntryResponse {
    pub available: bool,
    pub ready: bool,
    pub error: Option<String>,
    pub embedding_available: bool,
    pub embedding_ready: bool,
    pub embedding_error: Option<String>,
    pub preparation: ModelPreparationRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ModelStatusResponse {
    pub whisper: ModelStatusEntryResponse,
    pub llama: ModelStatusEntryResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ModelPrepareResponse {
    pub model: ModelKind,
    pub status: String,
    pub message: String,
    pub ready: bool,
    pub already_ready: bool,
    pub deduplicated: bool,
    pub preparation: ModelPreparationRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SttTranscriptText {
    pub transcript_id: String,
    pub file_name: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SttTranscriptListResponse {
    pub job_id: String,
    pub transcripts: Vec<SttTranscriptText>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SttProgressResponse {
    pub job_id: String,
    pub task: Option<TaskRecord>,
    pub phase: String,
    pub total_files: usize,
    pub completed_files: usize,
    pub progress_percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SummaryTextResponse {
    pub job_id: String,
    pub file_name: String,
    pub text: String,
    pub one_line_summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SummarySearchRequest {
    pub query: String,
    pub limit: Option<usize>,
    pub min_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct SummarySearchResultResponse {
    pub job_id: String,
    pub score: f32,
    pub source_file_name: String,
    pub summary_file_name: String,
    pub summary_excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct SummarySearchResponse {
    pub query: String,
    pub results: Vec<SummarySearchResultResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SummaryEmbeddingResponse {
    pub job_id: String,
    pub status: String,
    pub message: String,
    pub reused: bool,
    pub deduplicated: bool,
    pub queue: Option<QueueInfoResponse>,
    pub task: Option<TaskRecord>,
    pub metadata: Option<crate::index::SummaryEmbeddingRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct QueueInfoResponse {
    pub category: QueueCategory,
    pub position: usize,
    pub queued_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct QueueEntryResponse {
    pub job_id: String,
    pub task_type: TaskType,
    pub category: QueueCategory,
    pub queued_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct QueueBatchResponse {
    pub category: QueueCategory,
    pub running: Option<QueueEntryResponse>,
    pub entries: Vec<QueueEntryResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct QueueStatusResponse {
    pub paused: bool,
    pub burst_limit: u32,
    pub active_batch: Option<QueueBatchResponse>,
    pub pending_batches: Vec<QueueBatchResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct QueueCancelPendingResponse {
    pub total_cancelled: usize,
    pub ffmpeg_cancelled: usize,
    pub stt_cancelled: usize,
    pub summary_cancelled: usize,
    pub embedding_cancelled: usize,
}

impl From<DictionaryKeywords> for DictionaryKeywordListResponse {
    fn from(value: DictionaryKeywords) -> Self {
        Self {
            user_keywords: value.user_keywords,
            auto_keywords: value.auto_keywords,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct JobSubmissionResponse {
    pub job_id: String,
    pub status: JobStatus,
    pub message: String,
    pub reused: bool,
    pub deduplicated: bool,
    pub queue: Option<QueueInfoResponse>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub source_ref: String,
    pub source_kind: SourceKind,
    pub source_content_sha256: String,
    pub source_file_name: String,
    pub probe: JobProbe,
    pub outputs: JobOutputs,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct JobListResponse {
    pub jobs: Vec<JobRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BatchQueueSubmissionResponse {
    pub total_jobs: usize,
    pub ffmpeg_queued: usize,
    pub stt_queued: usize,
    pub summary_queued: usize,
    pub embedding_queued: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct AppState {
    pub repo_root: PathBuf,
    pub upload_limits: super::upload::UploadLimits,
    pub queue_dispatcher: super::queue_dispatcher::QueueDispatcher,
    pub invalid_request_body: ErrorResponse,
}

impl AppState {
    pub(crate) fn new(repo_root: PathBuf, upload_limits: super::upload::UploadLimits) -> Self {
        let queue_dispatcher = super::queue_dispatcher::QueueDispatcher::start(repo_root.clone());
        Self {
            repo_root,
            upload_limits,
            queue_dispatcher,
            invalid_request_body: ErrorResponse {
                code: "400".to_string(),
                message: "invalid request body".to_string(),
            },
        }
    }
}

pub(crate) fn build_job_submission_response(
    job: &JobRecord,
    reused: bool,
    deduplicated: bool,
    queue: Option<app::QueueTicket>,
) -> JobSubmissionResponse {
    JobSubmissionResponse {
        job_id: job.job_id.clone(),
        status: job.status.clone(),
        message: if reused {
            "completed job reused".to_string()
        } else if deduplicated {
            "job already running".to_string()
        } else {
            "job accepted".to_string()
        },
        reused,
        deduplicated,
        queue: queue.map(build_queue_info_response),
        started_at: job.started_at.clone(),
        finished_at: job.finished_at.clone(),
        source_ref: job.source_ref.clone(),
        source_kind: job.source_kind,
        source_content_sha256: job.source_content_sha256.clone(),
        source_file_name: job.source_file_name.clone(),
        probe: job.probe.clone(),
        outputs: job.outputs.clone(),
        error_message: super::errors::sanitize_optional_dependency_message(
            job.error_message.clone(),
        ),
    }
}

pub(crate) fn build_task_submission_response(
    job_id: String,
    task_type: TaskType,
    submission: &app::StageJobSubmission,
    accepted_message: &str,
    reused_message: &str,
    deduplicated_message: &str,
) -> TaskSubmissionResponse {
    TaskSubmissionResponse {
        job_id,
        task_type,
        status: "accepted".to_string(),
        message: if submission.reused() {
            reused_message.to_string()
        } else if submission.deduplicated() {
            deduplicated_message.to_string()
        } else {
            accepted_message.to_string()
        },
        reused: submission.reused(),
        deduplicated: submission.deduplicated(),
        queue: submission.queue.clone().map(build_queue_info_response),
        task: submission
            .job
            .task(task_type)
            .cloned()
            .map(super::errors::sanitize_task),
    }
}

pub(crate) fn build_task_status_response(
    job_id: String,
    task_type: TaskType,
    message: &str,
    task: Option<TaskRecord>,
) -> TaskSubmissionResponse {
    TaskSubmissionResponse {
        job_id,
        task_type,
        status: "ok".to_string(),
        message: message.to_string(),
        reused: false,
        deduplicated: false,
        queue: None,
        task: task.map(super::errors::sanitize_task),
    }
}

pub(crate) fn build_summary_embedding_submission_response(
    job_id: String,
    submission: &app::StageJobSubmission,
) -> SummaryEmbeddingResponse {
    SummaryEmbeddingResponse {
        job_id,
        status: "accepted".to_string(),
        message: if submission.reused() {
            "summary embedding reused".to_string()
        } else if submission.deduplicated() {
            "summary embedding already running".to_string()
        } else {
            "summary embedding accepted".to_string()
        },
        reused: submission.reused(),
        deduplicated: submission.deduplicated(),
        queue: submission.queue.clone().map(build_queue_info_response),
        task: submission
            .job
            .task(TaskType::Embedding)
            .cloned()
            .map(super::errors::sanitize_task),
        metadata: submission.job.summary_embedding.clone(),
    }
}

pub(crate) fn build_summary_embedding_status_response(
    job_id: String,
    job: &JobRecord,
) -> SummaryEmbeddingResponse {
    SummaryEmbeddingResponse {
        job_id,
        status: "ok".to_string(),
        message: "summary embedding task status".to_string(),
        reused: false,
        deduplicated: false,
        queue: None,
        task: job
            .task(TaskType::Embedding)
            .cloned()
            .map(super::errors::sanitize_task),
        metadata: job.summary_embedding.clone(),
    }
}

pub(crate) fn build_queue_info_response(queue: app::QueueTicket) -> QueueInfoResponse {
    QueueInfoResponse {
        category: queue.category,
        position: queue.position,
        queued_at: queue.queued_at,
    }
}

pub(crate) fn build_batch_queue_submission_response(
    submission: &app::BatchQueueSubmission,
) -> BatchQueueSubmissionResponse {
    BatchQueueSubmissionResponse {
        total_jobs: submission.total_jobs,
        ffmpeg_queued: submission.ffmpeg_queued,
        stt_queued: submission.stt_queued,
        summary_queued: submission.summary_queued,
        embedding_queued: submission.embedding_queued,
    }
}

pub(crate) fn build_model_status_response(status: app::ModelStatusSnapshot) -> ModelStatusResponse {
    let status = super::errors::sanitize_model_status_snapshot(status);
    ModelStatusResponse {
        whisper: build_model_status_entry_response(status.whisper),
        llama: build_model_status_entry_response(status.llama),
    }
}

fn build_model_status_entry_response(entry: app::ModelStatusEntry) -> ModelStatusEntryResponse {
    ModelStatusEntryResponse {
        available: entry.available,
        ready: entry.ready,
        error: entry.error,
        embedding_available: entry.embedding_available,
        embedding_ready: entry.embedding_ready,
        embedding_error: entry.embedding_error,
        preparation: super::errors::sanitize_model_preparation(entry.preparation),
    }
}

pub(crate) fn build_model_prepare_response(
    submission: &app::ModelPrepareSubmission,
) -> ModelPrepareResponse {
    let model_name = submission.model.as_str();
    ModelPrepareResponse {
        model: submission.model,
        status: if submission.already_ready() {
            "ok".to_string()
        } else {
            "accepted".to_string()
        },
        message: if submission.already_ready() {
            format!("{model_name} model already ready")
        } else if submission.deduplicated() {
            format!("{model_name} model preparation already running")
        } else {
            format!("{model_name} model preparation accepted")
        },
        ready: submission.already_ready(),
        already_ready: submission.already_ready(),
        deduplicated: submission.deduplicated(),
        preparation: super::errors::sanitize_model_preparation(submission.preparation.clone()),
    }
}
