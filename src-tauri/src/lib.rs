use std::net::TcpStream;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, RunEvent};

struct BackendProcess(Mutex<Option<Child>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Spawn the orchestrator process
            // Note: In a real production environment this should use Tauri's Sidecar feature
            // Here we spawn the local orchestrator directly assuming the binary is available or we run it via cargo for dev.
            println!("Starting backend orchestrator...");
            let mut cmd = Command::new("cargo");
            cmd.args(["run", "--bin", "recordroute-orchestrator"]);
            // If in production, you would run the sidecar binary directly instead:
            // let sidecar_command = app.shell().sidecar("recordroute-orchestrator").unwrap();
            
            match cmd.spawn() {
                Ok(child) => {
                    app.manage(BackendProcess(Mutex::new(Some(child))));
                    println!("Backend orchestrator started successfully.");
                    
                    // Poll for backend readiness
                    let start = Instant::now();
                    let timeout = Duration::from_secs(30);
                    let mut ready = false;
                    while start.elapsed() < timeout {
                        if let Ok(mut stream) = TcpStream::connect("127.0.0.1:18000") {
                            // Read for a very basic GET /readyz HTTP/1.1
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
                }
                Err(e) => {
                    eprintln!("Failed to start backend orchestrator: {}", e);
                }
            }

            Ok(())
        });

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let RunEvent::Exit = event {
            println!("Tauri app exiting, stopping backend orchestrator...");
            let state = app_handle.state::<BackendProcess>();
            if let Ok(mut lock) = state.0.lock() {
                if let Some(mut child) = lock.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                    println!("Backend orchestrator stopped.");
                }
            }
        }
    });
}
