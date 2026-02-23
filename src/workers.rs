use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use std::time::Duration;

use tokio::{
    sync::{mpsc, Semaphore},
    time::timeout,
};

use crate::{
    AppConfig, EngineClient, EngineDispatcher, EngineKind, JobRequest, JobStore, QueueDepthGuard,
};

pub fn build_dispatchers(
    config: &AppConfig,
    jobs: Arc<JobStore>,
    engine_client: Arc<dyn EngineClient>,
) -> HashMap<EngineKind, EngineDispatcher> {
    let mut dispatchers = HashMap::new();

    for engine in [EngineKind::Stt, EngineKind::Summarize, EngineKind::Embed] {
        let (sender, receiver) = mpsc::channel(config.queue_capacity(engine));
        let queue_depth = Arc::new(AtomicUsize::new(0));
        let semaphore = Arc::new(Semaphore::new(config.concurrency(engine).max(1)));

        spawn_worker_loop(
            engine,
            receiver,
            semaphore,
            jobs.clone(),
            engine_client.clone(),
            Duration::from_secs(config.job_timeout_secs),
        );

        dispatchers.insert(
            engine,
            EngineDispatcher {
                sender,
                queue_depth,
            },
        );
    }

    dispatchers
}

pub fn spawn_worker_loop(
    engine: EngineKind,
    mut receiver: mpsc::Receiver<JobRequest>,
    semaphore: Arc<Semaphore>,
    jobs: Arc<JobStore>,
    engine_client: Arc<dyn EngineClient>,
    job_timeout: Duration,
) {
    tokio::spawn(async move {
        while let Some(request) = receiver.recv().await {
            // Acquire permit before spawning so recv loop cannot drain queue without available
            // execution capacity. This preserves bounded-mpsc backpressure under load.
            let permit = semaphore
                .clone()
                .acquire_owned()
                .await
                .expect("engine semaphore closed unexpectedly");

            let jobs = jobs.clone();
            let engine_client = engine_client.clone();
            let job_timeout = job_timeout;

            tokio::spawn(async move {
                let _permit = permit;
                let _queue_depth_guard: Option<QueueDepthGuard> = request.queue_depth_guard;

                let start = std::time::Instant::now();
                jobs.mark_running(&request.job_id);
                tracing::info!(
                    job_id = %request.job_id.safe_for_logs(),
                    engine = engine.as_str(),
                    status_transition = "queued->running",
                    "job started"
                );

                match timeout(
                    job_timeout,
                    engine_client.call(request.engine, request.payload),
                )
                .await
                {
                    Ok(Ok(result)) => {
                        let latency_ms = start.elapsed().as_millis() as u64;
                        jobs.mark_succeeded(&request.job_id, result.payload);
                        tracing::info!(
                            job_id = %request.job_id.safe_for_logs(),
                            engine = engine.as_str(),
                            status_transition = "running->succeeded",
                            latency_ms,
                            "job completed"
                        );
                    }
                    Ok(Err(error)) => {
                        let latency_ms = start.elapsed().as_millis() as u64;
                        jobs.mark_failed(&request.job_id, error.code, error.message.clone());
                        tracing::warn!(
                            job_id = %request.job_id.safe_for_logs(),
                            engine = engine.as_str(),
                            status_transition = "running->failed",
                            latency_ms,
                            retryable = error.retryable,
                            error_code = error.code,
                            error_message = %crate::sanitize_for_logs(&error.message, 120),
                            "job failed"
                        );
                    }
                    Err(_) => {
                        let latency_ms = start.elapsed().as_millis() as u64;
                        jobs.mark_timeout(
                            &request.job_id,
                            "job_timeout",
                            format!(
                                "job exceeded timeout budget: {} ms",
                                job_timeout.as_millis()
                            ),
                        );
                        tracing::warn!(
                            job_id = %request.job_id.safe_for_logs(),
                            engine = engine.as_str(),
                            status_transition = "running->timeout",
                            latency_ms,
                            error_code = "job_timeout",
                            "job timed out"
                        );
                    }
                }
            });
        }
    });
}

impl crate::EngineDispatcher {
    // queue_depth is an approximation based on accepted enqueues. It is decremented via RAII
    // guard drop, which makes decrement reliable across success/failure/panic paths.
    pub fn enqueue(&self, mut request: JobRequest) -> Result<(), crate::QueueEnqueueError> {
        request.queue_depth_guard = Some(QueueDepthGuard::new(self.queue_depth.clone()));
        match self.sender.try_send(request) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => Err(crate::QueueEnqueueError::Full),
            Err(mpsc::error::TrySendError::Closed(_)) => Err(crate::QueueEnqueueError::Closed),
        }
    }

    pub fn queue_depth(&self) -> usize {
        self.queue_depth.load(Ordering::Relaxed)
    }
}
