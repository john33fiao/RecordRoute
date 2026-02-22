use std::{
    collections::HashMap,
    env,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use serde::Serialize;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Clone, Debug)]
struct AppState {
    ready: bool,
    jobs: Arc<JobStore>,
    queues: Arc<EngineQueueStore>,
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
}

#[derive(Clone, Debug, Serialize)]
struct Job {
    job_id: String,
    status: &'static str,
    engine: EngineKind,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum EngineKind {
    Stt,
    Summarize,
    Embed,
}

#[derive(Debug)]
struct EngineQueueStore {
    capacities: HashMap<EngineKind, usize>,
    depths: Mutex<HashMap<EngineKind, usize>>,
}

#[derive(Debug)]
enum QueueEnqueueError {
    Full { engine: EngineKind },
}

impl AppConfig {
    fn from_env() -> Self {
        let host = env::var("RECORDROUTE_API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let api_port = env::var("RECORDROUTE_API_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(18_000);
        let stt_queue_capacity = read_usize_env("RECORDROUTE_STT_QUEUE_CAPACITY", 1_024);
        let summarize_queue_capacity =
            read_usize_env("RECORDROUTE_SUMMARIZE_QUEUE_CAPACITY", 1_024);
        let embed_queue_capacity = read_usize_env("RECORDROUTE_EMBED_QUEUE_CAPACITY", 1_024);

        Self {
            host,
            api_port,
            stt_queue_capacity,
            summarize_queue_capacity,
            embed_queue_capacity,
        }
    }

    fn api_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.host, self.api_port).parse()
    }
}

impl JobStore {
    fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            jobs: Mutex::new(HashMap::new()),
        }
    }

    fn create_job(&self, engine: EngineKind) -> Job {
        let job_number = self.next_id.fetch_add(1, Ordering::Relaxed);
        let job_id = format!("job-{job_number:010}");
        let job = Job {
            job_id: job_id.clone(),
            status: "queued",
            engine,
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
}

impl EngineQueueStore {
    fn new(config: &AppConfig) -> Self {
        let capacities = HashMap::from([
            (EngineKind::Stt, config.stt_queue_capacity),
            (EngineKind::Summarize, config.summarize_queue_capacity),
            (EngineKind::Embed, config.embed_queue_capacity),
        ]);
        let depths = Mutex::new(HashMap::from([
            (EngineKind::Stt, 0),
            (EngineKind::Summarize, 0),
            (EngineKind::Embed, 0),
        ]));

        Self { capacities, depths }
    }

    fn enqueue(&self, engine: EngineKind) -> Result<(), QueueEnqueueError> {
        let capacity = *self.capacities.get(&engine).unwrap_or(&0);
        let mut depths = self.depths.lock().expect("queue depth mutex poisoned");
        let depth = depths.entry(engine).or_insert(0);

        if *depth >= capacity {
            return Err(QueueEnqueueError::Full { engine });
        }

        *depth += 1;
        Ok(())
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
    let state = AppState {
        ready: true,
        jobs: Arc::new(JobStore::new()),
        queues: Arc::new(EngineQueueStore::new(&config)),
    };

    tracing::info!(%addr, "starting RecordRoute API server");

    tokio::task::spawn_blocking(move || run_blocking_server(addr, state)).await??;
    Ok(())
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

            match state.queues.enqueue(engine) {
                Ok(()) => {
                    let job = state.jobs.create_job(engine);
                    json_response(
                        202,
                        &JobCreatedResponse {
                            job_id: &job.job_id,
                        },
                    )
                }
                Err(QueueEnqueueError::Full { engine }) => json_error_response(
                    429,
                    "queue_full",
                    match engine {
                        EngineKind::Stt => "stt queue is full",
                        EngineKind::Summarize => "summarize queue is full",
                        EngineKind::Embed => "embed queue is full",
                    },
                ),
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

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_state(ready: bool) -> AppState {
        let config = AppConfig {
            host: "0.0.0.0".to_string(),
            api_port: 18_000,
            stt_queue_capacity: 1_024,
            summarize_queue_capacity: 1_024,
            embed_queue_capacity: 1_024,
        };

        AppState {
            ready,
            jobs: Arc::new(JobStore::new()),
            queues: Arc::new(EngineQueueStore::new(&config)),
        }
    }

    #[test]
    fn healthz_route_returns_ok() {
        let response = route_request("GET /healthz HTTP/1.1", &build_state(true));
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("{\"status\":\"ok\"}"));
    }

    #[test]
    fn readyz_route_returns_service_unavailable_when_not_ready() {
        let response = route_request("GET /readyz HTTP/1.1", &build_state(false));
        assert!(response.starts_with("HTTP/1.1 503 Service Unavailable"));
        assert!(response.contains("{\"status\":\"not_ready\"}"));
    }

    #[test]
    fn post_jobs_returns_accepted_and_job_id() {
        let state = build_state(true);
        let response = route_request("POST /jobs HTTP/1.1", &state);

        assert!(response.starts_with("HTTP/1.1 202 Accepted"));
        assert!(response.contains("\"job_id\":\"job-"));
    }

    #[test]
    fn post_jobs_honors_per_engine_queue_capacity() {
        let state = AppState {
            ready: true,
            jobs: Arc::new(JobStore::new()),
            queues: Arc::new(EngineQueueStore {
                capacities: HashMap::from([
                    (EngineKind::Stt, 1),
                    (EngineKind::Summarize, 1),
                    (EngineKind::Embed, 1),
                ]),
                depths: Mutex::new(HashMap::from([
                    (EngineKind::Stt, 0),
                    (EngineKind::Summarize, 0),
                    (EngineKind::Embed, 0),
                ])),
            }),
        };

        let first_stt = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        let second_stt = route_request("POST /jobs?engine=stt HTTP/1.1", &state);
        let first_embed = route_request("POST /jobs?engine=embed HTTP/1.1", &state);

        assert!(first_stt.starts_with("HTTP/1.1 202 Accepted"));
        assert!(second_stt.starts_with("HTTP/1.1 429 Too Many Requests"));
        assert!(second_stt.contains("\"code\":\"queue_full\""));
        assert!(first_embed.starts_with("HTTP/1.1 202 Accepted"));
    }

    #[test]
    fn get_job_returns_existing_job() {
        let state = build_state(true);
        let create_response = route_request("POST /jobs HTTP/1.1", &state);
        let created_job_id = extract_job_id(&create_response);

        let response = route_request(&format!("GET /jobs/{created_job_id} HTTP/1.1"), &state);

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains(&format!("\"job_id\":\"{created_job_id}\"")));
        assert!(response.contains("\"status\":\"queued\""));
    }

    #[test]
    fn get_missing_job_returns_not_found() {
        let response = route_request("GET /jobs/job-0000000001 HTTP/1.1", &build_state(true));
        assert!(response.starts_with("HTTP/1.1 404 Not Found"));
        assert!(response.contains("\"code\":\"job_not_found\""));
    }

    #[test]
    fn unknown_get_route_returns_json_error() {
        let response = route_request("GET /unknown HTTP/1.1", &build_state(true));
        assert!(response.starts_with("HTTP/1.1 404 Not Found"));
        assert!(response.contains("\"code\":\"not_found\""));
    }

    #[test]
    fn unsupported_method_returns_json_error() {
        let response = route_request("DELETE /healthz HTTP/1.1", &build_state(true));
        assert!(response.starts_with("HTTP/1.1 405 Method Not Allowed"));
        assert!(response.contains("\"code\":\"method_not_allowed\""));
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
