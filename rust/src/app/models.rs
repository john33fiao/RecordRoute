use super::{
    MODEL_PREPARATION_HEARTBEAT_INTERVAL, MODEL_PREPARATION_STALE_THRESHOLD_SECS,
    MODEL_PREPARATION_WAIT_POLL_INTERVAL, ModelPrepareDisposition, ModelPrepareSubmission,
    ModelStatusEntry, ModelStatusSnapshot, now_rfc3339,
};
use crate::index::{IndexStore, ModelKind, ModelPreparationRecord, ModelPreparationStatus};
use crate::llama::Toolchain as LlamaToolchain;
use crate::whisper::Toolchain as WhisperToolchain;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModelInspection {
    ready: bool,
}

pub fn prepare_llama_model_with_repo_root(repo_root: &Path) -> Result<(), String> {
    ensure_model_prepared(repo_root, ModelKind::Llama).map(|_| ())
}

pub fn submit_model_preparation(
    repo_root: &Path,
    model: ModelKind,
) -> Result<ModelPrepareSubmission, String> {
    let index_store = IndexStore::new(repo_root);
    let inspection = inspect_model_preparation(repo_root, model)?;
    if inspection.ready {
        let finished_at = now_rfc3339()?;
        let preparation = index_store.update_model_preparation(model, |record| {
            record.mark_completed(finished_at.clone());
        })?;
        return Ok(ModelPrepareSubmission {
            model,
            disposition: ModelPrepareDisposition::AlreadyReady,
            preparation,
        });
    }

    let started_at = now_rfc3339()?;
    let mut deduplicated = false;
    let preparation = index_store.update_model_preparation(model, |record| {
        if record.status == ModelPreparationStatus::Running && !is_model_preparation_stale(record) {
            deduplicated = true;
            return;
        }
        record.mark_running(started_at.clone());
    })?;

    Ok(ModelPrepareSubmission {
        model,
        disposition: if deduplicated {
            ModelPrepareDisposition::Deduplicated
        } else {
            ModelPrepareDisposition::Submitted
        },
        preparation,
    })
}

pub fn execute_model_preparation(
    repo_root: &Path,
    model: ModelKind,
) -> Result<ModelPreparationRecord, String> {
    let index_store = IndexStore::new(repo_root);
    let heartbeat_at = now_rfc3339()?;
    index_store.update_model_preparation(model, |record| {
        if record.status == ModelPreparationStatus::Running {
            record.touch(heartbeat_at.clone());
        } else {
            record.mark_running(heartbeat_at.clone());
        }
    })?;

    let repo_root_for_heartbeat = repo_root.to_path_buf();
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let heartbeat_thread =
        thread::spawn(move || {
            loop {
                match stop_rx.recv_timeout(MODEL_PREPARATION_HEARTBEAT_INTERVAL) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if let Ok(heartbeat) = now_rfc3339() {
                            let _ = IndexStore::new(&repo_root_for_heartbeat)
                                .update_model_preparation(model, |record| {
                                    if record.status == ModelPreparationStatus::Running {
                                        record.touch(heartbeat.clone());
                                    }
                                });
                        }
                    }
                }
            }
        });

    let result = ensure_model_with_toolchain(repo_root, model);
    drop(stop_tx);
    let _ = heartbeat_thread.join();

    match result {
        Ok(()) => {
            let finished_at = now_rfc3339()?;
            index_store.update_model_preparation(model, |record| {
                record.mark_completed(finished_at.clone());
            })
        }
        Err(error) => {
            let finished_at = now_rfc3339()?;
            index_store.update_model_preparation(model, |record| {
                record.mark_failed(finished_at.clone(), error.clone());
            })?;
            Err(error)
        }
    }
}

pub fn wait_for_model_preparation(
    repo_root: &Path,
    model: ModelKind,
) -> Result<ModelPreparationRecord, String> {
    loop {
        let snapshot = collect_model_status_snapshot(repo_root)?;
        let entry = snapshot.entry(model).clone();
        if entry.ready {
            return Ok(entry.preparation);
        }

        match entry.preparation.status {
            ModelPreparationStatus::Running => {
                if is_model_preparation_stale(&entry.preparation) {
                    let submission = submit_model_preparation(repo_root, model)?;
                    if submission.already_ready() {
                        return Ok(submission.preparation);
                    }
                    if submission.should_execute() {
                        return execute_model_preparation(repo_root, model);
                    }
                }
                thread::sleep(MODEL_PREPARATION_WAIT_POLL_INTERVAL);
            }
            ModelPreparationStatus::Failed => {
                return Err(entry.error.unwrap_or_else(|| {
                    entry
                        .preparation
                        .last_error
                        .unwrap_or_else(|| format!("{} model preparation failed", model.as_str()))
                }));
            }
            ModelPreparationStatus::Idle => {
                return Err(format!(
                    "{} model preparation is not running",
                    model.as_str()
                ));
            }
            ModelPreparationStatus::Completed => {
                return Err(entry.error.unwrap_or_else(|| {
                    format!("{} model is not ready after preparation", model.as_str())
                }));
            }
        }
    }
}

pub fn ensure_model_prepared(
    repo_root: &Path,
    model: ModelKind,
) -> Result<ModelPreparationRecord, String> {
    let submission = submit_model_preparation(repo_root, model)?;
    if submission.already_ready() {
        return Ok(submission.preparation);
    }
    if submission.should_execute() {
        return execute_model_preparation(repo_root, model);
    }
    wait_for_model_preparation(repo_root, model)
}

pub fn collect_model_status_snapshot(repo_root: &Path) -> Result<ModelStatusSnapshot, String> {
    let preparations = IndexStore::new(repo_root).model_preparations()?;
    Ok(ModelStatusSnapshot {
        whisper: collect_model_status_entry(repo_root, ModelKind::Whisper, preparations.whisper),
        llama: collect_model_status_entry(repo_root, ModelKind::Llama, preparations.llama),
    })
}

fn collect_model_status_entry(
    repo_root: &Path,
    model: ModelKind,
    preparation: ModelPreparationRecord,
) -> ModelStatusEntry {
    match model {
        ModelKind::Whisper => match WhisperToolchain::discover(repo_root) {
            Ok(toolchain) => {
                let ready = toolchain.is_model_ready();
                let error = if ready {
                    None
                } else {
                    toolchain
                        .can_prepare_model()
                        .err()
                        .or_else(|| failed_model_preparation_error(&preparation))
                };
                ModelStatusEntry {
                    model,
                    available: true,
                    ready,
                    error,
                    embedding_available: true,
                    embedding_ready: ready,
                    embedding_error: None,
                    preparation,
                }
            }
            Err(error) => ModelStatusEntry {
                model,
                available: false,
                ready: false,
                error: Some(error),
                embedding_available: false,
                embedding_ready: false,
                embedding_error: None,
                preparation,
            },
        },
        ModelKind::Llama => match LlamaToolchain::discover(repo_root) {
            Ok(toolchain) => {
                let ready = toolchain.is_model_ready();
                let error = if ready {
                    None
                } else {
                    toolchain
                        .can_prepare_model()
                        .err()
                        .or_else(|| failed_model_preparation_error(&preparation))
                };
                ModelStatusEntry {
                    model,
                    available: true,
                    ready,
                    error,
                    embedding_available: toolchain.llama_embedding_path.is_file(),
                    embedding_ready: ready,
                    embedding_error: None,
                    preparation,
                }
            }
            Err(error) => ModelStatusEntry {
                model,
                available: false,
                ready: false,
                error: Some(error),
                embedding_available: false,
                embedding_ready: false,
                embedding_error: None,
                preparation,
            },
        },
    }
}

fn failed_model_preparation_error(preparation: &ModelPreparationRecord) -> Option<String> {
    if preparation.status == ModelPreparationStatus::Failed {
        preparation.last_error.clone()
    } else {
        None
    }
}

fn inspect_model_preparation(
    repo_root: &Path,
    model: ModelKind,
) -> Result<ModelInspection, String> {
    match model {
        ModelKind::Whisper => {
            let toolchain = WhisperToolchain::discover(repo_root)?;
            if toolchain.is_model_ready() {
                Ok(ModelInspection { ready: true })
            } else {
                toolchain.can_prepare_model()?;
                Ok(ModelInspection { ready: false })
            }
        }
        ModelKind::Llama => {
            let toolchain = LlamaToolchain::discover(repo_root)?;
            if toolchain.is_model_ready() {
                Ok(ModelInspection { ready: true })
            } else {
                toolchain.can_prepare_model()?;
                Ok(ModelInspection { ready: false })
            }
        }
    }
}

fn ensure_model_with_toolchain(repo_root: &Path, model: ModelKind) -> Result<(), String> {
    match model {
        ModelKind::Whisper => WhisperToolchain::discover(repo_root)?.ensure_model(),
        ModelKind::Llama => LlamaToolchain::discover(repo_root)?.ensure_model(),
    }
}

fn is_model_preparation_stale(record: &ModelPreparationRecord) -> bool {
    if record.status != ModelPreparationStatus::Running {
        return false;
    }

    let Some(heartbeat_at) = record
        .heartbeat_at
        .as_deref()
        .or(record.started_at.as_deref())
    else {
        return true;
    };

    let Ok(parsed_heartbeat_at) = OffsetDateTime::parse(heartbeat_at, &Rfc3339) else {
        return true;
    };

    OffsetDateTime::now_utc() - parsed_heartbeat_at
        >= time::Duration::seconds(MODEL_PREPARATION_STALE_THRESHOLD_SECS)
}
