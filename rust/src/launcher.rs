use crate::app::load_repo_env;
use crate::runtime_root;
use crate::server::SERVER_BIND;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const APP_URL: &str = "http://127.0.0.1:38080/";
const PING_PATH: &str = "/server/ping";
const SERVER_PID_FILE_NAME: &str = "server.pid";
const READY_TIMEOUT: Duration = Duration::from_secs(20);
const READY_POLL_INTERVAL: Duration = Duration::from_millis(250);
const TERMINATION_GRACE_TIMEOUT: Duration = Duration::from_secs(3);
const TERMINATION_POLL_INTERVAL: Duration = Duration::from_millis(100);

pub fn run_launcher() -> Result<(), String> {
    let runtime_root = runtime_root::resolve_runtime_root()?;
    load_repo_env(&runtime_root)?;

    let logs_dir = ensure_logs_dir(&runtime_root)?;
    let pid_file = logs_dir.join(SERVER_PID_FILE_NAME);

    stop_existing_owned_server(&pid_file)?;
    ensure_server_bind_available(SERVER_BIND, &pid_file)?;

    let mut server = spawn_server(&runtime_root, &logs_dir, &pid_file)?;
    if !wait_for_server_ready(READY_TIMEOUT) {
        let _ = shutdown_owned_server(&mut server);
        return Err(format!(
            "failed to start RecordRoute server within {:?}. See log: {}",
            READY_TIMEOUT,
            logs_dir.join("server.log").display()
        ));
    }

    if let Err(error) = open_browser(APP_URL) {
        let _ = shutdown_owned_server(&mut server);
        return Err(error);
    }

    wait_for_termination_signal();
    shutdown_owned_server(&mut server)
}

struct OwnedServer {
    child: Child,
    pid_file: PathBuf,
    stdout_pump: Option<JoinHandle<Result<(), String>>>,
    stderr_pump: Option<JoinHandle<Result<(), String>>>,
}

fn spawn_server(
    runtime_root: &Path,
    logs_dir: &Path,
    pid_file: &Path,
) -> Result<OwnedServer, String> {
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

    let log_path = logs_dir.join("server.log");
    let mut log = open_log_file(&log_path)?;
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

    let shared_log = Arc::new(Mutex::new(log));
    let mut child = Command::new(server_exe)
        .env(runtime_root::RUNTIME_ROOT_ENV_VAR, runtime_root)
        .current_dir(runtime_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to spawn RecordRouteServer: {error}"))?;

    write_pid_file(pid_file, child.id())?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture RecordRouteServer stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture RecordRouteServer stderr".to_string())?;

    Ok(OwnedServer {
        child,
        pid_file: pid_file.to_path_buf(),
        stdout_pump: Some(spawn_log_pump(
            stdout,
            Arc::clone(&shared_log),
            StreamTarget::Stdout,
        )),
        stderr_pump: Some(spawn_log_pump(stderr, shared_log, StreamTarget::Stderr)),
    })
}

fn ensure_logs_dir(runtime_root: &Path) -> Result<PathBuf, String> {
    let logs_dir = runtime_root.join("logs");
    fs::create_dir_all(&logs_dir).map_err(|error| {
        format!(
            "failed to create logs directory {}: {error}",
            logs_dir.display()
        )
    })?;
    Ok(logs_dir)
}

fn stop_existing_owned_server(pid_file: &Path) -> Result<(), String> {
    let owned_pid = match read_pid_file(pid_file) {
        Ok(pid) => pid,
        Err(error) => {
            remove_pid_file_if_exists(pid_file)?;
            eprintln!("{error}");
            None
        }
    };

    let Some(pid) = owned_pid else {
        return Ok(());
    };

    if !process_is_running(pid)? {
        remove_pid_file_if_exists(pid_file)?;
        return Ok(());
    }

    terminate_process(pid)?;
    if !wait_for_process_exit(pid, TERMINATION_GRACE_TIMEOUT)? {
        force_kill_process(pid)?;
        if !wait_for_process_exit(pid, TERMINATION_GRACE_TIMEOUT)? {
            return Err(format!(
                "failed to terminate owned RecordRouteServer process {pid}"
            ));
        }
    }

    remove_pid_file_if_exists(pid_file)?;
    Ok(())
}

fn ensure_server_bind_available(bind: &str, pid_file: &Path) -> Result<(), String> {
    if wait_for_port_available(bind, TERMINATION_GRACE_TIMEOUT) {
        return Ok(());
    }

    Err(format!(
        "{bind} is already in use by a server not owned by this launcher. \
Stop the existing process manually and rerun. If this is an older detached RecordRouteServer, \
remove {pid_file} only after confirming the old process is stopped.",
        pid_file = pid_file.display()
    ))
}

fn open_log_file(log_path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|error| format!("failed to open server log {}: {error}", log_path.display()))
}

enum StreamTarget {
    Stdout,
    Stderr,
}

fn spawn_log_pump<R>(
    mut reader: R,
    log: Arc<Mutex<File>>,
    target: StreamTarget,
) -> JoinHandle<Result<(), String>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = reader
                .read(&mut buffer)
                .map_err(|error| format!("failed to read server output: {error}"))?;
            if bytes_read == 0 {
                return Ok(());
            }

            {
                let mut log = log.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                log.write_all(&buffer[..bytes_read])
                    .map_err(|error| format!("failed to write server log: {error}"))?;
                log.flush()
                    .map_err(|error| format!("failed to flush server log: {error}"))?;
            }

            write_terminal_output(&buffer[..bytes_read], &target)?;
        }
    })
}

fn write_terminal_output(bytes: &[u8], target: &StreamTarget) -> Result<(), String> {
    match target {
        StreamTarget::Stdout => {
            let mut stdout = io::stdout().lock();
            match stdout.write_all(bytes).and_then(|_| stdout.flush()) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
                Err(error) => Err(format!("failed to write launcher stdout: {error}")),
            }
        }
        StreamTarget::Stderr => {
            let mut stderr = io::stderr().lock();
            match stderr.write_all(bytes).and_then(|_| stderr.flush()) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
                Err(error) => Err(format!("failed to write launcher stderr: {error}")),
            }
        }
    }
}

fn shutdown_owned_server(server: &mut OwnedServer) -> Result<(), String> {
    let terminate_result = terminate_child(&mut server.child);
    let pid_cleanup_result = remove_pid_file_if_exists(&server.pid_file);
    let stdout_result = join_output_pump(server.stdout_pump.take(), "stdout");
    let stderr_result = join_output_pump(server.stderr_pump.take(), "stderr");

    terminate_result?;
    pid_cleanup_result?;
    stdout_result?;
    stderr_result?;
    Ok(())
}

fn join_output_pump(
    handle: Option<JoinHandle<Result<(), String>>>,
    label: &str,
) -> Result<(), String> {
    match handle {
        Some(handle) => match handle.join() {
            Ok(result) => result,
            Err(_) => Err(format!("launcher {label} log pump panicked")),
        },
        None => Ok(()),
    }
}

fn write_pid_file(pid_file: &Path, pid: u32) -> Result<(), String> {
    fs::write(pid_file, format!("{pid}\n"))
        .map_err(|error| format!("failed to write pid file {}: {error}", pid_file.display()))
}

fn read_pid_file(pid_file: &Path) -> Result<Option<u32>, String> {
    if !pid_file.is_file() {
        return Ok(None);
    }

    let raw = fs::read_to_string(pid_file)
        .map_err(|error| format!("failed to read pid file {}: {error}", pid_file.display()))?;
    let pid = raw.trim();
    if pid.is_empty() {
        return Ok(None);
    }

    pid.parse::<u32>().map(Some).map_err(|_| {
        format!(
            "invalid pid file {}: expected numeric pid, found {:?}",
            pid_file.display(),
            pid
        )
    })
}

fn remove_pid_file_if_exists(pid_file: &Path) -> Result<(), String> {
    match fs::remove_file(pid_file) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove pid file {}: {error}",
            pid_file.display()
        )),
    }
}

fn port_is_available(bind: &str) -> bool {
    TcpListener::bind(bind).is_ok()
}

fn wait_for_port_available(bind: &str, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if port_is_available(bind) {
            return true;
        }
        thread::sleep(TERMINATION_POLL_INTERVAL);
    }
    port_is_available(bind)
}

fn process_is_running(pid: u32) -> Result<bool, String> {
    #[cfg(unix)]
    {
        let status = Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| format!("failed to execute kill -0: {error}"))?;
        Ok(status.success())
    }

    #[cfg(windows)]
    {
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
            .map_err(|error| format!("failed to execute tasklist: {error}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains(&format!("\"{pid}\"")))
    }
}

fn terminate_process(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        let status = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
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
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T"])
            .status()
            .map_err(|error| format!("failed to execute taskkill: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("taskkill exited with status {status}"))
        }
    }
}

fn force_kill_process(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        let status = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| format!("failed to execute kill -KILL: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("kill -KILL exited with status {status}"))
        }
    }

    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()
            .map_err(|error| format!("failed to execute taskkill /F: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("taskkill /F exited with status {status}"))
        }
    }
}

fn wait_for_process_exit(pid: u32, timeout: Duration) -> Result<bool, String> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if !process_is_running(pid)? {
            return Ok(true);
        }
        thread::sleep(TERMINATION_POLL_INTERVAL);
    }
    Ok(!process_is_running(pid)?)
}

fn wait_for_server_ready(timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if send_ping_request().unwrap_or(false) {
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
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async {
            let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
                .expect("signal");
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("signal");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = sighup.recv() => {}
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

    let graceful_signal_sent = terminate_process(child.id()).is_ok();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use uuid::Uuid;

    #[test]
    fn pid_file_round_trip_and_cleanup() {
        let temp_dir = std::env::temp_dir().join(format!("launcher-pid-{}", Uuid::now_v7()));
        fs::create_dir_all(&temp_dir).expect("temp dir");
        let pid_file = temp_dir.join("server.pid");

        write_pid_file(&pid_file, 4242).expect("write pid");
        assert_eq!(read_pid_file(&pid_file).expect("read pid"), Some(4242));

        remove_pid_file_if_exists(&pid_file).expect("remove pid");
        assert_eq!(read_pid_file(&pid_file).expect("read missing pid"), None);
    }

    #[test]
    fn read_pid_file_reports_invalid_contents() {
        let temp_dir =
            std::env::temp_dir().join(format!("launcher-pid-invalid-{}", Uuid::now_v7()));
        fs::create_dir_all(&temp_dir).expect("temp dir");
        let pid_file = temp_dir.join("server.pid");
        fs::write(&pid_file, "not-a-pid\n").expect("write invalid pid");

        let error = read_pid_file(&pid_file).expect_err("invalid pid");
        assert!(error.contains("invalid pid file"));
    }

    #[test]
    fn port_is_available_detects_bound_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("local addr");
        let bind = addr.to_string();

        assert!(!port_is_available(&bind));
        drop(listener);
        assert!(wait_for_port_available(&bind, Duration::from_secs(1)));
    }

    #[test]
    fn stop_existing_owned_server_removes_stale_pid_file() {
        let temp_dir = std::env::temp_dir().join(format!("launcher-stale-pid-{}", Uuid::now_v7()));
        fs::create_dir_all(&temp_dir).expect("temp dir");
        let pid_file = temp_dir.join("server.pid");
        write_pid_file(&pid_file, u32::MAX).expect("write stale pid");

        stop_existing_owned_server(&pid_file).expect("remove stale pid");
        assert!(!pid_file.exists());
    }

    #[test]
    fn ensure_server_bind_available_rejects_unknown_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let bind = listener.local_addr().expect("local addr").to_string();
        let pid_file =
            std::env::temp_dir().join(format!("launcher-unknown-listener-{}.pid", Uuid::now_v7()));

        let error = ensure_server_bind_available(&bind, &pid_file).expect_err("listener conflict");
        assert!(error.contains("not owned by this launcher"));
    }

    #[test]
    fn terminate_child_stops_spawned_process() {
        let mut child = spawn_sleep_process();
        terminate_child(&mut child).expect("terminate child");
        assert!(child.try_wait().expect("child status").is_some());
    }

    fn spawn_sleep_process() -> Child {
        #[cfg(unix)]
        {
            Command::new("sleep")
                .arg("30")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn sleep")
        }

        #[cfg(windows)]
        {
            Command::new("powershell")
                .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn sleep")
        }
    }
}
