use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    process::Stdio,
    time::Duration,
};

use tokio::{
    process::{Child, Command},
    sync::watch,
    task::JoinHandle,
};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};

#[derive(Clone, Debug)]
pub struct EngineProcessSpec {
    pub name: &'static str,
    pub port: u16,
    pub command: String,
    pub args: Vec<String>,
    pub health_path: &'static str,
}

#[derive(Debug)]
pub struct EngineManager {
    shutdown_tx: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

#[derive(Clone, Debug)]
pub struct EngineManagerConfig {
    pub startup_timeout: Duration,
    pub readiness_poll_interval: Duration,
    pub shutdown_grace: Duration,
    pub restart_backoff_base: Duration,
    pub restart_backoff_max: Duration,
}

impl EngineManager {
    pub fn spawn(
        specs: Vec<EngineProcessSpec>,
        config: EngineManagerConfig,
        degraded: Arc<AtomicBool>,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let tasks = specs
            .into_iter()
            .map(|spec| {
                let mut rx = shutdown_rx.clone();
                let cfg = config.clone();
                let deg = degraded.clone();
                tokio::spawn(async move {
                    supervise_engine(spec, cfg, &mut rx, deg).await;
                })
            })
            .collect();

        Self { shutdown_tx, tasks }
    }

    pub async fn shutdown(mut self) {
        let _ = self.shutdown_tx.send(true);
        for task in self.tasks.drain(..) {
            if let Err(error) = task.await {
                tracing::warn!(error = %error, "engine supervisor join failed");
            }
        }
    }
}

async fn supervise_engine(
    spec: EngineProcessSpec,
    config: EngineManagerConfig,
    shutdown_rx: &mut watch::Receiver<bool>,
    degraded: Arc<AtomicBool>,
) {
    let mut restart_attempt = 0usize;

    loop {
        if *shutdown_rx.borrow() {
            tracing::info!(engine = spec.name, "engine supervisor stopped before spawn");
            return;
        }

        match spawn_engine(&spec) {
            Ok(mut child) => {
                tracing::info!(
                    engine = spec.name,
                    port = spec.port,
                    "engine process spawned"
                );

                match wait_for_readiness(&spec, &config, shutdown_rx).await {
                    Ok(true) => {
                        degraded.store(false, Ordering::Relaxed);
                        tracing::info!(engine = spec.name, "engine readiness passed");
                        restart_attempt = 0;
                    }
                    Ok(false) => {
                        degraded.store(true, Ordering::Relaxed);
                        tracing::info!(
                            engine = spec.name,
                            "engine shutdown requested during startup"
                        );
                        let _ = graceful_shutdown(&spec, &mut child, config.shutdown_grace).await;
                        return;
                    }
                    Err(error) => {
                        degraded.store(true, Ordering::Relaxed);
                        tracing::warn!(engine = spec.name, error = %error, "engine readiness failed");
                        let _ = graceful_shutdown(&spec, &mut child, config.shutdown_grace).await;
                        backoff_sleep(&spec, &config, restart_attempt, shutdown_rx).await;
                        restart_attempt = restart_attempt.saturating_add(1);
                        continue;
                    }
                }

                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        tracing::info!(engine = spec.name, "engine shutdown signal received");
                        let _ = graceful_shutdown(&spec, &mut child, config.shutdown_grace).await;
                        return;
                    }
                    status = child.wait() => {
                        degraded.store(true, Ordering::Relaxed);
                        match status {
                            Ok(exit) => tracing::warn!(engine = spec.name, exit = %exit, "engine exited unexpectedly"),
                            Err(error) => tracing::warn!(engine = spec.name, error = %error, "failed waiting engine process"),
                        }
                        backoff_sleep(&spec, &config, restart_attempt, shutdown_rx).await;
                        restart_attempt = restart_attempt.saturating_add(1);
                    }
                }
            }
            Err(error) => {
                degraded.store(true, Ordering::Relaxed);
                tracing::error!(engine = spec.name, error = %error, "engine spawn failed");
                backoff_sleep(&spec, &config, restart_attempt, shutdown_rx).await;
                restart_attempt = restart_attempt.saturating_add(1);
            }
        }
    }
}

fn spawn_engine(spec: &EngineProcessSpec) -> Result<Child, std::io::Error> {
    let mut cmd = Command::new(&spec.command);
    cmd.args(&spec.args)
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(spec.port.to_string())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    tracing::info!(engine = spec.name, command = %spec.command, args = ?spec.args, "spawning engine command");
    cmd.spawn()
}

async fn wait_for_readiness(
    spec: &EngineProcessSpec,
    config: &EngineManagerConfig,
    shutdown_rx: &mut watch::Receiver<bool>,
) -> Result<bool, std::io::Error> {
    let timeout = tokio::time::sleep(config.startup_timeout);
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => return Ok(false),
            _ = &mut timeout => return Ok(false),
            _ = tokio::time::sleep(config.readiness_poll_interval) => {
                if readiness_probe(spec.port, spec.health_path, config.readiness_poll_interval).await? {
                    return Ok(true);
                }
            }
        }
    }
}

async fn readiness_probe(
    port: u16,
    health_path: &str,
    timeout: Duration,
) -> Result<bool, std::io::Error> {
    let health_path = health_path.to_string();
    tokio::task::spawn_blocking(move || {
        let addr = ("127.0.0.1", port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::AddrNotAvailable, "no address")
            })?;

        let mut stream = TcpStream::connect_timeout(&addr, timeout)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            health_path, port
        );
        stream.write_all(request.as_bytes())?;
        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        Ok(response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200"))
    })
    .await
    .map_err(|error| std::io::Error::other(format!("probe join error: {error}")))?
}

async fn graceful_shutdown(
    spec: &EngineProcessSpec,
    child: &mut Child,
    grace: Duration,
) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        if let Some(pid) = child.id() {
            let _ = Command::new("kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .status()
                .await;
            tracing::info!(engine = spec.name, pid, "sent SIGTERM to engine process");
        }
    }

    match tokio::time::timeout(grace, child.wait()).await {
        Ok(wait_result) => {
            let status = wait_result?;
            tracing::info!(engine = spec.name, exit = %status, "engine exited gracefully");
            Ok(())
        }
        Err(_) => {
            tracing::warn!(
                engine = spec.name,
                "engine graceful shutdown timed out; forcing kill"
            );
            child.start_kill()?;
            let _ = child.wait().await;
            Ok(())
        }
    }
}

async fn backoff_sleep(
    spec: &EngineProcessSpec,
    config: &EngineManagerConfig,
    attempt: usize,
    shutdown_rx: &mut watch::Receiver<bool>,
) {
    let exponent = attempt.min(6) as u32;
    let mut delay = config
        .restart_backoff_base
        .mul_f32((2u32.saturating_pow(exponent)) as f32);
    if delay > config.restart_backoff_max {
        delay = config.restart_backoff_max;
    }
    tracing::info!(
        engine = spec.name,
        backoff_ms = delay.as_millis() as u64,
        "waiting before engine restart"
    );

    tokio::select! {
        _ = shutdown_rx.changed() => {}
        _ = tokio::time::sleep(delay) => {}
    }
}
