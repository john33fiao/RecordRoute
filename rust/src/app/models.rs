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
    let _ = ensure_model_prepared(repo_root, ModelKind::Llama)?;
    let _ = ensure_llama_embedding_model_prepared(repo_root)?;
    Ok(())
}

pub fn prepare_models_with_repo_root(repo_root: &Path) -> Result<(), String> {
    let _ = ensure_model_prepared(repo_root, ModelKind::Whisper)?;
    prepare_llama_model_with_repo_root(repo_root)?;
    Ok(())
}

pub fn submit_llama_umbrella_preparation(
    repo_root: &Path,
) -> Result<ModelPrepareSubmission, String> {
    inspect_model_preparation(repo_root, ModelKind::Llama)?;
    if let Err(error) = inspect_llama_embedding_preparation(repo_root) {
        let _ = mark_llama_embedding_preparation_failed(repo_root, error.clone());
        return Err(error);
    }

    let summary = submit_model_preparation(repo_root, ModelKind::Llama)?;
    let embedding = submit_llama_embedding_preparation(repo_root)?;
    let embedding_should_execute = embedding.execute_embedding_requested;

    Ok(ModelPrepareSubmission {
        model: ModelKind::Llama,
        disposition: if summary.should_execute() || embedding_should_execute {
            ModelPrepareDisposition::Submitted
        } else if summary.deduplicated() || embedding.deduplicated() {
            ModelPrepareDisposition::Deduplicated
        } else {
            ModelPrepareDisposition::AlreadyReady
        },
        preparation: summary.preparation,
        execute_requested: summary.execute_requested,
        execute_embedding_requested: embedding_should_execute,
    })
}

pub fn execute_llama_umbrella_preparation(
    repo_root: &Path,
    submission: &ModelPrepareSubmission,
) -> Result<ModelPreparationRecord, String> {
    let preparation = if submission.execute_requested {
        execute_model_preparation(repo_root, ModelKind::Llama)?
    } else {
        IndexStore::new(repo_root).model_preparations()?.llama
    };

    if submission.execute_embedding_requested {
        execute_llama_embedding_preparation(repo_root)?;
    }

    Ok(preparation)
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
            execute_requested: false,
            execute_embedding_requested: false,
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
        execute_requested: !deduplicated,
        execute_embedding_requested: false,
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
        whisper: collect_whisper_status_entry(repo_root, preparations.whisper),
        llama: collect_llama_status_entry(
            repo_root,
            preparations.llama,
            preparations.llama_embedding,
        ),
    })
}

fn collect_whisper_status_entry(
    repo_root: &Path,
    preparation: ModelPreparationRecord,
) -> ModelStatusEntry {
    match WhisperToolchain::discover(repo_root) {
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
                model: ModelKind::Whisper,
                available: true,
                ready,
                error,
                embedding_available: false,
                embedding_ready: false,
                embedding_error: None,
                preparation,
            }
        }
        Err(error) => ModelStatusEntry {
            model: ModelKind::Whisper,
            available: false,
            ready: false,
            error: Some(error),
            embedding_available: false,
            embedding_ready: false,
            embedding_error: None,
            preparation,
        },
    }
}

fn collect_llama_status_entry(
    repo_root: &Path,
    preparation: ModelPreparationRecord,
    embedding_preparation: ModelPreparationRecord,
) -> ModelStatusEntry {
    match LlamaToolchain::discover(repo_root) {
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
            let embedding_available = toolchain.llama_embedding_path.is_file();
            let embedding_ready = embedding_available && toolchain.is_embedding_model_ready();
            let embedding_error = if !embedding_available {
                Some(llama_embedding_toolchain_error(&toolchain))
            } else if embedding_ready {
                None
            } else {
                toolchain
                    .can_prepare_embedding_model()
                    .err()
                    .or_else(|| failed_model_preparation_error(&embedding_preparation))
            };

            ModelStatusEntry {
                model: ModelKind::Llama,
                available: true,
                ready,
                error,
                embedding_available,
                embedding_ready,
                embedding_error,
                preparation,
            }
        }
        Err(error) => ModelStatusEntry {
            model: ModelKind::Llama,
            available: false,
            ready: false,
            error: Some(error.clone()),
            embedding_available: false,
            embedding_ready: false,
            embedding_error: Some(error),
            preparation,
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

fn inspect_llama_embedding_preparation(repo_root: &Path) -> Result<ModelInspection, String> {
    let toolchain = LlamaToolchain::discover(repo_root)?;
    if !toolchain.llama_embedding_path.is_file() {
        return Err(llama_embedding_toolchain_error(&toolchain));
    }
    if toolchain.is_embedding_model_ready() {
        Ok(ModelInspection { ready: true })
    } else {
        toolchain.can_prepare_embedding_model()?;
        Ok(ModelInspection { ready: false })
    }
}

fn submit_llama_embedding_preparation(repo_root: &Path) -> Result<ModelPrepareSubmission, String> {
    let index_store = IndexStore::new(repo_root);
    let inspection = match inspect_llama_embedding_preparation(repo_root) {
        Ok(inspection) => inspection,
        Err(error) => {
            let _ = mark_llama_embedding_preparation_failed(repo_root, error.clone());
            return Err(error);
        }
    };

    if inspection.ready {
        let finished_at = now_rfc3339()?;
        let preparation = index_store.update_llama_embedding_preparation(|record| {
            record.mark_completed(finished_at.clone());
        })?;
        return Ok(ModelPrepareSubmission {
            model: ModelKind::Llama,
            disposition: ModelPrepareDisposition::AlreadyReady,
            preparation,
            execute_requested: false,
            execute_embedding_requested: false,
        });
    }

    let started_at = now_rfc3339()?;
    let mut deduplicated = false;
    let preparation = index_store.update_llama_embedding_preparation(|record| {
        if record.status == ModelPreparationStatus::Running && !is_model_preparation_stale(record) {
            deduplicated = true;
            return;
        }
        record.mark_running(started_at.clone());
    })?;

    Ok(ModelPrepareSubmission {
        model: ModelKind::Llama,
        disposition: if deduplicated {
            ModelPrepareDisposition::Deduplicated
        } else {
            ModelPrepareDisposition::Submitted
        },
        preparation,
        execute_requested: false,
        execute_embedding_requested: !deduplicated,
    })
}

fn execute_llama_embedding_preparation(repo_root: &Path) -> Result<ModelPreparationRecord, String> {
    let index_store = IndexStore::new(repo_root);
    let heartbeat_at = now_rfc3339()?;
    index_store.update_llama_embedding_preparation(|record| {
        if record.status == ModelPreparationStatus::Running {
            record.touch(heartbeat_at.clone());
        } else {
            record.mark_running(heartbeat_at.clone());
        }
    })?;

    let repo_root_for_heartbeat = repo_root.to_path_buf();
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let heartbeat_thread = thread::spawn(move || {
        loop {
            match stop_rx.recv_timeout(MODEL_PREPARATION_HEARTBEAT_INTERVAL) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if let Ok(heartbeat) = now_rfc3339() {
                        let _ = IndexStore::new(&repo_root_for_heartbeat)
                            .update_llama_embedding_preparation(|record| {
                                if record.status == ModelPreparationStatus::Running {
                                    record.touch(heartbeat.clone());
                                }
                            });
                    }
                }
            }
        }
    });

    let result = ensure_llama_embedding_model_with_toolchain(repo_root);
    drop(stop_tx);
    let _ = heartbeat_thread.join();

    match result {
        Ok(()) => {
            let finished_at = now_rfc3339()?;
            index_store.update_llama_embedding_preparation(|record| {
                record.mark_completed(finished_at.clone());
            })
        }
        Err(error) => {
            let finished_at = now_rfc3339()?;
            index_store.update_llama_embedding_preparation(|record| {
                record.mark_failed(finished_at.clone(), error.clone());
            })?;
            Err(error)
        }
    }
}

fn wait_for_llama_embedding_model_prepared(
    repo_root: &Path,
) -> Result<ModelPreparationRecord, String> {
    loop {
        let preparations = IndexStore::new(repo_root).model_preparations()?;
        let preparation = preparations.llama_embedding;
        let entry = collect_model_status_snapshot(repo_root)?.llama;
        if entry.embedding_ready {
            return Ok(preparation);
        }

        match preparation.status {
            ModelPreparationStatus::Running => {
                if is_model_preparation_stale(&preparation) {
                    let submission = submit_llama_embedding_preparation(repo_root)?;
                    if submission.already_ready() {
                        return Ok(submission.preparation);
                    }
                    if submission.should_execute() {
                        return execute_llama_embedding_preparation(repo_root);
                    }
                }
                thread::sleep(MODEL_PREPARATION_WAIT_POLL_INTERVAL);
            }
            ModelPreparationStatus::Failed => {
                return Err(entry.embedding_error.unwrap_or_else(|| {
                    preparation
                        .last_error
                        .unwrap_or_else(|| "llama embedding model preparation failed".to_string())
                }));
            }
            ModelPreparationStatus::Idle => {
                return Err("llama embedding model preparation is not running".to_string());
            }
            ModelPreparationStatus::Completed => {
                return Err(entry.embedding_error.unwrap_or_else(|| {
                    "llama embedding model is not ready after preparation".to_string()
                }));
            }
        }
    }
}

fn ensure_llama_embedding_model_prepared(
    repo_root: &Path,
) -> Result<ModelPreparationRecord, String> {
    let submission = submit_llama_embedding_preparation(repo_root)?;
    if submission.already_ready() {
        return Ok(submission.preparation);
    }
    if submission.should_execute() {
        return execute_llama_embedding_preparation(repo_root);
    }
    wait_for_llama_embedding_model_prepared(repo_root)
}

fn ensure_model_with_toolchain(repo_root: &Path, model: ModelKind) -> Result<(), String> {
    match model {
        ModelKind::Whisper => WhisperToolchain::discover(repo_root)?.ensure_model(),
        ModelKind::Llama => LlamaToolchain::discover(repo_root)?.ensure_model(),
    }
}

fn ensure_llama_embedding_model_with_toolchain(repo_root: &Path) -> Result<(), String> {
    let toolchain = LlamaToolchain::discover(repo_root)?;
    if !toolchain.llama_embedding_path.is_file() {
        return Err(llama_embedding_toolchain_error(&toolchain));
    }
    toolchain.ensure_embedding_model()
}

fn mark_llama_embedding_preparation_failed(
    repo_root: &Path,
    error: String,
) -> Result<ModelPreparationRecord, String> {
    let finished_at = now_rfc3339()?;
    IndexStore::new(repo_root).update_llama_embedding_preparation(|record| {
        record.mark_failed(finished_at.clone(), error.clone());
    })
}

fn llama_embedding_toolchain_error(toolchain: &LlamaToolchain) -> String {
    format!(
        "local llama embedding toolchain not found. Build it first with {}",
        toolchain.build_script_path.display()
    )
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
