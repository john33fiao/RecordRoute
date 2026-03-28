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
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || run_dispatcher(repo_root, rx));
        let dispatcher = Self { tx };
        dispatcher.wake();
        dispatcher
    }

    pub(crate) fn wake(&self) {
        let _ = self.tx.send(());
    }
}

fn run_dispatcher(repo_root: PathBuf, rx: mpsc::Receiver<()>) {
    if let Err(error) = app::recover_interrupted_active_entry(&repo_root) {
        eprintln!("{error}");
    }

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
