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
}

#[derive(Clone, Debug, Serialize)]
struct Job {
    job_id: String,
    status: &'static str,
}

impl AppConfig {
    fn from_env() -> Self {
        let host = env::var("RECORDROUTE_API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let api_port = env::var("RECORDROUTE_API_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(18_000);

        Self { host, api_port }
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

    fn create_job(&self) -> Job {
        let job_number = self.next_id.fetch_add(1, Ordering::Relaxed);
        let job_id = format!("job-{job_number:010}");
        let job = Job {
            job_id: job_id.clone(),
            status: "queued",
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
    let path = parts.next().unwrap_or_default();

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
            let job = state.jobs.create_job();
            json_response(
                202,
                &JobCreatedResponse {
                    job_id: &job.job_id,
                },
            )
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
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_state(ready: bool) -> AppState {
        AppState {
            ready,
            jobs: Arc::new(JobStore::new()),
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
