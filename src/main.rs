use std::{
    collections::HashMap,
    env,
    future::Future,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tracing_subscriber::{fmt as tracing_fmt, EnvFilter};

mod audio;
mod domain;
mod engine_manager;
mod workers;

use domain::{JobId, JobStore};
use engine_manager::{EngineManager, EngineManagerConfig, EngineProcessSpec};
use workers::build_dispatchers;

#[derive(Clone)]
struct AppState {
    ready: bool,
    degraded: Arc<AtomicBool>,
    config: Arc<AppConfig>,
    jobs: Arc<JobStore>,
    dispatchers: Arc<HashMap<EngineKind, EngineDispatcher>>,
    rejection_metrics: Arc<RejectionMetrics>,
}

pub(crate) struct EngineDispatcher {
    pub(crate) sender: mpsc::Sender<JobRequest>,
    pub(crate) queue_depth: Arc<AtomicUsize>,
    pub(crate) queue_capacity: usize,
    pub(crate) worker_count: usize,
}

#[derive(Debug)]
struct RejectionMetrics {
    counters: Mutex<HashMap<(EngineKind, &'static str), u64>>,
}

#[derive(Serialize)]
struct MetricsResponse {
    readiness: ReadinessMetrics,
    engines: Vec<EngineMetrics>,
    rejections: Vec<RejectionMetric>,
}

#[derive(Serialize)]
struct ReadinessMetrics {
    ready: bool,
    degraded: bool,
}

#[derive(Serialize)]
struct EngineMetrics {
    engine: &'static str,
    queue_depth: usize,
    queue_capacity: usize,
    worker_count: usize,
    running: usize,
}

#[derive(Serialize)]
struct RejectionMetric {
    engine: &'static str,
    reason: &'static str,
    count: u64,
}

#[derive(Debug)]
pub(crate) struct JobRequest {
    pub(crate) job_id: JobId,
    pub(crate) engine: EngineKind,
    pub(crate) payload: Value,
    pub(crate) timeout_budget: Duration,
    pub(crate) queue_depth_guard: Option<QueueDepthGuard>,
}

#[derive(Clone, Debug)]
pub(crate) struct AppConfig {
    host: String,
    api_port: u16,
    stt_queue_capacity: usize,
    summarize_queue_capacity: usize,
    embed_queue_capacity: usize,
    stt_concurrency: usize,
    summarize_concurrency: usize,
    embed_concurrency: usize,
    engine_timeout_secs: u64,
    pub(crate) job_timeout_secs: u64,
    pub(crate) job_timeout_min_secs: u64,
    pub(crate) job_timeout_max_secs: u64,
    pub(crate) stt_timeout_per_audio_sec_ms: u64,
    pub(crate) stt_timeout_buffer_ms: u64,
    engine_connect_timeout_ms: u64,
    engine_retry_count: usize,
    engine_base_backoff_ms: u64,
    stt_engine_url: String,
    summarize_engine_url: String,
    embed_engine_url: String,
    engine_supervision_enabled: bool,
    stt_engine_command: String,
    stt_engine_args: Vec<String>,
    summarize_engine_command: String,
    summarize_engine_args: Vec<String>,
    embed_engine_command: String,
    embed_engine_args: Vec<String>,
    engine_startup_timeout_secs: u64,
    engine_readiness_poll_ms: u64,
    engine_shutdown_grace_secs: u64,
    engine_restart_backoff_base_ms: u64,
    engine_restart_backoff_max_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EngineKind {
    Stt,
    Summarize,
    Embed,
}

#[derive(Debug)]
pub(crate) enum QueueEnqueueError {
    Full,
    Closed,
}

#[derive(Debug)]
pub(crate) struct EngineResult {
    pub(crate) payload: Value,
}

#[derive(Debug)]
pub(crate) struct EngineError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) retryable: bool,
}

type EngineFuture<'a> =
    Pin<Box<dyn Future<Output = Result<EngineResult, EngineError>> + Send + 'a>>;

pub(crate) trait EngineClient: Send + Sync {
    fn call(&self, engine: EngineKind, payload: Value) -> EngineFuture<'_>;
}

#[derive(Clone)]
struct HttpEngineClient {
    endpoints: HashMap<EngineKind, String>,
    connect_timeout: Duration,
    timeout: Duration,
    retry_count: usize,
    base_backoff: Duration,
}

#[derive(Debug)]
pub(crate) struct QueueDepthGuard {
    queue_depth: Arc<AtomicUsize>,
}

impl QueueDepthGuard {
    pub(crate) fn new(queue_depth: Arc<AtomicUsize>) -> Self {
        queue_depth.fetch_add(1, Ordering::Relaxed);
        Self { queue_depth }
    }
}

impl Drop for QueueDepthGuard {
    fn drop(&mut self) {
        self.queue_depth.fetch_sub(1, Ordering::Relaxed);
    }
}

impl AppConfig {
    pub(crate) fn from_env() -> Self {
        let host = env::var("RECORDROUTE_API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let api_port = env::var("RECORDROUTE_API_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(18_000);

        Self {
            host,
            api_port,
            stt_queue_capacity: read_usize_env("RECORDROUTE_STT_QUEUE_CAPACITY", 1_024),
            summarize_queue_capacity: read_usize_env("RECORDROUTE_SUMMARIZE_QUEUE_CAPACITY", 1_024),
            embed_queue_capacity: read_usize_env("RECORDROUTE_EMBED_QUEUE_CAPACITY", 1_024),
            stt_concurrency: read_usize_env("RECORDROUTE_STT_CONCURRENCY", 2),
            summarize_concurrency: read_usize_env("RECORDROUTE_SUMMARIZE_CONCURRENCY", 2),
            embed_concurrency: read_usize_env("RECORDROUTE_EMBED_CONCURRENCY", 2),
            engine_timeout_secs: read_u64_env("RECORDROUTE_ENGINE_TIMEOUT_SECS", 60),
            job_timeout_secs: read_u64_env("RECORDROUTE_JOB_TIMEOUT_SECS", 120),
            job_timeout_min_secs: read_u64_env("RECORDROUTE_JOB_TIMEOUT_MIN_SECS", 30),
            job_timeout_max_secs: read_u64_env("RECORDROUTE_JOB_TIMEOUT_MAX_SECS", 900),
            stt_timeout_per_audio_sec_ms: read_u64_env(
                "RECORDROUTE_STT_TIMEOUT_PER_AUDIO_SEC_MS",
                1_500,
            ),
            stt_timeout_buffer_ms: read_u64_env("RECORDROUTE_STT_TIMEOUT_BUFFER_MS", 5_000),
            engine_connect_timeout_ms: read_u64_env("RECORDROUTE_ENGINE_CONNECT_TIMEOUT_MS", 1_500),
            engine_retry_count: read_usize_env("RECORDROUTE_ENGINE_RETRY_COUNT", 1),
            engine_base_backoff_ms: read_u64_env("RECORDROUTE_ENGINE_BACKOFF_MS", 200),
            stt_engine_url: env::var("RECORDROUTE_STT_ENGINE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:18103/infer".to_string()),
            summarize_engine_url: env::var("RECORDROUTE_SUMMARIZE_ENGINE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:18101/infer".to_string()),
            embed_engine_url: env::var("RECORDROUTE_EMBED_ENGINE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:18102/infer".to_string()),
            engine_supervision_enabled: env::var("RECORDROUTE_ENGINE_SUPERVISION_ENABLED")
                .ok()
                .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
                .unwrap_or(false),
            stt_engine_command: env::var("RECORDROUTE_STT_ENGINE_COMMAND")
                .unwrap_or_else(|_| "whisper-server".to_string()),
            stt_engine_args: read_csv_env("RECORDROUTE_STT_ENGINE_ARGS"),
            summarize_engine_command: env::var("RECORDROUTE_SUMMARIZE_ENGINE_COMMAND")
                .unwrap_or_else(|_| "llama-text".to_string()),
            summarize_engine_args: read_csv_env("RECORDROUTE_SUMMARIZE_ENGINE_ARGS"),
            embed_engine_command: env::var("RECORDROUTE_EMBED_ENGINE_COMMAND")
                .unwrap_or_else(|_| "llama-embed".to_string()),
            embed_engine_args: read_csv_env("RECORDROUTE_EMBED_ENGINE_ARGS"),
            engine_startup_timeout_secs: read_u64_env(
                "RECORDROUTE_ENGINE_STARTUP_TIMEOUT_SECS",
                20,
            ),
            engine_readiness_poll_ms: read_u64_env("RECORDROUTE_ENGINE_READINESS_POLL_MS", 500),
            engine_shutdown_grace_secs: read_u64_env("RECORDROUTE_ENGINE_SHUTDOWN_GRACE_SECS", 5),
            engine_restart_backoff_base_ms: read_u64_env(
                "RECORDROUTE_ENGINE_RESTART_BACKOFF_BASE_MS",
                500,
            ),
            engine_restart_backoff_max_ms: read_u64_env(
                "RECORDROUTE_ENGINE_RESTART_BACKOFF_MAX_MS",
                15_000,
            ),
        }
    }

    fn api_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.host, self.api_port).parse()
    }

    pub(crate) fn queue_capacity(&self, engine: EngineKind) -> usize {
        match engine {
            EngineKind::Stt => self.stt_queue_capacity,
            EngineKind::Summarize => self.summarize_queue_capacity,
            EngineKind::Embed => self.embed_queue_capacity,
        }
    }

    pub(crate) fn concurrency(&self, engine: EngineKind) -> usize {
        match engine {
            EngineKind::Stt => self.stt_concurrency,
            EngineKind::Summarize => self.summarize_concurrency,
            EngineKind::Embed => self.embed_concurrency,
        }
    }
}

impl RejectionMetrics {
    fn new() -> Self {
        Self {
            counters: Mutex::new(HashMap::new()),
        }
    }

    fn increment(&self, engine: EngineKind, reason: &'static str) -> u64 {
        let mut counters = self
            .counters
            .lock()
            .expect("rejection metrics mutex poisoned");
        let counter = counters.entry((engine, reason)).or_insert(0);
        *counter += 1;
        *counter
    }

    fn snapshot(&self) -> Vec<RejectionMetric> {
        let counters = self
            .counters
            .lock()
            .expect("rejection metrics mutex poisoned");

        counters
            .iter()
            .map(|((engine, reason), count)| RejectionMetric {
                engine: engine.as_str(),
                reason,
                count: *count,
            })
            .collect()
    }

    #[cfg(test)]
    fn count(&self, engine: EngineKind, reason: &'static str) -> u64 {
        *self
            .counters
            .lock()
            .expect("rejection metrics mutex poisoned")
            .get(&(engine, reason))
            .unwrap_or(&0)
    }
}

impl EngineKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            EngineKind::Stt => "stt",
            EngineKind::Summarize => "summarize",
            EngineKind::Embed => "embed",
        }
    }
}

impl EngineClient for HttpEngineClient {
    fn call(&self, engine: EngineKind, payload: Value) -> EngineFuture<'_> {
        Box::pin(async move {
            let endpoint = self
                .endpoints
                .get(&engine)
                .cloned()
                .ok_or_else(|| EngineError {
                    code: "engine_not_configured",
                    message: format!("no endpoint configured for engine {}", engine.as_str()),
                    retryable: false,
                })?;

            for attempt in 0..=self.retry_count {
                match call_engine_http(&endpoint, &payload, self.connect_timeout, self.timeout)
                    .await
                {
                    Ok((status, body)) if (200..300).contains(&status) => {
                        let json =
                            serde_json::from_str::<Value>(&body).map_err(|error| EngineError {
                                code: "engine_invalid_json",
                                message: format!("failed to decode engine response: {error}"),
                                retryable: false,
                            })?;
                        return Ok(EngineResult { payload: json });
                    }
                    Ok((status, _)) if status >= 500 => {
                        if attempt < self.retry_count {
                            sleep_backoff(self.base_backoff, attempt).await;
                            continue;
                        }
                        return Err(EngineError {
                            code: "engine_upstream_5xx",
                            message: format!("engine returned status {status}"),
                            retryable: true,
                        });
                    }
                    Ok((status, _)) => {
                        return Err(EngineError {
                            code: "engine_upstream_4xx",
                            message: format!("engine returned status {status}"),
                            retryable: false,
                        });
                    }
                    Err(error) => {
                        if attempt < self.retry_count {
                            sleep_backoff(self.base_backoff, attempt).await;
                            continue;
                        }
                        return Err(error);
                    }
                }
            }

            Err(EngineError {
                code: "engine_retry_exhausted",
                message: "engine retry budget exhausted".to_string(),
                retryable: true,
            })
        })
    }
}

#[tokio::main]
async fn main() {
    init_tracing();

    if let Err(error) = run_server().await {
        tracing::error!(error = %error, "server terminated with error");
    }
}

async fn run_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::from_env();
    let addr = config.api_addr()?;
    let jobs = Arc::new(JobStore::new());
    let engine_client = Arc::new(HttpEngineClient {
        endpoints: HashMap::from([
            (EngineKind::Stt, config.stt_engine_url.clone()),
            (EngineKind::Summarize, config.summarize_engine_url.clone()),
            (EngineKind::Embed, config.embed_engine_url.clone()),
        ]),
        connect_timeout: Duration::from_millis(config.engine_connect_timeout_ms),
        timeout: Duration::from_secs(config.engine_timeout_secs),
        retry_count: config.engine_retry_count,
        base_backoff: Duration::from_millis(config.engine_base_backoff_ms),
    });

    let dispatchers = build_dispatchers(&config, jobs.clone(), engine_client);

    let _engine_manager = if config.engine_supervision_enabled {
        tracing::info!("engine supervision enabled");
        Some(EngineManager::spawn(
            engine_specs_from_config(&config),
            supervision_config(&config),
        ))
    } else {
        tracing::info!("engine supervision disabled");
        None
    };

    let state = AppState {
        ready: true,
        degraded: Arc::new(AtomicBool::new(false)),
        config: Arc::new(config.clone()),
        jobs,
        dispatchers: Arc::new(dispatchers),
        rejection_metrics: Arc::new(RejectionMetrics::new()),
    };

    tracing::info!(%addr, "starting RecordRoute API server");

    tokio::task::spawn_blocking(move || run_blocking_server(addr, state)).await??;
    Ok(())
}

fn engine_specs_from_config(config: &AppConfig) -> Vec<EngineProcessSpec> {
    vec![
        EngineProcessSpec {
            name: "llama-text",
            port: 18101,
            command: config.summarize_engine_command.clone(),
            args: config.summarize_engine_args.clone(),
            health_path: "/healthz",
        },
        EngineProcessSpec {
            name: "llama-embed",
            port: 18102,
            command: config.embed_engine_command.clone(),
            args: config.embed_engine_args.clone(),
            health_path: "/healthz",
        },
        EngineProcessSpec {
            name: "whisper-server",
            port: 18103,
            command: config.stt_engine_command.clone(),
            args: config.stt_engine_args.clone(),
            health_path: "/healthz",
        },
    ]
}

fn supervision_config(config: &AppConfig) -> EngineManagerConfig {
    EngineManagerConfig {
        startup_timeout: Duration::from_secs(config.engine_startup_timeout_secs),
        readiness_poll_interval: Duration::from_millis(config.engine_readiness_poll_ms),
        shutdown_grace: Duration::from_secs(config.engine_shutdown_grace_secs),
        restart_backoff_base: Duration::from_millis(config.engine_restart_backoff_base_ms),
        restart_backoff_max: Duration::from_millis(config.engine_restart_backoff_max_ms),
    }
}

fn run_blocking_server(
    addr: SocketAddr,
    state: AppState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(addr)?;

    for stream in listener.incoming() {
        match stream {
            Ok(socket) => {
                if let Err(error) = handle_connection(socket, &state) {
                    tracing::warn!(error = %error, "request handling failed");
                }
            }
            Err(error) => tracing::warn!(error = %error, "incoming connection failed"),
        }
    }

    Ok(())
}

fn handle_connection(
    mut socket: TcpStream,
    state: &AppState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut buffer = [0; 4096];
    let bytes_read = socket.read(&mut buffer)?;
    if bytes_read == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let first_line = request.lines().next().unwrap_or_default();
    let body = request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.as_bytes())
        .unwrap_or_default();
    let response = route_request_with_body(first_line, body, state);

    socket.write_all(response.as_bytes())?;
    socket.flush()?;

    Ok(())
}

#[cfg(test)]
fn route_request(request_line: &str, state: &AppState) -> String {
    route_request_with_body(request_line, &[], state)
}

fn route_request_with_body(request_line: &str, body: &[u8], state: &AppState) -> String {
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path_with_query = parts.next().unwrap_or_default();
    let (path, query) = parse_path_and_query(path_with_query);

    match (method, path) {
        ("GET", "/healthz") => json_response(200, &StatusBody { status: "ok" }),
        ("GET", "/readyz") => {
            if state.ready && !state.degraded.load(Ordering::Relaxed) {
                json_response(200, &StatusBody { status: "ready" })
            } else {
                json_response(503, &StatusBody { status: "degraded" })
            }
        }
        ("GET", "/metrics") => {
            let engines = [EngineKind::Stt, EngineKind::Summarize, EngineKind::Embed]
                .iter()
                .filter_map(|engine| {
                    state
                        .dispatchers
                        .get(engine)
                        .map(|dispatcher| EngineMetrics {
                            engine: engine.as_str(),
                            queue_depth: dispatcher.queue_depth(),
                            queue_capacity: dispatcher.queue_capacity,
                            worker_count: dispatcher.worker_count,
                            running: state.jobs.running_count(*engine),
                        })
                })
                .collect();

            json_response(
                200,
                &MetricsResponse {
                    readiness: ReadinessMetrics {
                        ready: state.ready,
                        degraded: state.degraded.load(Ordering::Relaxed),
                    },
                    engines,
                    rejections: state.rejection_metrics.snapshot(),
                },
            )
        }
        ("POST", "/jobs") => {
            let engine = parse_engine_kind(query).unwrap_or(EngineKind::Stt);
            let Some(dispatcher) = state.dispatchers.get(&engine) else {
                return json_error_response(
                    503,
                    "engine_dispatcher_unavailable",
                    "engine unavailable",
                );
            };

            let job = state.jobs.create_queued_job(engine);
            let timeout_budget = derive_job_timeout_budget(engine, query, &state.config);
            let normalized_stt_audio = if engine == EngineKind::Stt && !body.is_empty() {
                match audio::normalize_to_wav_mono_16k(body) {
                    Ok(audio) => Some(audio),
                    Err(error) => {
                        state.jobs.mark_failed(
                            &job.job_id,
                            "invalid_audio_payload",
                            format!("audio normalization failed: {error}"),
                        );
                        return json_error_response(
                            400,
                            "invalid_audio_payload",
                            "invalid or unsupported audio payload",
                        );
                    }
                }
            } else {
                None
            };
            let request = JobRequest {
                job_id: job.job_id.clone(),
                engine,
                payload: build_engine_payload(
                    &job.job_id,
                    engine,
                    timeout_budget,
                    normalized_stt_audio,
                ),
                timeout_budget,
                queue_depth_guard: None,
            };

            match dispatcher.enqueue(request) {
                Ok(()) => {
                    state.degraded.store(false, Ordering::Relaxed);
                    json_response(
                        202,
                        &JobCreatedResponse {
                            job_id: job.job_id.as_str(),
                        },
                    )
                }
                Err(QueueEnqueueError::Full) => {
                    state.degraded.store(true, Ordering::Relaxed);
                    let reason = rejection_reason_for_full(engine, dispatcher, &state.jobs);
                    let rejection_count = state.rejection_metrics.increment(engine, reason);
                    state
                        .jobs
                        .mark_rejected_capacity_exceeded(&job.job_id, engine, reason);
                    tracing::warn!(
                        engine = engine.as_str(),
                        reason,
                        queue_depth = dispatcher.queue_depth(),
                        queue_capacity = dispatcher.queue_capacity,
                        running = state.jobs.running_count(engine),
                        worker_count = dispatcher.worker_count,
                        rejection_count,
                        "job rejected due to capacity pressure"
                    );
                    json_error_response(
                        429,
                        reason,
                        &format!("{} capacity exceeded ({reason})", engine.as_str()),
                    )
                }
                Err(QueueEnqueueError::Closed) => {
                    state.degraded.store(true, Ordering::Relaxed);
                    let rejection_count = state
                        .rejection_metrics
                        .increment(engine, "engine_dispatcher_closed");
                    state.jobs.mark_failed(
                        &job.job_id,
                        "engine_dispatcher_closed",
                        "engine dispatcher closed".to_string(),
                    );
                    tracing::warn!(
                        engine = engine.as_str(),
                        reason = "engine_dispatcher_closed",
                        rejection_count,
                        "job rejected because dispatcher is closed"
                    );
                    json_error_response(503, "engine_dispatcher_closed", "engine dispatcher closed")
                }
            }
        }
        ("GET", path) if path.starts_with("/jobs/") => {
            let job_id_raw = path.trim_start_matches("/jobs/");
            let job_id = match JobId::parse(job_id_raw) {
                Ok(job_id) => job_id,
                Err(error) => {
                    tracing::warn!(
                        job_id = %sanitize_for_logs(job_id_raw, 24),
                        reason = error.message(),
                        "invalid job_id in request"
                    );
                    return json_error_response(400, "invalid_job_id", error.message());
                }
            };

            match state.jobs.get_job(&job_id) {
                Some(job) => json_response(200, &job),
                None => json_error_response(404, "job_not_found", "job not found"),
            }
        }
        ("GET", _) => json_error_response(404, "not_found", "not found"),
        _ => json_error_response(405, "method_not_allowed", "method not allowed"),
    }
}

fn build_engine_payload(
    job_id: &JobId,
    engine: EngineKind,
    timeout_budget: Duration,
    normalized_stt_audio: Option<Vec<u8>>,
) -> Value {
    let mut payload = json!({
        "job_id": job_id,
        "engine": engine.as_str(),
        "timeout_budget_ms": timeout_budget.as_millis(),
    });

    if engine == EngineKind::Stt {
        let normalized_bytes = normalized_stt_audio
            .as_ref()
            .map(|audio| audio.len())
            .unwrap_or(0);
        payload["audio_contract"] = json!({
            "normalized_by": "recordroute_symphonia",
            "format": "wav_mono_pcm16_16khz",
            "conversion_required": normalized_bytes == 0,
            "normalized_bytes": normalized_bytes,
        });
    }

    payload
}

fn derive_job_timeout_budget(
    engine: EngineKind,
    query: Option<&str>,
    config: &AppConfig,
) -> Duration {
    let default_timeout = Duration::from_secs(config.job_timeout_secs);
    let Some(query) = query else {
        return default_timeout;
    };

    let min_ms = config.job_timeout_min_secs.saturating_mul(1_000);
    let max_ms = config.job_timeout_max_secs.saturating_mul(1_000);
    let min_ms = min_ms.min(max_ms);
    let max_ms = max_ms.max(min_ms);

    if engine != EngineKind::Stt {
        let clamped = default_timeout.as_millis() as u64;
        return Duration::from_millis(clamped.clamp(min_ms, max_ms));
    }

    let Some(audio_ms) = query_u64(query, "audio_ms") else {
        let clamped = default_timeout.as_millis() as u64;
        return Duration::from_millis(clamped.clamp(min_ms, max_ms));
    };

    let per_audio_sec_ms = config.stt_timeout_per_audio_sec_ms;
    let buffer_ms = config.stt_timeout_buffer_ms;
    let estimated_ms = audio_ms
        .saturating_mul(per_audio_sec_ms)
        .checked_div(1_000)
        .unwrap_or(u64::MAX)
        .saturating_add(buffer_ms);

    Duration::from_millis(estimated_ms.clamp(min_ms, max_ms))
}

fn query_u64(query: &str, key: &str) -> Option<u64> {
    query
        .split('&')
        .filter_map(|token| token.split_once('='))
        .find_map(|(k, v)| (k == key).then_some(v))
        .and_then(|v| v.parse::<u64>().ok())
}

fn rejection_reason_for_full(
    engine: EngineKind,
    dispatcher: &EngineDispatcher,
    jobs: &JobStore,
) -> &'static str {
    let running = jobs.running_count(engine);
    if running >= dispatcher.worker_count {
        "engine_full"
    } else {
        "queue_full"
    }
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
}

#[derive(Serialize)]
struct StatusBody<'a> {
    status: &'a str,
}

#[derive(Serialize)]
struct JobCreatedResponse<'a> {
    job_id: &'a str,
}

pub(crate) fn sanitize_for_logs(input: &str, max_len: usize) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
        if out.len() >= max_len {
            out.push('…');
            break;
        }
    }
    out
}

fn json_response(status_code: u16, body: &impl Serialize) -> String {
    let body_text = serde_json::to_string(body).unwrap_or_else(|_| "{}".to_string());

    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status_code,
        reason_phrase(status_code),
        body_text.len(),
        body_text
    )
}

fn json_error_response(status_code: u16, code: &str, message: &str) -> String {
    json_response(status_code, &ErrorBody { code, message })
}

fn reason_phrase(status_code: u16) -> &'static str {
    match status_code {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

fn parse_path_and_query(path_with_query: &str) -> (&str, Option<&str>) {
    match path_with_query.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (path_with_query, None),
    }
}

fn parse_engine_kind(query: Option<&str>) -> Option<EngineKind> {
    let query = query?;
    for token in query.split('&') {
        let (key, value) = token.split_once('=')?;
        if key == "engine" {
            return match value {
                "stt" => Some(EngineKind::Stt),
                "summarize" => Some(EngineKind::Summarize),
                "embed" => Some(EngineKind::Embed),
                _ => None,
            };
        }
    }
    None
}

fn read_usize_env(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
}

fn read_u64_env(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn read_csv_env(key: &str) -> Vec<String> {
    env::var(key)
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

async fn sleep_backoff(base: Duration, attempt: usize) {
    let factor = 2u32.saturating_pow(attempt as u32);
    tokio::time::sleep(base * factor).await;
}

async fn call_engine_http(
    endpoint: &str,
    payload: &Value,
    connect_timeout: Duration,
    timeout: Duration,
) -> Result<(u16, String), EngineError> {
    let endpoint = endpoint.to_string();
    let body = payload.to_string();

    tokio::task::spawn_blocking(move || {
        let parsed = parse_http_endpoint(&endpoint).ok_or_else(|| EngineError {
            code: "engine_endpoint_invalid",
            message: format!("invalid endpoint: {endpoint}"),
            retryable: false,
        })?;

        let connect_addr = (parsed.host.as_str(), parsed.port)
            .to_socket_addrs()
            .map_err(|error| EngineError {
                code: "engine_transport_error",
                message: format!("engine resolve failed: {error}"),
                retryable: true,
            })?
            .next()
            .ok_or_else(|| EngineError {
                code: "engine_transport_error",
                message: "engine resolve returned no addresses".to_string(),
                retryable: true,
            })?;

        let mut stream = TcpStream::connect_timeout(&connect_addr, connect_timeout).map_err(|error| {
            let timed_out = matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            );
            EngineError {
                code: if timed_out { "engine_connect_timeout" } else { "engine_transport_error" },
                message: format!("engine connect failed: {error}"),
                retryable: true,
            }
        })?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|error| EngineError {
                code: "engine_transport_error",
                message: format!("failed to set read timeout: {error}"),
                retryable: true,
            })?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(|error| EngineError {
                code: "engine_transport_error",
                message: format!("failed to set write timeout: {error}"),
                retryable: true,
            })?;

        let request = format!(
            "POST {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            parsed.path,
            parsed.host,
            parsed.port,
            body.len(),
            body,
        );

        stream.write_all(request.as_bytes()).map_err(|error| EngineError {
            code: "engine_transport_error",
            message: format!("engine write failed: {error}"),
            retryable: true,
        })?;

        let mut response = String::new();
        stream.read_to_string(&mut response).map_err(|error| {
            let timed_out = matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            );
            EngineError {
                code: if timed_out {
                    "engine_request_timeout"
                } else {
                    "engine_transport_error"
                },
                message: format!("engine read failed: {error}"),
                retryable: true,
            }
        })?;

        let mut lines = response.lines();
        let status_line = lines.next().unwrap_or_default();
        let status = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .ok_or_else(|| EngineError {
                code: "engine_invalid_http",
                message: format!("invalid status line: {status_line}"),
                retryable: false,
            })?;

        let body = response
            .split_once("\r\n\r\n")
            .map(|(_, body)| body.to_string())
            .unwrap_or_default();

        Ok((status, body))
    })
    .await
    .map_err(|error| EngineError {
        code: "engine_transport_error",
        message: format!("engine task join error: {error}"),
        retryable: true,
    })?
}

struct ParsedEndpoint {
    host: String,
    port: u16,
    path: String,
}

fn parse_http_endpoint(endpoint: &str) -> Option<ParsedEndpoint> {
    let rest = endpoint.strip_prefix("http://")?;
    let (host_port, path) = match rest.split_once('/') {
        Some((hp, p)) => (hp, format!("/{p}")),
        None => (rest, "/".to_string()),
    };

    let (host, port) = match host_port.split_once(':') {
        Some((host, port)) => (host.to_string(), port.parse().ok()?),
        None => (host_port.to_string(), 80),
    };

    Some(ParsedEndpoint { host, port, path })
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_fmt().with_env_filter(filter).init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{JobIdValidationError, JobStatus};
    use std::{
        net::TcpListener,
        sync::atomic::{AtomicBool, AtomicUsize},
    };

    #[derive(Clone)]
    struct MockEngineClient {
        fail_with_5xx: Arc<AtomicBool>,
        delay: Duration,
        inflight: Arc<AtomicUsize>,
        max_inflight: Arc<AtomicUsize>,
    }

    impl EngineClient for MockEngineClient {
        fn call(&self, _engine: EngineKind, payload: Value) -> EngineFuture<'_> {
            Box::pin(async move {
                let now = self.inflight.fetch_add(1, Ordering::SeqCst) + 1;
                self.max_inflight.fetch_max(now, Ordering::SeqCst);
                if self.delay > Duration::ZERO {
                    tokio::time::sleep(self.delay).await;
                }
                if self.fail_with_5xx.load(Ordering::Relaxed) {
                    self.inflight.fetch_sub(1, Ordering::SeqCst);
                    return Err(EngineError {
                        code: "engine_upstream_5xx",
                        message: "mock engine 5xx".to_string(),
                        retryable: true,
                    });
                }
                self.inflight.fetch_sub(1, Ordering::SeqCst);
                Ok(EngineResult {
                    payload: json!({"ok": true, "echo": payload}),
                })
            })
        }
    }

    fn build_state(
        queue_capacity: usize,
        concurrency: usize,
        client: Arc<dyn EngineClient>,
    ) -> AppState {
        let config = AppConfig {
            host: "0.0.0.0".to_string(),
            api_port: 18_000,
            stt_queue_capacity: queue_capacity,
            summarize_queue_capacity: queue_capacity,
            embed_queue_capacity: queue_capacity,
            stt_concurrency: concurrency,
            summarize_concurrency: concurrency,
            embed_concurrency: concurrency,
            engine_timeout_secs: 60,
            job_timeout_secs: 2,
            job_timeout_min_secs: 1,
            job_timeout_max_secs: 30,
            stt_timeout_per_audio_sec_ms: 1_500,
            stt_timeout_buffer_ms: 5_000,
            engine_connect_timeout_ms: 50,
            engine_retry_count: 0,
            engine_base_backoff_ms: 1,
            stt_engine_url: "http://127.0.0.1:18103/infer".to_string(),
            summarize_engine_url: "http://127.0.0.1:18101/infer".to_string(),
            embed_engine_url: "http://127.0.0.1:18102/infer".to_string(),
            engine_supervision_enabled: false,
            stt_engine_command: "whisper-server".to_string(),
            stt_engine_args: vec![],
            summarize_engine_command: "llama-text".to_string(),
            summarize_engine_args: vec![],
            embed_engine_command: "llama-embed".to_string(),
            embed_engine_args: vec![],
            engine_startup_timeout_secs: 20,
            engine_readiness_poll_ms: 200,
            engine_shutdown_grace_secs: 1,
            engine_restart_backoff_base_ms: 100,
            engine_restart_backoff_max_ms: 500,
        };

        let jobs = Arc::new(JobStore::new());
        let dispatchers = build_dispatchers(&config, jobs.clone(), client);

        AppState {
            ready: true,
            degraded: Arc::new(AtomicBool::new(false)),
            config: Arc::new(config),
            jobs,
            dispatchers: Arc::new(dispatchers),
            rejection_metrics: Arc::new(RejectionMetrics::new()),
        }
    }

    fn build_state_with_job_timeout(
        queue_capacity: usize,
        concurrency: usize,
        client: Arc<dyn EngineClient>,
        job_timeout_secs: u64,
    ) -> AppState {
        let config = AppConfig {
            host: "0.0.0.0".to_string(),
            api_port: 18_000,
            stt_queue_capacity: queue_capacity,
            summarize_queue_capacity: queue_capacity,
            embed_queue_capacity: queue_capacity,
            stt_concurrency: concurrency,
            summarize_concurrency: concurrency,
            embed_concurrency: concurrency,
            engine_timeout_secs: 60,
            job_timeout_secs,
            job_timeout_min_secs: 0,
            job_timeout_max_secs: 30,
            stt_timeout_per_audio_sec_ms: 1_500,
            stt_timeout_buffer_ms: 5_000,
            engine_connect_timeout_ms: 50,
            engine_retry_count: 0,
            engine_base_backoff_ms: 1,
            stt_engine_url: "http://127.0.0.1:18103/infer".to_string(),
            summarize_engine_url: "http://127.0.0.1:18101/infer".to_string(),
            embed_engine_url: "http://127.0.0.1:18102/infer".to_string(),
            engine_supervision_enabled: false,
            stt_engine_command: "whisper-server".to_string(),
            stt_engine_args: vec![],
            summarize_engine_command: "llama-text".to_string(),
            summarize_engine_args: vec![],
            embed_engine_command: "llama-embed".to_string(),
            embed_engine_args: vec![],
            engine_startup_timeout_secs: 20,
            engine_readiness_poll_ms: 200,
            engine_shutdown_grace_secs: 1,
            engine_restart_backoff_base_ms: 100,
            engine_restart_backoff_max_ms: 500,
        };

        let jobs = Arc::new(JobStore::new());
        let dispatchers = build_dispatchers(&config, jobs.clone(), client);

        AppState {
            ready: true,
            degraded: Arc::new(AtomicBool::new(false)),
            config: Arc::new(config),
            jobs,
            dispatchers: Arc::new(dispatchers),
            rejection_metrics: Arc::new(RejectionMetrics::new()),
        }
    }

    #[tokio::test]
    async fn healthz_route_returns_ok() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::ZERO,
            inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight: Arc::new(AtomicUsize::new(0)),
        });
        let response = route_request("GET /healthz HTTP/1.1", &build_state(8, 1, client));
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("{\"status\":\"ok\"}"));
    }

    #[tokio::test]
    async fn readyz_returns_degraded_when_flag_is_set() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::ZERO,
            inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight: Arc::new(AtomicUsize::new(0)),
        });
        let state = build_state(8, 1, client);
        state.degraded.store(true, Ordering::Relaxed);

        let response = route_request("GET /readyz HTTP/1.1", &state);
        assert!(response.starts_with("HTTP/1.1 503 Service Unavailable"));
        assert!(response.contains("{\"status\":\"degraded\"}"));
    }

    #[tokio::test]
    async fn metrics_route_returns_queue_and_rejection_metrics() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::from_millis(200),
            inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight: Arc::new(AtomicUsize::new(0)),
        });
        let state = build_state(1, 1, client);

        let _ = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        let _ = route_request("POST /jobs?engine=stt HTTP/1.1", &state);

        let response = route_request("GET /metrics HTTP/1.1", &state);
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("\"readiness\""));
        assert!(response.contains("\"engines\""));
        assert!(response.contains("\"rejections\""));
        assert!(response.contains("\"engine\":\"stt\""));
    }

    #[tokio::test]
    async fn post_jobs_transitions_to_succeeded() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::from_millis(10),
            inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight: Arc::new(AtomicUsize::new(0)),
        });
        let state = build_state(8, 1, client);

        let response = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        let created_job_id = extract_job_id(&response);

        wait_for_status(&state, &created_job_id, JobStatus::Completed).await;

        let get_response = route_request(&format!("GET /jobs/{created_job_id} HTTP/1.1"), &state);
        assert!(get_response.starts_with("HTTP/1.1 200 OK"));
        assert!(get_response.contains("\"status\":\"completed\""));
        assert!(get_response.contains("\"result\""));
    }

    #[tokio::test]
    async fn stt_payload_declares_rust_audio_preprocessing_contract() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::from_millis(10),
            inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight: Arc::new(AtomicUsize::new(0)),
        });
        let state = build_state(8, 1, client);

        let response = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        let created_job_id = extract_job_id(&response);

        wait_for_status(&state, &created_job_id, JobStatus::Completed).await;

        let get_response = route_request(&format!("GET /jobs/{created_job_id} HTTP/1.1"), &state);
        assert!(get_response.contains("\"audio_contract\""));
        assert!(get_response.contains("\"normalized_by\":\"recordroute_symphonia\""));
        assert!(get_response.contains("\"conversion_required\":true"));
        assert!(get_response.contains("\"normalized_bytes\":0"));
    }

    #[tokio::test]
    async fn post_jobs_rejects_invalid_stt_audio_payload() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::from_millis(10),
            inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight: Arc::new(AtomicUsize::new(0)),
        });
        let state = build_state(8, 1, client);

        let response =
            route_request_with_body("POST /jobs?engine=stt HTTP/1.1", b"not-audio", &state);

        assert!(response.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(response.contains("\"code\":\"invalid_audio_payload\""));
    }

    #[tokio::test]
    async fn post_jobs_transitions_to_timeout_on_job_timeout() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::from_millis(30),
            inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight: Arc::new(AtomicUsize::new(0)),
        });
        let state = build_state_with_job_timeout(8, 1, client, 0);

        let response = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        let created_job_id = extract_job_id(&response);

        wait_for_status(&state, &created_job_id, JobStatus::Timeout).await;

        let get_response = route_request(&format!("GET /jobs/{created_job_id} HTTP/1.1"), &state);
        assert!(get_response.contains("\"status\":\"timeout\""));
        assert!(get_response.contains("\"code\":\"job_timeout\""));
    }

    #[tokio::test]
    async fn post_jobs_transitions_to_failed_on_engine_error() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(true)),
            delay: Duration::from_millis(5),
            inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight: Arc::new(AtomicUsize::new(0)),
        });
        let state = build_state(8, 1, client);

        let response = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        let created_job_id = extract_job_id(&response);

        wait_for_status(&state, &created_job_id, JobStatus::Failed).await;

        let get_response = route_request(&format!("GET /jobs/{created_job_id} HTTP/1.1"), &state);
        assert!(get_response.contains("\"status\":\"failed\""));
        assert!(get_response.contains("\"code\":\"engine_upstream_5xx\""));
    }

    #[tokio::test]
    async fn capacity_rejection_returns_429_and_records_rejected_job() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::from_millis(200),
            inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight: Arc::new(AtomicUsize::new(0)),
        });
        let state = build_state(1, 1, client);

        let first = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        let second = route_request("POST /jobs?engine=stt HTTP/1.1", &state);

        assert!(first.starts_with("HTTP/1.1 202 Accepted"));
        assert!(second.starts_with("HTTP/1.1 429 Too Many Requests"));
        let reason = if second.contains("\"code\":\"engine_full\"") {
            "engine_full"
        } else {
            assert!(second.contains("\"code\":\"queue_full\""));
            "queue_full"
        };

        assert_eq!(find_first_rejected_error_code(&state), Some(reason));
        assert_eq!(state.rejection_metrics.count(EngineKind::Stt, reason), 1);

        let rejected = find_status_count(&state, JobStatus::Rejected);
        assert!(rejected >= 1);
    }

    #[tokio::test]
    async fn queue_is_drained_and_accepts_new_jobs_again() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::from_millis(40),
            inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight: Arc::new(AtomicUsize::new(0)),
        });
        let state = build_state(1, 1, client);

        let first = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        assert!(first.starts_with("HTTP/1.1 202 Accepted"));

        let second = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        assert!(second.starts_with("HTTP/1.1 429 Too Many Requests"));

        let first_job_id = extract_job_id(&first);
        wait_for_status(&state, &first_job_id, JobStatus::Completed).await;

        let third = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        assert!(third.starts_with("HTTP/1.1 202 Accepted"));
    }

    #[test]
    fn full_rejection_reason_distinguishes_queue_vs_engine_pressure() {
        let jobs = JobStore::new();
        let queued_job = jobs.create_queued_job(EngineKind::Stt);

        let queue_pressure_dispatcher = EngineDispatcher {
            sender: tokio::sync::mpsc::channel(1).0,
            queue_depth: Arc::new(AtomicUsize::new(1)),
            queue_capacity: 1,
            worker_count: 2,
        };
        let queue_reason =
            rejection_reason_for_full(EngineKind::Stt, &queue_pressure_dispatcher, &jobs);
        assert_eq!(queue_reason, "queue_full");

        jobs.mark_running(&queued_job.job_id);
        let engine_pressure_dispatcher = EngineDispatcher {
            sender: tokio::sync::mpsc::channel(1).0,
            queue_depth: Arc::new(AtomicUsize::new(1)),
            queue_capacity: 1,
            worker_count: 1,
        };
        let engine_reason =
            rejection_reason_for_full(EngineKind::Stt, &engine_pressure_dispatcher, &jobs);
        assert_eq!(engine_reason, "engine_full");
    }

    #[tokio::test]
    async fn inflight_never_exceeds_worker_concurrency() {
        let max_inflight = Arc::new(AtomicUsize::new(0));
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::from_millis(60),
            inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight: max_inflight.clone(),
        });
        let state = build_state(16, 2, client);

        let mut ids = Vec::new();
        for _ in 0..8 {
            let response = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
            assert!(response.starts_with("HTTP/1.1 202 Accepted"));
            ids.push(extract_job_id(&response));
        }

        for id in ids {
            wait_for_status(&state, &id, JobStatus::Completed).await;
        }

        assert!(max_inflight.load(Ordering::SeqCst) <= 2);
    }

    #[tokio::test]
    async fn queue_depth_gauge_returns_to_zero_after_processing() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::from_millis(20),
            inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight: Arc::new(AtomicUsize::new(0)),
        });
        let state = build_state(8, 1, client);

        let mut ids = Vec::new();
        for _ in 0..3 {
            let response = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
            ids.push(extract_job_id(&response));
        }
        for id in ids {
            wait_for_status(&state, &id, JobStatus::Completed).await;
        }

        let depth = state
            .dispatchers
            .get(&EngineKind::Stt)
            .expect("stt dispatcher")
            .queue_depth
            .load(Ordering::Relaxed);
        assert_eq!(depth, 0);
    }

    #[tokio::test]
    async fn get_job_rejects_invalid_job_id() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::ZERO,
            inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight: Arc::new(AtomicUsize::new(0)),
        });
        let state = build_state(8, 1, client);

        let response = route_request("GET /jobs/bad*id HTTP/1.1", &state);
        assert!(response.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(response.contains("\"code\":\"invalid_job_id\""));
    }

    #[test]
    fn job_id_parse_validation_rules() {
        assert!(JobId::parse("job-abc_123").is_ok());
        assert!(matches!(JobId::parse(""), Err(JobIdValidationError::Empty)));
        assert!(matches!(
            JobId::parse(&"a".repeat(65)),
            Err(JobIdValidationError::TooLong)
        ));
        assert!(matches!(
            JobId::parse("bad/id"),
            Err(JobIdValidationError::InvalidCharacter)
        ));
    }

    #[test]
    fn call_engine_http_times_out_on_connect() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let result = runtime.block_on(call_engine_http(
            "http://10.255.255.1:81/infer",
            &json!({"ping": true}),
            Duration::from_millis(20),
            Duration::from_millis(40),
        ));
        let error = result.expect_err("connect should timeout or fail");
        assert!(matches!(
            error.code,
            "engine_connect_timeout" | "engine_transport_error"
        ));
    }

    #[test]
    fn call_engine_http_times_out_on_stalled_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener bind");
        let addr = listener.local_addr().expect("listener local addr");
        let handle = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("accept connection");
            let mut buf = [0_u8; 512];
            let _ = socket.read(&mut buf);
            std::thread::sleep(Duration::from_millis(120));
        });

        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let result = runtime.block_on(call_engine_http(
            &format!("http://{}/infer", addr),
            &json!({"ping": true}),
            Duration::from_millis(30),
            Duration::from_millis(40),
        ));
        let error = result.expect_err("read should timeout");
        assert_eq!(error.code, "engine_request_timeout");

        handle.join().expect("join listener thread");
    }

    #[tokio::test]
    async fn job_store_mutex_contention_smoke_measurement() {
        let store = Arc::new(JobStore::new());
        let start = std::time::Instant::now();

        let mut tasks = Vec::new();
        for _ in 0..32 {
            let store = store.clone();
            tasks.push(tokio::spawn(async move {
                for _ in 0..200 {
                    let job = store.create_queued_job(EngineKind::Stt);
                    store.mark_running(&job.job_id);
                    store.mark_succeeded(&job.job_id, json!({"ok": true}));
                }
            }));
        }

        for task in tasks {
            task.await.expect("contention task should complete");
        }

        let elapsed_ms = start.elapsed().as_millis();
        println!("job store contention smoke: {} ms", elapsed_ms);

        let completed = find_status_count(
            &AppState {
                ready: true,
                degraded: Arc::new(AtomicBool::new(false)),
                config: Arc::new(AppConfig::from_env()),
                jobs: store,
                dispatchers: Arc::new(HashMap::new()),
                rejection_metrics: Arc::new(RejectionMetrics::new()),
            },
            JobStatus::Completed,
        );
        assert_eq!(completed, 32 * 200);
    }

    #[tokio::test]
    async fn timeout_budget_formula_applies_for_stt_audio_length() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::ZERO,
            inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight: Arc::new(AtomicUsize::new(0)),
        });
        let state = build_state(8, 1, client);

        let budget =
            derive_job_timeout_budget(EngineKind::Stt, Some("audio_ms=10000"), &state.config);
        assert_eq!(budget, Duration::from_secs(20));
    }

    #[tokio::test]
    async fn timeout_budget_is_clamped_to_max() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::ZERO,
            inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight: Arc::new(AtomicUsize::new(0)),
        });
        let state = build_state(8, 1, client);

        let budget =
            derive_job_timeout_budget(EngineKind::Stt, Some("audio_ms=999999999"), &state.config);
        assert_eq!(budget, Duration::from_secs(30));
    }

    async fn wait_for_status(state: &AppState, job_id: &str, target: JobStatus) {
        let job_id = JobId::parse(job_id).expect("valid generated job id");
        for _ in 0..80 {
            if let Some(job) = state.jobs.get_job(&job_id) {
                if job.status == target {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        panic!("job {job_id} did not transition to target status");
    }

    fn find_status_count(state: &AppState, target: JobStatus) -> usize {
        state
            .jobs
            .jobs
            .lock()
            .expect("job store mutex poisoned")
            .values()
            .filter(|job| job.status == target)
            .count()
    }

    fn find_first_rejected_error_code(state: &AppState) -> Option<&'static str> {
        state
            .jobs
            .jobs
            .lock()
            .expect("job store mutex poisoned")
            .values()
            .find(|job| job.status == JobStatus::Rejected)
            .and_then(|job| job.error.as_ref().map(|error| error.code))
    }

    fn extract_job_id(response: &str) -> String {
        let marker = "\"job_id\":\"";
        let start = response.find(marker).expect("response includes job_id") + marker.len();
        let end = response[start..]
            .find('"')
            .expect("job_id closing quote found")
            + start;
        response[start..end].to_string()
    }
}
