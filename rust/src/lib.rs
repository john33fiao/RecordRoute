mod app;
mod error;
mod ffmpeg;
mod index;
mod launcher;
mod llama;
mod runtime_root;
mod server;
mod tool_runtime;
mod whisper;

pub use app::main_cli;

pub fn main_server() -> Result<(), String> {
    let runtime_root = runtime_root::resolve_runtime_root()?;
    app::load_repo_env(&runtime_root)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create tokio runtime: {error}"))?;
    runtime.block_on(server::serve_with_runtime_root(runtime_root))
}

pub fn main_launcher() -> Result<(), String> {
    launcher::run_launcher()
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::{Mutex, OnceLock};

    pub(crate) fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }
}
