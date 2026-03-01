use std::{
    collections::HashMap,
    fmt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::Value;

use crate::{sanitize_for_logs, EngineKind};

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Job {
    pub(crate) job_id: JobId,
    pub(crate) status: JobStatus,
    pub(crate) engine: EngineKind,
    pub(crate) created_at_ms: u64,
    pub(crate) started_at_ms: Option<u64>,
    pub(crate) finished_at_ms: Option<u64>,
    pub(crate) result: Option<Value>,
    pub(crate) error: Option<JobError>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub(crate) struct JobId(String);

#[derive(Clone, Debug, Serialize)]
pub(crate) struct JobError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Canceled,
    Rejected,
    Timeout,
}

#[derive(Debug)]
pub(crate) enum JobIdValidationError {
    Empty,
    TooLong,
    InvalidCharacter,
}

#[derive(Debug)]
pub(crate) struct JobStore {
    next_id: AtomicU64,
    pub(crate) jobs: Mutex<HashMap<JobId, Job>>,
}

impl JobId {
    const MAX_LEN: usize = 64;

    pub(crate) fn parse(input: &str) -> Result<Self, JobIdValidationError> {
        if input.is_empty() {
            return Err(JobIdValidationError::Empty);
        }
        if input.len() > Self::MAX_LEN {
            return Err(JobIdValidationError::TooLong);
        }
        if !input
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(JobIdValidationError::InvalidCharacter);
        }

        Ok(Self(input.to_string()))
    }

    pub(crate) fn generated(job_number: u64) -> Self {
        Self(format!("job-{job_number:010}"))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn safe_for_logs(&self) -> String {
        sanitize_for_logs(self.as_str(), 24)
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl JobIdValidationError {
    pub(crate) fn message(&self) -> &'static str {
        match self {
            JobIdValidationError::Empty => "job_id must not be empty",
            JobIdValidationError::TooLong => "job_id must be at most 64 characters",
            JobIdValidationError::InvalidCharacter => {
                "job_id supports only [a-zA-Z0-9_-] characters"
            }
        }
    }
}

impl JobStore {
    pub(crate) fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            jobs: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn create_queued_job(&self, engine: EngineKind) -> Job {
        let job_number = self.next_id.fetch_add(1, Ordering::Relaxed);
        let job_id = JobId::generated(job_number);
        let now = now_ms();
        let job = Job {
            job_id: job_id.clone(),
            status: JobStatus::Queued,
            engine,
            created_at_ms: now,
            started_at_ms: None,
            finished_at_ms: None,
            result: None,
            error: None,
        };

        self.jobs
            .lock()
            .expect("job store mutex poisoned")
            .insert(job_id, job.clone());

        job
    }

    pub(crate) fn get_job(&self, job_id: &JobId) -> Option<Job> {
        self.jobs
            .lock()
            .expect("job store mutex poisoned")
            .get(job_id)
            .cloned()
    }

    pub(crate) fn mark_running(&self, job_id: &JobId) {
        self.transition(job_id, |job| {
            if job.status == JobStatus::Queued {
                job.status = JobStatus::Running;
                job.started_at_ms = Some(now_ms());
            }
        });
    }

    pub(crate) fn mark_succeeded(&self, job_id: &JobId, result: Value) {
        self.transition(job_id, |job| {
            if job.status == JobStatus::Running {
                job.status = JobStatus::Completed;
                job.result = Some(result.clone());
                job.finished_at_ms = Some(now_ms());
            }
        });
    }

    pub(crate) fn mark_failed(&self, job_id: &JobId, code: &'static str, message: String) {
        self.transition(job_id, |job| {
            if matches!(job.status, JobStatus::Queued | JobStatus::Running) {
                job.status = JobStatus::Failed;
                if job.started_at_ms.is_none() {
                    job.started_at_ms = Some(now_ms());
                }
                job.finished_at_ms = Some(now_ms());
                job.error = Some(JobError {
                    code,
                    message: message.clone(),
                });
            }
        });
    }

    pub(crate) fn mark_rejected_capacity_exceeded(
        &self,
        job_id: &JobId,
        engine: EngineKind,
        code: &'static str,
    ) {
        self.transition(job_id, |job| {
            if job.status == JobStatus::Queued {
                job.status = JobStatus::Rejected;
                job.finished_at_ms = Some(now_ms());
                job.error = Some(JobError {
                    code,
                    message: format!("{} capacity exceeded ({code})", engine.as_str()),
                });
            }
        });
    }

    pub(crate) fn mark_canceled(&self, job_id: &JobId, message: String) {
        self.transition(job_id, |job| {
            if matches!(job.status, JobStatus::Queued | JobStatus::Running) {
                job.status = JobStatus::Canceled;
                if job.started_at_ms.is_none() {
                    job.started_at_ms = Some(now_ms());
                }
                job.finished_at_ms = Some(now_ms());
                job.error = Some(JobError {
                    code: "job_canceled",
                    message: message.clone(),
                });
            }
        });
    }

    pub(crate) fn mark_timeout(&self, job_id: &JobId, message: String) {
        self.transition(job_id, |job| {
            if job.status == JobStatus::Running {
                job.status = JobStatus::Timeout;
                job.finished_at_ms = Some(now_ms());
                job.error = Some(JobError {
                    code: "job_timeout",
                    message: message.clone(),
                });
            }
        });
    }

    pub(crate) fn running_count(&self, engine: EngineKind) -> usize {
        self.jobs
            .lock()
            .expect("job store mutex poisoned")
            .values()
            .filter(|job| job.engine == engine && job.status == JobStatus::Running)
            .count()
    }

    fn transition<F>(&self, job_id: &JobId, mut updater: F)
    where
        F: FnMut(&mut Job),
    {
        if let Some(job) = self
            .jobs
            .lock()
            .expect("job store mutex poisoned")
            .get_mut(job_id)
        {
            updater(job);
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
