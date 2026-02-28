use std::{
    env,
    fs::{self, File},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

#[derive(Serialize)]
struct ProbeSummary {
    os: String,
    api_port: u16,
    swagger_port: u16,
    port_conflict_check: &'static str,
    startup_check: &'static str,
    runtime_contract_check: &'static str,
    shutdown_check: &'static str,
    artifacts_dir: String,
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let os = env::consts::OS.to_string();
    let api_port = read_port_env("RECORDROUTE_TAURI_PROBE_API_PORT", 18_010)?;
    let swagger_port = read_port_env("RECORDROUTE_TAURI_PROBE_SWAGGER_PORT", 14_010)?;

    let artifacts_dir = prepare_artifacts_dir(&os)?;
    println!(
        "[tauri_lifecycle_probe] artifacts: {}",
        artifacts_dir.display()
    );

    let orchestrator_bin = sibling_bin_path("recordroute-orchestrator")?;
    let swagger_bin = sibling_bin_path("swagger_server")?;

    let conflict_log = artifacts_dir.join("orchestrator-port-conflict.log");
    let conflict_listener = TcpListener::bind(("127.0.0.1", api_port))?;
    let mut conflict_child = spawn_logged(
        &orchestrator_bin,
        &[
            ("RECORDROUTE_API_HOST", "127.0.0.1"),
            ("RECORDROUTE_API_PORT", &api_port.to_string()),
            ("RECORDROUTE_ENGINE_SUPERVISION_ENABLED", "false"),
        ],
        &conflict_log,
    )?;

    let conflict_exited = wait_exit(&mut conflict_child, Duration::from_secs(10))?;
    drop(conflict_listener);
    if !conflict_exited || !log_contains(&conflict_log, "address already in use") {
        return Err("port conflict check failed: expected bind failure log".into());
    }

    let orchestrator_log = artifacts_dir.join("orchestrator.log");
    let swagger_log = artifacts_dir.join("swagger.log");

    let mut orchestrator = spawn_logged(
        &orchestrator_bin,
        &[
            ("RECORDROUTE_API_HOST", "127.0.0.1"),
            ("RECORDROUTE_API_PORT", &api_port.to_string()),
            ("RECORDROUTE_ENGINE_SUPERVISION_ENABLED", "false"),
        ],
        &orchestrator_log,
    )?;

    let mut swagger = spawn_logged(
        &swagger_bin,
        &[
            ("RECORDROUTE_SWAGGER_HOST", "127.0.0.1"),
            ("RECORDROUTE_SWAGGER_PORT", &swagger_port.to_string()),
            ("RECORDROUTE_SWAGGER_ROOT", "docs/swagger"),
        ],
        &swagger_log,
    )?;

    wait_http_ok(api_port, "/healthz", Duration::from_secs(20))?;
    wait_http_ok(api_port, "/readyz", Duration::from_secs(20))?;
    wait_http_ok(api_port, "/metrics", Duration::from_secs(20))?;
    wait_http_ok(swagger_port, "/", Duration::from_secs(20))?;
    wait_http_ok(swagger_port, "/openapi.yaml", Duration::from_secs(20))?;

    let created_job_id = post_job_and_extract_job_id(api_port)?;
    wait_http_ok(
        api_port,
        &format!("/jobs/{created_job_id}"),
        Duration::from_secs(20),
    )?;

    terminate_child(&mut swagger)?;
    terminate_child(&mut orchestrator)?;

    ensure_exited(&mut swagger, "swagger")?;
    ensure_exited(&mut orchestrator, "orchestrator")?;

    let summary = ProbeSummary {
        os,
        api_port,
        swagger_port,
        port_conflict_check: "pass",
        startup_check: "pass",
        runtime_contract_check: "pass",
        shutdown_check: "pass",
        artifacts_dir: artifacts_dir.display().to_string(),
    };

    let summary_path = artifacts_dir.join("summary.json");
    let summary_json = serde_json::to_string_pretty(&summary)?;
    fs::write(&summary_path, summary_json)?;

    println!("[tauri_lifecycle_probe] READY: {}", summary_path.display());
    Ok(())
}

fn read_port_env(
    key: &str,
    default_value: u16,
) -> Result<u16, Box<dyn std::error::Error + Send + Sync>> {
    match env::var(key) {
        Ok(v) => Ok(v.parse::<u16>()?),
        Err(_) => Ok(default_value),
    }
}

fn prepare_artifacts_dir(os: &str) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let pid = std::process::id();
    let dir = PathBuf::from("artifacts")
        .join("tauri-lifecycle")
        .join(format!("{}-{}-{}", ts, os, pid));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn sibling_bin_path(bin_name: &str) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let exe = env::current_exe()?;
    let bin_dir = exe
        .parent()
        .ok_or("current executable parent directory missing")?;

    #[cfg(windows)]
    let file_name = format!("{}.exe", bin_name);
    #[cfg(not(windows))]
    let file_name = bin_name.to_string();

    let target = bin_dir.join(file_name);
    if !target.exists() {
        build_bin(bin_name)?;
    }
    if !target.exists() {
        return Err(format!(
            "required sibling binary not found after build: {}",
            target.display()
        )
        .into());
    }
    Ok(target)
}

fn build_bin(bin_name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let status = Command::new("cargo")
        .arg("build")
        .arg("--bin")
        .arg(bin_name)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("failed to build required binary: {}", bin_name).into())
    }
}

fn spawn_logged(
    cmd: &Path,
    envs: &[(&str, &str)],
    log_path: &Path,
) -> Result<Child, Box<dyn std::error::Error + Send + Sync>> {
    let stdout = File::create(log_path)?;
    let stderr = stdout.try_clone()?;

    let mut command = Command::new(cmd);
    command
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));

    for (k, v) in envs {
        command.env(k, v);
    }

    Ok(command.spawn()?)
}

fn wait_exit(
    child: &mut Child,
    timeout: Duration,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if child.try_wait()?.is_some() {
            return Ok(true);
        }
        thread::sleep(Duration::from_millis(200));
    }
    terminate_child(child)?;
    Ok(false)
}

fn wait_http_ok(
    port: u16,
    path: &str,
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(status) = get_http_status(port, path) {
            if status == 200 {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err(format!(
        "timeout waiting for http 200: {}:{}{}",
        "127.0.0.1", port, path
    )
    .into())
}

fn get_http_status(port: u16, path: &str) -> Result<u16, Box<dyn std::error::Error + Send + Sync>> {
    let (status, _) = send_http_request(port, "GET", path)?;
    Ok(status)
}

fn post_job_and_extract_job_id(
    port: u16,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let (status, body) = send_http_request(port, "POST", "/jobs?engine=stt")?;
    if status != 202 {
        return Err(format!("expected 202 from POST /jobs?engine=stt, got {status}").into());
    }

    let parsed: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("failed to parse POST /jobs response body as json: {e}"))?;

    let job_id = parsed
        .get("job_id")
        .and_then(|v| v.as_str())
        .ok_or("missing string field `job_id` in POST /jobs response")?;

    if job_id.is_empty() {
        return Err("POST /jobs returned empty job_id".into());
    }

    Ok(job_id.to_string())
}

fn send_http_request(
    port: u16,
    method: &str,
    path: &str,
) -> Result<(u16, String), Box<dyn std::error::Error + Send + Sync>> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;

    let req = format!(
        "{} {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
        method, path, port
    );
    stream.write_all(req.as_bytes())?;

    let mut buf = Vec::with_capacity(1024);
    stream.read_to_end(&mut buf)?;
    let raw = String::from_utf8_lossy(&buf);
    let first_line = raw.lines().next().unwrap_or_default();
    let code = first_line
        .split_whitespace()
        .nth(1)
        .ok_or("invalid http status line")?
        .parse::<u16>()?;

    let body = raw
        .split_once("\r\n\r\n")
        .map(|(_, b)| b)
        .unwrap_or_default()
        .to_string();

    Ok((code, body))
}

fn terminate_child(child: &mut Child) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if child.try_wait()?.is_none() {
        child.kill()?;
        let _ = child.wait();
    }
    Ok(())
}

fn ensure_exited(
    child: &mut Child,
    name: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if child.try_wait()?.is_none() {
        return Err(format!("{} process still alive after terminate", name).into());
    }
    Ok(())
}

fn log_contains(path: &Path, needle: &str) -> bool {
    match fs::read_to_string(path) {
        Ok(content) => content.to_lowercase().contains(&needle.to_lowercase()),
        Err(_) => false,
    }
}
