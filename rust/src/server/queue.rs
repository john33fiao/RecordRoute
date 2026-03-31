use crate::app::{self, DispatchState};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

#[derive(Debug, Clone)]
pub(crate) struct QueueDispatcher {
    tx: mpsc::Sender<()>,
}

impl QueueDispatcher {
    pub(crate) fn start(repo_root: PathBuf) -> Self {
        let start_paused = queue_start_paused_from_env();
        if start_paused && let Err(error) = app::set_queue_paused(&repo_root, true) {
            eprintln!("{error}");
        }

        if let Err(error) = app::recover_interrupted_active_entry(&repo_root) {
            eprintln!("{error}");
        }
        app::refresh_audio_cache(&repo_root);

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || run_dispatcher(repo_root, rx));
        let dispatcher = Self { tx };
        if !start_paused {
            dispatcher.wake();
        }
        dispatcher
    }

    pub(crate) fn wake(&self) {
        let _ = self.tx.send(());
    }
}

fn run_dispatcher(repo_root: PathBuf, rx: mpsc::Receiver<()>) {
    let mut state = DispatchState::default();

    loop {
        loop {
            match app::dispatch_one(&repo_root, &mut state) {
                Ok(true) => continue,
                Ok(false) => break,
                Err(error) => {
                    eprintln!("{error}");
                    break;
                }
            }
        }

        if rx.recv().is_err() {
            break;
        }
        while rx.try_recv().is_ok() {}
    }
}

fn queue_start_paused_from_env() -> bool {
    match std::env::var(super::QUEUE_START_PAUSED_ENV_VAR) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}
