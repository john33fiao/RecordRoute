use std::{
    env, fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
};

fn main() {
    let host = env::var("RECORDROUTE_SWAGGER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("RECORDROUTE_SWAGGER_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(14_000);
    let root = env::var("RECORDROUTE_SWAGGER_ROOT").unwrap_or_else(|_| "docs/swagger".to_string());
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .expect("valid swagger bind address");

    let listener = TcpListener::bind(addr).expect("bind swagger server");
    println!("swagger server listening on {addr}, root={root}");

    for conn in listener.incoming() {
        match conn {
            Ok(mut socket) => {
                if let Err(err) = handle(&mut socket, &root) {
                    eprintln!("swagger request error: {err}");
                }
            }
            Err(err) => eprintln!("incoming error: {err}"),
        }
    }
}

fn handle(
    socket: &mut TcpStream,
    root: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut buf = [0u8; 4096];
    let read = socket.read(&mut buf)?;
    if read == 0 {
        return Ok(());
    }
    let req = String::from_utf8_lossy(&buf[..read]);
    let line = req.lines().next().unwrap_or_default();
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or("/");

    if method != "GET" {
        write_response(socket, 405, "text/plain", b"method not allowed")?;
        return Ok(());
    }

    let mut rel = if path == "/" {
        "index.html".to_string()
    } else {
        path.trim_start_matches('/').to_string()
    };
    if rel.contains("..") {
        rel = "index.html".to_string();
    }
    let fs_path = PathBuf::from(root).join(rel);
    match fs::read(&fs_path) {
        Ok(bytes) => {
            let mime = if fs_path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                "application/yaml"
            } else if fs_path.extension().and_then(|e| e.to_str()) == Some("html") {
                "text/html; charset=utf-8"
            } else {
                "application/octet-stream"
            };
            write_response(socket, 200, mime, &bytes)?;
        }
        Err(_) => write_response(socket, 404, "text/plain", b"not found")?,
    }
    Ok(())
}

fn write_response(
    socket: &mut TcpStream,
    status: u16,
    ctype: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Unknown",
    };
    let head = format!("HTTP/1.1 {status} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    socket.write_all(head.as_bytes())?;
    socket.write_all(body)?;
    socket.flush()
}
