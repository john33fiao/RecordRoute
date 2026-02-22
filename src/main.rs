use std::{
    env,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
};

use tracing_subscriber::{fmt, EnvFilter};

#[derive(Clone, Debug)]
struct AppState {
    ready: bool,
}

#[derive(Debug, Clone)]
struct AppConfig {
    host: String,
    api_port: u16,
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
    let state = AppState { ready: true };

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
    let mut buffer = [0; 2048];
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

    if method != "GET" {
        return plain_text_response(405, "method not allowed");
    }

    match path {
        "/healthz" => json_response(200, "ok"),
        "/readyz" => {
            if state.ready {
                json_response(200, "ready")
            } else {
                json_response(503, "not_ready")
            }
        }
        _ => plain_text_response(404, "not found"),
    }
}

fn json_response(status_code: u16, status: &str) -> String {
    let body = format!("{{\"status\":\"{}\"}}", status);
    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status_code,
        reason_phrase(status_code),
        body.len(),
        body
    )
}

fn plain_text_response(status_code: u16, body: &str) -> String {
    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status_code,
        reason_phrase(status_code),
        body.len(),
        body
    )
}

fn reason_phrase(status_code: u16) -> &'static str {
    match status_code {
        200 => "OK",
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

    #[test]
    fn healthz_route_returns_ok() {
        let response = route_request("GET /healthz HTTP/1.1", &AppState { ready: true });
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("{\"status\":\"ok\"}"));
    }

    #[test]
    fn readyz_route_returns_service_unavailable_when_not_ready() {
        let response = route_request("GET /readyz HTTP/1.1", &AppState { ready: false });
        assert!(response.starts_with("HTTP/1.1 503 Service Unavailable"));
        assert!(response.contains("{\"status\":\"not_ready\"}"));
    }

    #[test]
    fn post_returns_method_not_allowed() {
        let response = route_request("POST /healthz HTTP/1.1", &AppState { ready: true });
        assert!(response.starts_with("HTTP/1.1 405 Method Not Allowed"));
    }
}
