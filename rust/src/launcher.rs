use crate::app::load_repo_env;
use crate::runtime_root;
use crate::server::SERVER_BIND;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const APP_URL: &str = "http://127.0.0.1:38080/";
const PING_PATH: &str = "/server/ping";
const READY_TIMEOUT: Duration = Duration::from_secs(20);
const READY_POLL_INTERVAL: Duration = Duration::from_millis(250);
const TERMINATION_GRACE_TIMEOUT: Duration = Duration::from_secs(3);
const TERMINATION_POLL_INTERVAL: Duration = Duration::from_millis(100);

pub fn run_launcher() -> Result<(), String> {
    let runtime_root = runtime_root::resolve_runtime_root()?;
    load_repo_env(&runtime_root)?;

    if check_server_ready() {
        open_browser(APP_URL)?;
        return Ok(());
    }

    let mut child = spawn_server(&runtime_root)?;
    if !wait_for_server_ready(READY_TIMEOUT) {
        let _ = terminate_child(&mut child);
        return Err(format!(
            "failed to start RecordRoute server within {:?}. See log: {}",
            READY_TIMEOUT,
            runtime_root.join("logs/server.log").display()
        ));
    }

    open_browser(APP_URL)?;

    wait_for_termination_signal();
    terminate_child(&mut child)?;
    Ok(())
}

fn spawn_server(runtime_root: &Path) -> Result<Child, String> {
    let exe_name = if cfg!(windows) {
        "RecordRouteServer.exe"
    } else {
        "RecordRouteServer"
    };
    let server_exe = runtime_root.join(exe_name);
    if !server_exe.is_file() {
        return Err(format!(
            "server executable not found: {}",
            server_exe.display()
        ));
    }

    let logs_dir = runtime_root.join("logs");
    fs::create_dir_all(&logs_dir).map_err(|error| {
        format!(
            "failed to create logs directory {}: {error}",
            logs_dir.display()
        )
    })?;
    let log_path = logs_dir.join("server.log");

    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|error| format!("failed to open server log {}: {error}", log_path.display()))?;
    writeln!(
        log,
        "\n===== launcher session started at {} =====",
        crate::app::now_rfc3339()?
    )
    .map_err(|error| {
        format!(
            "failed to write server log header {}: {error}",
            log_path.display()
        )
    })?;

    let stdout = log.try_clone().map_err(|error| {
        format!(
            "failed to clone server log handle {}: {error}",
            log_path.display()
        )
    })?;

    Command::new(server_exe)
        .env(runtime_root::RUNTIME_ROOT_ENV_VAR, runtime_root)
        .current_dir(runtime_root)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(log))
        .spawn()
        .map_err(|error| format!("failed to spawn RecordRouteServer: {error}"))
}

fn check_server_ready() -> bool {
    send_ping_request().unwrap_or(false)
}

fn wait_for_server_ready(timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if check_server_ready() {
            return true;
        }
        thread::sleep(READY_POLL_INTERVAL);
    }
    false
}

fn send_ping_request() -> Result<bool, String> {
    let mut stream = std::net::TcpStream::connect(SERVER_BIND)
        .map_err(|error| format!("connect failed: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| format!("set read timeout failed: {error}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| format!("set write timeout failed: {error}"))?;

    let body = r#"{"code":"200","message":"launcher"}"#;
    let request = format!(
        "POST {PING_PATH} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );

    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("write failed: {error}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("read failed: {error}"))?;

    Ok(response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200"))
}

fn open_browser(url: &str) -> Result<(), String> {
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", "start", "", url]).status()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).status()
    } else {
        Command::new("xdg-open").arg(url).status()
    }
    .map_err(|error| format!("failed to launch browser: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("failed to launch browser for {url}"))
    }
}

fn wait_for_termination_signal() {
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("signal");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
            }
        });
    }

    #[cfg(not(unix))]
    {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async {
            let _ = tokio::signal::ctrl_c().await;
        });
    }
}

fn terminate_child(child: &mut Child) -> Result<(), String> {
    if child
        .try_wait()
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Ok(());
    }

    let graceful_signal_sent = send_termination_signal(child).is_ok();
    if graceful_signal_sent && wait_for_child_exit(child, TERMINATION_GRACE_TIMEOUT)? {
        return Ok(());
    }

    child
        .kill()
        .map_err(|error| format!("failed to terminate child process: {error}"))?;
    if wait_for_child_exit(child, TERMINATION_GRACE_TIMEOUT)? {
        return Ok(());
    }

    Err("child process did not terminate after forced kill".to_string())
}

fn wait_for_child_exit(child: &mut Child, timeout: Duration) -> Result<bool, String> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            return Ok(true);
        }
        thread::sleep(TERMINATION_POLL_INTERVAL);
    }

    Ok(child
        .try_wait()
        .map_err(|error| error.to_string())?
        .is_some())
}

fn send_termination_signal(child: &Child) -> Result<(), String> {
    #[cfg(unix)]
    {
        let pid = child.id().to_string();
        let status = Command::new("kill")
            .args(["-TERM", pid.as_str()])
            .status()
            .map_err(|error| format!("failed to execute kill -TERM: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("kill -TERM exited with status {status}"))
        }
    }

    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let status = Command::new("taskkill")
            .args(["/PID", pid.as_str(), "/T"])
            .status()
            .map_err(|error| format!("failed to execute taskkill: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("taskkill exited with status {status}"))
        }
    }
}
