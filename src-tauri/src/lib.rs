use std::process::{Command, Child};
use std::sync::Mutex;
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
