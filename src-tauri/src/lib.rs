use std::net::TcpStream;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, RunEvent};
use tauri_plugin_shell::ShellExt;

#[cfg(debug_assertions)]
type ChildProcess = std::process::Child;

#[cfg(not(debug_assertions))]
type ChildProcess = tauri_plugin_shell::process::CommandChild;

struct BackendProcesses {
    orchestrator: Mutex<Option<ChildProcess>>,
    swagger: Mutex<Option<ChildProcess>>,
}

#[cfg(debug_assertions)]
fn spawn_orchestrator(_app: &tauri::App) -> Result<ChildProcess, String> {
    let mut cmd = std::process::Command::new("cargo");
    cmd.args(["run", "--bin", "recordroute-orchestrator"]);
    for (key, value) in std::env::vars() {
        if key.starts_with("RECORDROUTE_") {
            cmd.env(key, value);
        }
    }
    cmd.spawn().map_err(|e| e.to_string())
}

#[cfg(not(debug_assertions))]
fn spawn_orchestrator(app: &tauri::App) -> Result<ChildProcess, String> {
    let mut cmd = app
        .shell()
        .sidecar("recordroute-orchestrator")
        .map_err(|e| e.to_string())?;
    for (key, value) in std::env::vars() {
        if key.starts_with("RECORDROUTE_") {
            cmd = cmd.env(key, value);
        }
    }
    let (_, child) = cmd.spawn().map_err(|e| e.to_string())?;
    Ok(child)
}

#[cfg(debug_assertions)]
fn spawn_swagger(_app: &tauri::App) -> Result<ChildProcess, String> {
    let mut cmd = std::process::Command::new("cargo");
    cmd.args(["run", "--bin", "swagger_server"]);
    cmd.spawn().map_err(|e| e.to_string())
}

#[cfg(not(debug_assertions))]
fn spawn_swagger(app: &tauri::App) -> Result<ChildProcess, String> {
    let cmd = app
        .shell()
        .sidecar("swagger_server")
        .map_err(|e| e.to_string())?;
    let (_, child) = cmd.spawn().map_err(|e| e.to_string())?;
    Ok(child)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            println!("Starting backend orchestrator and swagger...");
            
            let orchestrator = spawn_orchestrator(app).unwrap_or_else(|e| {
                eprintln!("Failed to start orchestrator: {}", e);
                std::process::exit(1);
            });
            
            let swagger = spawn_swagger(app).unwrap_or_else(|e| {
                eprintln!("Failed to start swagger: {}", e);
                std::process::exit(1);
            });

            app.manage(BackendProcesses {
                orchestrator: Mutex::new(Some(orchestrator)),
                swagger: Mutex::new(Some(swagger)),
            });

            println!("Backend services started successfully.");

            // Poll for backend readiness
            let start = Instant::now();
            let timeout = Duration::from_secs(30);
            let mut ready = false;
            while start.elapsed() < timeout {
                if let Ok(mut stream) = TcpStream::connect("127.0.0.1:18000") {
                    use std::io::{Read, Write};
                    let _ = stream.write_all(b"GET /readyz HTTP/1.1\r\nHost: 127.0.0.1:18000\r\nConnection: close\r\n\r\n");
                    let mut buf = vec![0; 128];
                    if let Ok(bytes_read) = stream.read(&mut buf) {
                        let response = String::from_utf8_lossy(&buf[..bytes_read]);
                        if response.contains("200 OK") || response.contains("ready") {
                            ready = true;
                            break;
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(500));
            }

            if ready {
                println!("Backend orchestrator is ready.");
            } else {
                eprintln!("Warning: Backend orchestrator readiness check timed out.");
            }

            Ok(())
        });

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let RunEvent::Exit = event {
            println!("Tauri app exiting, stopping backend services...");
            let state = app_handle.state::<BackendProcesses>();
            
            if let Ok(mut lock) = state.orchestrator.lock() {
                if let Some(mut child) = lock.take() {
                    // Try graceful shutdown via HTTP POST /shutdown
                    if let Ok(mut stream) = TcpStream::connect("127.0.0.1:18000") {
                        use std::io::Write;
                        let request = "POST /shutdown HTTP/1.1\r\nHost: 127.0.0.1:18000\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";
                        let _ = stream.write_all(request.as_bytes());
                    }

                    // For dev mode, we could wait; for CommandChild, wait might not be trivial.
                    // Just sleep briefly and then kill.
                    std::thread::sleep(Duration::from_millis(1500));
                    
                    #[cfg(debug_assertions)]
                    { let _ = child.kill(); let _ = child.wait(); }
                    
                    #[cfg(not(debug_assertions))]
                    { let _ = child.kill(); }
                }
            }
            
            if let Ok(mut lock) = state.swagger.lock() {
                if let Some(mut child) = lock.take() {
                    #[cfg(debug_assertions)]
                    { let _ = child.kill(); let _ = child.wait(); }
                    
                    #[cfg(not(debug_assertions))]
                    { let _ = child.kill(); }
                }
            }
            println!("Backend services stopped.");
        }
    });
}
