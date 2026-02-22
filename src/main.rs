use std::{
    collections::HashMap,
    env,
    future::Future,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    pin::Pin,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::{mpsc, Semaphore};
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Clone)]
struct AppState {
    ready: bool,
    jobs: Arc<JobStore>,
    dispatchers: Arc<HashMap<EngineKind, EngineDispatcher>>,
}

struct EngineDispatcher {
    sender: mpsc::Sender<JobRequest>,
    queue_depth: Arc<AtomicUsize>,
}

#[derive(Clone, Debug)]
struct JobRequest {
    job_id: String,
    engine: EngineKind,
    payload: Value,
}

#[derive(Debug)]
struct JobStore {
    next_id: AtomicU64,
    jobs: Mutex<HashMap<String, Job>>,
}

#[derive(Clone, Debug)]
struct AppConfig {
    host: String,
    api_port: u16,
    stt_queue_capacity: usize,
    summarize_queue_capacity: usize,
    embed_queue_capacity: usize,
    stt_concurrency: usize,
    summarize_concurrency: usize,
    embed_concurrency: usize,
    engine_timeout_secs: u64,
    engine_retry_count: usize,
    engine_base_backoff_ms: u64,
    stt_engine_url: String,
    summarize_engine_url: String,
    embed_engine_url: String,
}

#[derive(Clone, Debug, Serialize)]
struct Job {
    job_id: String,
    status: JobStatus,
    engine: EngineKind,
    created_at_ms: u64,
    started_at_ms: Option<u64>,
    finished_at_ms: Option<u64>,
    result: Option<Value>,
    error: Option<JobError>,
}

#[derive(Clone, Debug, Serialize)]
struct JobError {
    code: &'static str,
    message: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum EngineKind {
    Stt,
    Summarize,
    Embed,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Rejected,
}

#[derive(Debug)]
enum QueueEnqueueError {
    Full,
    Closed,
}

#[derive(Debug)]
struct EngineResult {
    payload: Value,
}

#[derive(Debug)]
struct EngineError {
    code: &'static str,
    message: String,
    retryable: bool,
}

type EngineFuture<'a> =
    Pin<Box<dyn Future<Output = Result<EngineResult, EngineError>> + Send + 'a>>;

trait EngineClient: Send + Sync {
    fn call(&self, engine: EngineKind, payload: Value) -> EngineFuture<'_>;
}

#[derive(Clone)]
struct HttpEngineClient {
    endpoints: HashMap<EngineKind, String>,
    timeout: Duration,
    retry_count: usize,
    base_backoff: Duration,
}

impl AppConfig {
    fn from_env() -> Self {
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
            engine_retry_count: read_usize_env("RECORDROUTE_ENGINE_RETRY_COUNT", 1),
            engine_base_backoff_ms: read_u64_env("RECORDROUTE_ENGINE_BACKOFF_MS", 200),
            stt_engine_url: env::var("RECORDROUTE_STT_ENGINE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:18103/infer".to_string()),
            summarize_engine_url: env::var("RECORDROUTE_SUMMARIZE_ENGINE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:18101/infer".to_string()),
            embed_engine_url: env::var("RECORDROUTE_EMBED_ENGINE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:18102/infer".to_string()),
        }
    }

    fn api_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.host, self.api_port).parse()
    }

    fn queue_capacity(&self, engine: EngineKind) -> usize {
        match engine {
            EngineKind::Stt => self.stt_queue_capacity,
            EngineKind::Summarize => self.summarize_queue_capacity,
            EngineKind::Embed => self.embed_queue_capacity,
        }
    }

    fn concurrency(&self, engine: EngineKind) -> usize {
        match engine {
            EngineKind::Stt => self.stt_concurrency,
            EngineKind::Summarize => self.summarize_concurrency,
            EngineKind::Embed => self.embed_concurrency,
        }
    }
}

impl JobStore {
    fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            jobs: Mutex::new(HashMap::new()),
        }
    }

    fn create_queued_job(&self, engine: EngineKind) -> Job {
        let job_number = self.next_id.fetch_add(1, Ordering::Relaxed);
        let job_id = format!("job-{job_number:010}");
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

    fn get_job(&self, job_id: &str) -> Option<Job> {
        self.jobs
            .lock()
            .expect("job store mutex poisoned")
            .get(job_id)
            .cloned()
    }

    fn mark_running(&self, job_id: &str) {
        self.transition(job_id, |job| {
            if job.status == JobStatus::Queued {
                job.status = JobStatus::Running;
                job.started_at_ms = Some(now_ms());
            }
        });
    }

    fn mark_succeeded(&self, job_id: &str, result: Value) {
        self.transition(job_id, |job| {
            if job.status == JobStatus::Running {
                job.status = JobStatus::Succeeded;
                job.result = Some(result.clone());
                job.finished_at_ms = Some(now_ms());
            }
        });
    }

    fn mark_failed(&self, job_id: &str, code: &'static str, message: String) {
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

    fn mark_rejected_queue_full(&self, job_id: &str, engine: EngineKind) {
        self.transition(job_id, |job| {
            if job.status == JobStatus::Queued {
                job.status = JobStatus::Rejected;
                job.finished_at_ms = Some(now_ms());
                job.error = Some(JobError {
                    code: "queue_full",
                    message: format!("{} queue is full", engine.as_str()),
                });
            }
        });
    }

    fn transition<F>(&self, job_id: &str, mut updater: F)
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

impl EngineKind {
    fn as_str(self) -> &'static str {
        match self {
            EngineKind::Stt => "stt",
            EngineKind::Summarize => "summarize",
            EngineKind::Embed => "embed",
        }
    }
}

impl EngineDispatcher {
    fn enqueue(&self, request: JobRequest) -> Result<(), QueueEnqueueError> {
        match self.sender.try_send(request) {
            Ok(()) => {
                self.queue_depth.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => Err(QueueEnqueueError::Full),
            Err(mpsc::error::TrySendError::Closed(_)) => Err(QueueEnqueueError::Closed),
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
                match call_engine_http(&endpoint, &payload, self.timeout).await {
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
        timeout: Duration::from_secs(config.engine_timeout_secs),
        retry_count: config.engine_retry_count,
        base_backoff: Duration::from_millis(config.engine_base_backoff_ms),
    });

    let dispatchers = build_dispatchers(&config, jobs.clone(), engine_client);

    let state = AppState {
        ready: true,
        jobs,
        dispatchers: Arc::new(dispatchers),
    };

    tracing::info!(%addr, "starting RecordRoute API server");

    tokio::task::spawn_blocking(move || run_blocking_server(addr, state)).await??;
    Ok(())
}

fn build_dispatchers(
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
            queue_depth.clone(),
            semaphore,
            jobs.clone(),
            engine_client.clone(),
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

fn spawn_worker_loop(
    engine: EngineKind,
    mut receiver: mpsc::Receiver<JobRequest>,
    queue_depth: Arc<AtomicUsize>,
    semaphore: Arc<Semaphore>,
    jobs: Arc<JobStore>,
    engine_client: Arc<dyn EngineClient>,
) {
    tokio::spawn(async move {
        while let Some(request) = receiver.recv().await {
            queue_depth.fetch_sub(1, Ordering::Relaxed);

            let semaphore = semaphore.clone();
            let jobs = jobs.clone();
            let engine_client = engine_client.clone();

            tokio::spawn(async move {
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .expect("engine semaphore closed unexpectedly");

                let start = std::time::Instant::now();
                jobs.mark_running(&request.job_id);
                tracing::info!(
                    job_id = %request.job_id,
                    engine = engine.as_str(),
                    status_transition = "queued->running",
                    "job started"
                );

                match engine_client.call(request.engine, request.payload).await {
                    Ok(result) => {
                        let latency_ms = start.elapsed().as_millis() as u64;
                        jobs.mark_succeeded(&request.job_id, result.payload);
                        tracing::info!(
                            job_id = %request.job_id,
                            engine = engine.as_str(),
                            status_transition = "running->succeeded",
                            latency_ms,
                            "job completed"
                        );
                    }
                    Err(error) => {
                        let latency_ms = start.elapsed().as_millis() as u64;
                        jobs.mark_failed(&request.job_id, error.code, error.message.clone());
                        tracing::warn!(
                            job_id = %request.job_id,
                            engine = engine.as_str(),
                            status_transition = "running->failed",
                            latency_ms,
                            retryable = error.retryable,
                            error_code = error.code,
                            error_message = %error.message,
                            "job failed"
                        );
                    }
                }
            });
        }
    });
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
    let response = route_request(first_line, state);

    socket.write_all(response.as_bytes())?;
    socket.flush()?;

    Ok(())
}

fn route_request(request_line: &str, state: &AppState) -> String {
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path_with_query = parts.next().unwrap_or_default();
    let (path, query) = parse_path_and_query(path_with_query);

    match (method, path) {
        ("GET", "/healthz") => json_response(200, &StatusBody { status: "ok" }),
        ("GET", "/readyz") => {
            if state.ready {
                json_response(200, &StatusBody { status: "ready" })
            } else {
                json_response(
                    503,
                    &StatusBody {
                        status: "not_ready",
                    },
                )
            }
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
            let request = JobRequest {
                job_id: job.job_id.clone(),
                engine,
                payload: json!({
                    "job_id": job.job_id,
                    "engine": engine.as_str(),
                }),
            };

            match dispatcher.enqueue(request) {
                Ok(()) => json_response(
                    202,
                    &JobCreatedResponse {
                        job_id: &job.job_id,
                    },
                ),
                Err(QueueEnqueueError::Full) => {
                    state.jobs.mark_rejected_queue_full(&job.job_id, engine);
                    json_error_response(
                        429,
                        "queue_full",
                        &format!("{} queue is full", engine.as_str()),
                    )
                }
                Err(QueueEnqueueError::Closed) => {
                    state.jobs.mark_failed(
                        &job.job_id,
                        "engine_dispatcher_closed",
                        "engine dispatcher closed".to_string(),
                    );
                    json_error_response(503, "engine_dispatcher_closed", "engine dispatcher closed")
                }
            }
        }
        ("GET", path) if path.starts_with("/jobs/") => {
            let job_id = path.trim_start_matches("/jobs/");
            match state.jobs.get_job(job_id) {
                Some(job) => json_response(200, &job),
                None => json_error_response(404, "job_not_found", "job not found"),
            }
        }
        ("GET", _) => json_error_response(404, "not_found", "not found"),
        _ => json_error_response(405, "method_not_allowed", "method not allowed"),
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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64
}

async fn sleep_backoff(base: Duration, attempt: usize) {
    let factor = 2u32.saturating_pow(attempt as u32);
    tokio::time::sleep(base * factor).await;
}

async fn call_engine_http(
    endpoint: &str,
    payload: &Value,
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

        let mut stream = TcpStream::connect((parsed.host.as_str(), parsed.port)).map_err(|error| {
            EngineError {
                code: "engine_transport_error",
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
            let timed_out = error.kind() == std::io::ErrorKind::TimedOut;
            EngineError {
                code: if timed_out {
                    "engine_timeout"
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
    fmt().with_env_filter(filter).init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[derive(Clone)]
    struct MockEngineClient {
        fail_with_5xx: Arc<AtomicBool>,
        delay: Duration,
    }

    impl EngineClient for MockEngineClient {
        fn call(&self, _engine: EngineKind, payload: Value) -> EngineFuture<'_> {
            Box::pin(async move {
                if self.delay > Duration::ZERO {
                    tokio::time::sleep(self.delay).await;
                }
                if self.fail_with_5xx.load(Ordering::Relaxed) {
                    return Err(EngineError {
                        code: "engine_upstream_5xx",
                        message: "mock engine 5xx".to_string(),
                        retryable: true,
                    });
                }
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
            engine_retry_count: 0,
            engine_base_backoff_ms: 1,
            stt_engine_url: "http://127.0.0.1:18103/infer".to_string(),
            summarize_engine_url: "http://127.0.0.1:18101/infer".to_string(),
            embed_engine_url: "http://127.0.0.1:18102/infer".to_string(),
        };

        let jobs = Arc::new(JobStore::new());
        let dispatchers = build_dispatchers(&config, jobs.clone(), client);

        AppState {
            ready: true,
            jobs,
            dispatchers: Arc::new(dispatchers),
        }
    }

    #[tokio::test]
    async fn healthz_route_returns_ok() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::ZERO,
        });
        let response = route_request("GET /healthz HTTP/1.1", &build_state(8, 1, client));
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("{\"status\":\"ok\"}"));
    }

    #[tokio::test]
    async fn post_jobs_transitions_to_succeeded() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::from_millis(10),
        });
        let state = build_state(8, 1, client);

        let response = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        let created_job_id = extract_job_id(&response);

        wait_for_status(&state, &created_job_id, JobStatus::Succeeded).await;

        let get_response = route_request(&format!("GET /jobs/{created_job_id} HTTP/1.1"), &state);
        assert!(get_response.starts_with("HTTP/1.1 200 OK"));
        assert!(get_response.contains("\"status\":\"succeeded\""));
        assert!(get_response.contains("\"result\""));
    }

    #[tokio::test]
    async fn post_jobs_transitions_to_failed_on_engine_error() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(true)),
            delay: Duration::from_millis(5),
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
    async fn queue_full_returns_429_and_records_rejected_job() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::from_millis(200),
        });
        let state = build_state(1, 1, client);

        let first = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        let second = route_request("POST /jobs?engine=stt HTTP/1.1", &state);

        assert!(first.starts_with("HTTP/1.1 202 Accepted"));
        assert!(second.starts_with("HTTP/1.1 429 Too Many Requests"));
        assert!(second.contains("\"code\":\"queue_full\""));

        let rejected = find_status_count(&state, JobStatus::Rejected);
        assert!(rejected >= 1);
    }

    #[tokio::test]
    async fn queue_is_drained_and_accepts_new_jobs_again() {
        let client = Arc::new(MockEngineClient {
            fail_with_5xx: Arc::new(AtomicBool::new(false)),
            delay: Duration::from_millis(40),
        });
        let state = build_state(1, 1, client);

        let first = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        assert!(first.starts_with("HTTP/1.1 202 Accepted"));

        let second = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        assert!(second.starts_with("HTTP/1.1 429 Too Many Requests"));

        let first_job_id = extract_job_id(&first);
        wait_for_status(&state, &first_job_id, JobStatus::Succeeded).await;

        let third = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        assert!(third.starts_with("HTTP/1.1 202 Accepted"));
    }

    async fn wait_for_status(state: &AppState, job_id: &str, target: JobStatus) {
        for _ in 0..80 {
            if let Some(job) = state.jobs.get_job(job_id) {
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
