use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

use crate::models::Job;
use crate::pipeline::PipelineProcessor;
use crate::storage::RecordingRepository;

pub struct JobWorker {
    repo: Arc<dyn RecordingRepository>,
    processor: PipelineProcessor,
    notify: Arc<Notify>,
    poll_interval: Duration,
    max_attempts: i32,
    retry_backoff: Duration,
}

impl JobWorker {
    pub fn new(
        repo: Arc<dyn RecordingRepository>,
        processor: PipelineProcessor,
        notify: Arc<Notify>,
        poll_interval: Duration,
        max_attempts: i32,
        retry_backoff: Duration,
    ) -> Self {
        Self {
            repo,
            processor,
            notify,
            poll_interval,
            max_attempts,
            retry_backoff,
        }
    }

    pub fn spawn(self) {
        tokio::spawn(async move {
            self.run_loop().await;
        });
    }

    async fn run_loop(self) {
        loop {
            match self.repo.claim_next_job().await {
                Ok(Some(job)) => {
                    self.handle_job(job).await;
                    continue;
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::error!("failed to claim next job: {error}");
                }
            }

            tokio::select! {
                _ = self.notify.notified() => {},
                _ = tokio::time::sleep(self.poll_interval) => {},
            }
        }
    }

    async fn handle_job(&self, job: Job) {
        if let Err(error) = self.processor.process_job(job.clone()).await {
            tracing::error!(
                "job {} failed at step {}: {}",
                job.id,
                error.step,
                error.source
            );
            if let Err(update_error) = self
                .repo
                .reschedule_or_fail_job(
                    &job,
                    error.step,
                    &error.source.to_string(),
                    self.max_attempts,
                    self.retry_backoff,
                )
                .await
            {
                tracing::error!("failed to update job {} after pipeline failure: {update_error}", job.id);
            }
        }
    }
}
