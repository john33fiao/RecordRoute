use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use std::time::Duration;

use tokio::{
    sync::{mpsc, Mutex},
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

        spawn_worker_pool(
            engine,
            receiver,
            config.concurrency(engine).max(1),
            jobs.clone(),
            engine_client.clone(),
            Duration::from_secs(config.job_timeout_secs),
        );

        dispatchers.insert(
            engine,
            EngineDispatcher {
                sender,
                queue_depth,
                queue_capacity: config.queue_capacity(engine),
                worker_count: config.concurrency(engine).max(1),
            },
        );
    }

    dispatchers
}

pub fn spawn_worker_pool(
    engine: EngineKind,
    receiver: mpsc::Receiver<JobRequest>,
    worker_count: usize,
    jobs: Arc<JobStore>,
    engine_client: Arc<dyn EngineClient>,
    _job_timeout: Duration,
) {
    let receiver = Arc::new(Mutex::new(receiver));

    for worker_index in 0..worker_count {
        let receiver = receiver.clone();
        let jobs = jobs.clone();
        let engine_client = engine_client.clone();

        tokio::spawn(async move {
            loop {
                let request = {
                    let mut locked = receiver.lock().await;
                    locked.recv().await
                };

                let Some(request) = request else {
                    tracing::info!(
                        engine = engine.as_str(),
                        worker_index,
                        "worker channel closed"
                    );
                    break;
                };

                let _queue_depth_guard: Option<QueueDepthGuard> = request.queue_depth_guard;

                let start = std::time::Instant::now();
                jobs.mark_running(&request.job_id);
                tracing::info!(
                    job_id = %request.job_id.safe_for_logs(),
                    engine = engine.as_str(),
                    worker_index,
                    status_transition = "queued->running",
                    "job started"
                );

                let timeout_budget = request.timeout_budget;

                match timeout(
                    timeout_budget,
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
                            worker_index,
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
                            worker_index,
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
                        jobs.mark_canceled(
                            &request.job_id,
                            format!(
                                "job canceled after timeout budget exceeded: {} ms",
                                timeout_budget.as_millis()
                            ),
                        );
                        tracing::warn!(
                            job_id = %request.job_id.safe_for_logs(),
                            engine = engine.as_str(),
                            worker_index,
                            status_transition = "running->canceled",
                            latency_ms,
                            error_code = "job_canceled",
                            "job canceled due to timeout budget"
                        );
                    }
                }
            }
        });
    }
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
