use super::{
    MODEL_PREPARATION_HEARTBEAT_INTERVAL, MODEL_PREPARATION_STALE_THRESHOLD_SECS,
    MODEL_PREPARATION_WAIT_POLL_INTERVAL, ModelPrepareDisposition, ModelPrepareSubmission,
    ModelStatusEntry, ModelStatusSnapshot, now_rfc3339,
};
use crate::error::{AppError, AppResult, dependency_unavailable_or_internal};
use crate::index::{IndexStore, ModelKind, ModelPreparationRecord, ModelPreparationStatus};
use crate::llama::{
    Toolchain as LlamaToolchain, embedding_model_description, missing_embedding_toolchain_message,
    summary_model_description,
};
use crate::whisper::{
    Toolchain as WhisperToolchain, model_description as whisper_model_description,
};
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModelInspection {
    ready: bool,
}

#[derive(Debug, Clone)]
struct PreparationWaitState {
    ready: bool,
    error: Option<String>,
    preparation: ModelPreparationRecord,
}

#[derive(Debug, Clone)]
struct TargetRuntimeStatus {
    available: bool,
    ready: bool,
    error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreparationTarget {
    Whisper,
    LlamaSummary,
    LlamaEmbedding,
}

impl PreparationTarget {
    fn from_model_kind(model: ModelKind) -> Self {
        match model {
            ModelKind::Whisper => Self::Whisper,
            ModelKind::Llama => Self::LlamaSummary,
        }
    }

    fn model_kind(self) -> ModelKind {
        match self {
            Self::Whisper => ModelKind::Whisper,
            Self::LlamaSummary | Self::LlamaEmbedding => ModelKind::Llama,
        }
    }

    fn update(
        self,
        index_store: &IndexStore,
        update: impl FnOnce(&mut ModelPreparationRecord),
    ) -> Result<ModelPreparationRecord, String> {
        match self {
            Self::Whisper => index_store.update_model_preparation(ModelKind::Whisper, update),
            Self::LlamaSummary => index_store.update_model_preparation(ModelKind::Llama, update),
            Self::LlamaEmbedding => index_store.update_llama_embedding_preparation(update),
        }
    }

    fn preparation_record(
        self,
        index_store: &IndexStore,
    ) -> Result<ModelPreparationRecord, String> {
        let preparations = index_store.model_preparations()?;
        Ok(match self {
            Self::Whisper => preparations.whisper,
            Self::LlamaSummary => preparations.llama,
            Self::LlamaEmbedding => preparations.llama_embedding,
        })
    }

    fn inspect(self, repo_root: &Path) -> Result<ModelInspection, String> {
        match self {
            Self::Whisper => {
                let toolchain = WhisperToolchain::discover(repo_root)?;
                if toolchain.is_model_ready() {
                    Ok(ModelInspection { ready: true })
                } else {
                    toolchain.can_prepare_model()?;
                    Ok(ModelInspection { ready: false })
                }
            }
            Self::LlamaSummary => {
                let toolchain = LlamaToolchain::discover(repo_root)?;
                if toolchain.is_model_ready() {
                    Ok(ModelInspection { ready: true })
                } else {
                    toolchain.can_prepare_model()?;
                    Ok(ModelInspection { ready: false })
                }
            }
            Self::LlamaEmbedding => {
                let toolchain = LlamaToolchain::discover(repo_root)?;
                if !toolchain.llama_embedding_path.is_file() {
                    return Err(missing_embedding_toolchain_message(&toolchain));
                }
                if toolchain.is_embedding_model_ready() {
                    Ok(ModelInspection { ready: true })
                } else {
                    toolchain.can_prepare_embedding_model()?;
                    Ok(ModelInspection { ready: false })
                }
            }
        }
    }

    fn ensure(self, repo_root: &Path) -> Result<(), String> {
        match self {
            Self::Whisper => WhisperToolchain::discover(repo_root)?.ensure_model(),
            Self::LlamaSummary => LlamaToolchain::discover(repo_root)?.ensure_model(),
            Self::LlamaEmbedding => {
                let toolchain = LlamaToolchain::discover(repo_root)?;
                if !toolchain.llama_embedding_path.is_file() {
                    return Err(missing_embedding_toolchain_message(&toolchain));
                }
                toolchain.ensure_embedding_model()
            }
        }
    }

    fn status(self, repo_root: &Path, preparation: &ModelPreparationRecord) -> TargetRuntimeStatus {
        match self {
            Self::Whisper => match WhisperToolchain::discover(repo_root) {
                Ok(toolchain) => {
                    let ready = toolchain.is_model_ready();
                    let error = if ready {
                        None
                    } else {
                        toolchain
                            .can_prepare_model()
                            .err()
                            .or_else(|| failed_model_preparation_error(preparation))
                    };
                    TargetRuntimeStatus {
                        available: true,
                        ready,
                        error,
                    }
                }
                Err(error) => TargetRuntimeStatus {
                    available: false,
                    ready: false,
                    error: Some(error),
                },
            },
            Self::LlamaSummary => match LlamaToolchain::discover(repo_root) {
                Ok(toolchain) => {
                    let ready = toolchain.is_model_ready();
                    let error = if ready {
                        None
                    } else {
                        toolchain
                            .can_prepare_model()
                            .err()
                            .or_else(|| failed_model_preparation_error(preparation))
                    };
                    TargetRuntimeStatus {
                        available: true,
                        ready,
                        error,
                    }
                }
                Err(error) => TargetRuntimeStatus {
                    available: false,
                    ready: false,
                    error: Some(error),
                },
            },
            Self::LlamaEmbedding => match LlamaToolchain::discover(repo_root) {
                Ok(toolchain) => {
                    let available = toolchain.llama_embedding_path.is_file();
                    let ready = available && toolchain.is_embedding_model_ready();
                    let error = if !available {
                        Some(missing_embedding_toolchain_message(&toolchain))
                    } else if ready {
                        None
                    } else {
                        toolchain
                            .can_prepare_embedding_model()
                            .err()
                            .or_else(|| failed_model_preparation_error(preparation))
                    };
                    TargetRuntimeStatus {
                        available,
                        ready,
                        error,
                    }
                }
                Err(error) => TargetRuntimeStatus {
                    available: false,
                    ready: false,
                    error: Some(error),
                },
            },
        }
    }

    fn load_wait_state(self, repo_root: &Path) -> Result<PreparationWaitState, String> {
        match self {
            Self::Whisper | Self::LlamaSummary => {
                let entry = collect_model_status_snapshot(repo_root)?
                    .entry(self.model_kind())
                    .clone();
                Ok(PreparationWaitState {
                    ready: entry.ready,
                    error: entry.error,
                    preparation: entry.preparation,
                })
            }
            Self::LlamaEmbedding => {
                let index_store = IndexStore::new(repo_root);
                let preparation = self.preparation_record(&index_store)?;
                let status = self.status(repo_root, &preparation);
                Ok(PreparationWaitState {
                    ready: status.ready,
                    error: status.error,
                    preparation,
                })
            }
        }
    }

    fn submit(self, repo_root: &Path) -> Result<ModelPrepareSubmission, String> {
        match self {
            Self::Whisper | Self::LlamaSummary => {
                submit_model_preparation(repo_root, self.model_kind())
            }
            Self::LlamaEmbedding => submit_llama_embedding_preparation(repo_root),
        }
    }

    fn execute(self, repo_root: &Path) -> Result<ModelPreparationRecord, String> {
        match self {
            Self::Whisper | Self::LlamaSummary => {
                execute_model_preparation(repo_root, self.model_kind())
            }
            Self::LlamaEmbedding => execute_llama_embedding_preparation(repo_root),
        }
    }

    fn wait(self, repo_root: &Path) -> Result<ModelPreparationRecord, String> {
        match self {
            Self::Whisper | Self::LlamaSummary => {
                wait_for_model_preparation(repo_root, self.model_kind())
            }
            Self::LlamaEmbedding => wait_for_llama_embedding_model_prepared(repo_root),
        }
    }

    fn log_prepare_start(self, repo_root: &Path) -> Result<(), String> {
        match self {
            Self::Whisper => {
                let toolchain = WhisperToolchain::discover(repo_root)?;
                eprintln!(
                    "Preparing whisper model at {}...",
                    whisper_model_description(&toolchain)
                );
            }
            Self::LlamaSummary => {
                let toolchain = LlamaToolchain::discover(repo_root)?;
                eprintln!(
                    "Preparing llama summary model from {}...",
                    summary_model_description(&toolchain)
                );
            }
            Self::LlamaEmbedding => {
                let toolchain = LlamaToolchain::discover(repo_root)?;
                eprintln!(
                    "Preparing llama embedding model from {}...",
                    embedding_model_description(&toolchain)
                );
            }
        }
        Ok(())
    }

    fn log_wait_message(self) {
        match self {
            Self::Whisper => eprintln!("whisper model preparation already running. Waiting..."),
            Self::LlamaSummary => eprintln!("llama model preparation already running. Waiting..."),
            Self::LlamaEmbedding => {
                eprintln!("Llama embedding model preparation already running. Waiting...")
            }
        }
    }

    fn log_prepare_ready(self) {
        match self {
            Self::Whisper => eprintln!("Whisper model ready."),
            Self::LlamaSummary => eprintln!("Llama summary model ready."),
            Self::LlamaEmbedding => eprintln!("Llama embedding model ready."),
        }
    }

    fn idle_error(self) -> String {
        match self {
            Self::Whisper => "whisper model preparation is not running".to_string(),
            Self::LlamaSummary => "llama model preparation is not running".to_string(),
            Self::LlamaEmbedding => "llama embedding model preparation is not running".to_string(),
        }
    }

    fn completed_not_ready_error(self) -> String {
        match self {
            Self::Whisper => "whisper model is not ready after preparation".to_string(),
            Self::LlamaSummary => "llama model is not ready after preparation".to_string(),
            Self::LlamaEmbedding => {
                "llama embedding model is not ready after preparation".to_string()
            }
        }
    }

    fn failed_default_error(self) -> String {
        match self {
            Self::Whisper => "whisper model preparation failed".to_string(),
            Self::LlamaSummary => "llama model preparation failed".to_string(),
            Self::LlamaEmbedding => "llama embedding model preparation failed".to_string(),
        }
    }
}

pub fn prepare_llama_model_with_repo_root(repo_root: &Path) -> Result<(), String> {
    let _ = prepare_llama_summary_model_with_progress(repo_root)?;
    let _ = prepare_llama_embedding_model_with_progress(repo_root)?;
    Ok(())
}

pub fn prepare_models_with_repo_root(repo_root: &Path) -> Result<(), String> {
    eprintln!("Preparing runtime models...");
    let _ = prepare_whisper_model_with_progress(repo_root)?;
    prepare_llama_model_with_repo_root(repo_root)?;
    eprintln!("Runtime models ready.");
    Ok(())
}

pub fn submit_model_preparation_api(
    repo_root: &Path,
    model: ModelKind,
) -> AppResult<ModelPrepareSubmission> {
    submit_model_preparation(repo_root, model).map_err(dependency_unavailable_or_internal)
}

pub fn submit_llama_umbrella_preparation_api(
    repo_root: &Path,
) -> AppResult<ModelPrepareSubmission> {
    submit_llama_umbrella_preparation(repo_root).map_err(dependency_unavailable_or_internal)
}

pub fn execute_model_preparation_api(
    repo_root: &Path,
    model: ModelKind,
) -> AppResult<ModelPreparationRecord> {
    execute_model_preparation(repo_root, model).map_err(dependency_unavailable_or_internal)
}

pub fn execute_llama_umbrella_preparation_api(
    repo_root: &Path,
    submission: &ModelPrepareSubmission,
) -> AppResult<ModelPreparationRecord> {
    execute_llama_umbrella_preparation(repo_root, submission)
        .map_err(dependency_unavailable_or_internal)
}

pub fn collect_model_status_snapshot_api(repo_root: &Path) -> AppResult<ModelStatusSnapshot> {
    collect_model_status_snapshot(repo_root).map_err(AppError::internal)
}

pub fn submit_llama_umbrella_preparation(
    repo_root: &Path,
) -> Result<ModelPrepareSubmission, String> {
    PreparationTarget::LlamaSummary.inspect(repo_root)?;
    if let Err(error) = PreparationTarget::LlamaEmbedding.inspect(repo_root) {
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
    let target = PreparationTarget::from_model_kind(model);
    let inspection = target.inspect(repo_root)?;
    let (preparation, disposition, execute_requested) =
        submit_preparation_record(&index_store, target, inspection)?;

    Ok(ModelPrepareSubmission {
        model,
        disposition,
        preparation,
        execute_requested,
        execute_embedding_requested: false,
    })
}

pub fn execute_model_preparation(
    repo_root: &Path,
    model: ModelKind,
) -> Result<ModelPreparationRecord, String> {
    execute_preparation_record(repo_root, PreparationTarget::from_model_kind(model))
}

pub fn wait_for_model_preparation(
    repo_root: &Path,
    model: ModelKind,
) -> Result<ModelPreparationRecord, String> {
    let target = PreparationTarget::from_model_kind(model);
    wait_for_preparation_state(
        repo_root,
        target,
        |root| submit_model_preparation(root, model),
        |root| execute_model_preparation(root, model),
    )
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
    let whisper = PreparationTarget::Whisper.status(repo_root, &preparations.whisper);
    let llama_summary = PreparationTarget::LlamaSummary.status(repo_root, &preparations.llama);
    let llama_embedding =
        PreparationTarget::LlamaEmbedding.status(repo_root, &preparations.llama_embedding);
    Ok(ModelStatusSnapshot {
        whisper: ModelStatusEntry {
            model: ModelKind::Whisper,
            available: whisper.available,
            ready: whisper.ready,
            error: whisper.error,
            embedding_available: false,
            embedding_ready: false,
            embedding_error: None,
            preparation: preparations.whisper,
        },
        llama: ModelStatusEntry {
            model: ModelKind::Llama,
            available: llama_summary.available,
            ready: llama_summary.ready,
            error: llama_summary.error,
            embedding_available: llama_embedding.available,
            embedding_ready: llama_embedding.ready,
            embedding_error: llama_embedding.error,
            preparation: preparations.llama,
        },
    })
}

fn submit_preparation_record(
    index_store: &IndexStore,
    target: PreparationTarget,
    inspection: ModelInspection,
) -> Result<(ModelPreparationRecord, ModelPrepareDisposition, bool), String> {
    if inspection.ready {
        let finished_at = now_rfc3339()?;
        let preparation = target.update(index_store, |record| {
            record.mark_completed(finished_at.clone());
        })?;
        return Ok((preparation, ModelPrepareDisposition::AlreadyReady, false));
    }

    let started_at = now_rfc3339()?;
    let mut deduplicated = false;
    let preparation = target.update(index_store, |record| {
        if record.status == ModelPreparationStatus::Running && !is_model_preparation_stale(record) {
            deduplicated = true;
            return;
        }
        record.mark_running(started_at.clone());
    })?;

    Ok((
        preparation,
        if deduplicated {
            ModelPrepareDisposition::Deduplicated
        } else {
            ModelPrepareDisposition::Submitted
        },
        !deduplicated,
    ))
}

fn execute_preparation_record(
    repo_root: &Path,
    target: PreparationTarget,
) -> Result<ModelPreparationRecord, String> {
    let index_store = IndexStore::new(repo_root);
    let heartbeat_at = now_rfc3339()?;
    target.update(&index_store, |record| {
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
                        let _ =
                            target.update(&IndexStore::new(&repo_root_for_heartbeat), |record| {
                                if record.status == ModelPreparationStatus::Running {
                                    record.touch(heartbeat.clone());
                                }
                            });
                    }
                }
            }
        }
    });

    let result = target.ensure(repo_root);
    drop(stop_tx);
    let _ = heartbeat_thread.join();

    match result {
        Ok(()) => {
            let finished_at = now_rfc3339()?;
            target.update(&index_store, |record| {
                record.mark_completed(finished_at.clone());
            })
        }
        Err(error) => {
            let finished_at = now_rfc3339()?;
            target.update(&index_store, |record| {
                record.mark_failed(finished_at.clone(), error.clone());
            })?;
            Err(error)
        }
    }
}

fn wait_for_preparation_state(
    repo_root: &Path,
    target: PreparationTarget,
    resubmit: impl Fn(&Path) -> Result<ModelPrepareSubmission, String>,
    execute: impl Fn(&Path) -> Result<ModelPreparationRecord, String>,
) -> Result<ModelPreparationRecord, String> {
    loop {
        let state = target.load_wait_state(repo_root)?;
        if state.ready {
            if state.preparation.status == ModelPreparationStatus::Completed {
                return Ok(state.preparation);
            }

            return Ok(resubmit(repo_root)?.preparation);
        }

        match state.preparation.status {
            ModelPreparationStatus::Running => {
                if is_model_preparation_stale(&state.preparation) {
                    let submission = resubmit(repo_root)?;
                    if submission.already_ready() {
                        return Ok(submission.preparation);
                    }
                    if submission.should_execute() {
                        return execute(repo_root);
                    }
                }
                thread::sleep(MODEL_PREPARATION_WAIT_POLL_INTERVAL);
            }
            ModelPreparationStatus::Failed => {
                return Err(state.error.unwrap_or_else(|| {
                    state
                        .preparation
                        .last_error
                        .unwrap_or_else(|| target.failed_default_error())
                }));
            }
            ModelPreparationStatus::Idle => return Err(target.idle_error()),
            ModelPreparationStatus::Completed => {
                return Err(state
                    .error
                    .unwrap_or_else(|| target.completed_not_ready_error()));
            }
        }
    }
}

fn failed_model_preparation_error(preparation: &ModelPreparationRecord) -> Option<String> {
    if preparation.status == ModelPreparationStatus::Failed {
        preparation.last_error.clone()
    } else {
        None
    }
}

fn submit_llama_embedding_preparation(repo_root: &Path) -> Result<ModelPrepareSubmission, String> {
    let index_store = IndexStore::new(repo_root);
    let inspection = match PreparationTarget::LlamaEmbedding.inspect(repo_root) {
        Ok(inspection) => inspection,
        Err(error) => {
            let _ = mark_llama_embedding_preparation_failed(repo_root, error.clone());
            return Err(error);
        }
    };
    let (preparation, disposition, execute_requested) =
        submit_preparation_record(&index_store, PreparationTarget::LlamaEmbedding, inspection)?;

    Ok(ModelPrepareSubmission {
        model: ModelKind::Llama,
        disposition,
        preparation,
        execute_requested: false,
        execute_embedding_requested: execute_requested,
    })
}

fn execute_llama_embedding_preparation(repo_root: &Path) -> Result<ModelPreparationRecord, String> {
    execute_preparation_record(repo_root, PreparationTarget::LlamaEmbedding)
}

fn wait_for_llama_embedding_model_prepared(
    repo_root: &Path,
) -> Result<ModelPreparationRecord, String> {
    wait_for_preparation_state(
        repo_root,
        PreparationTarget::LlamaEmbedding,
        submit_llama_embedding_preparation,
        execute_llama_embedding_preparation,
    )
}

fn prepare_whisper_model_with_progress(repo_root: &Path) -> Result<ModelPreparationRecord, String> {
    prepare_target_with_progress(repo_root, PreparationTarget::Whisper)
}

fn prepare_llama_summary_model_with_progress(
    repo_root: &Path,
) -> Result<ModelPreparationRecord, String> {
    prepare_target_with_progress(repo_root, PreparationTarget::LlamaSummary)
}

fn prepare_llama_embedding_model_with_progress(
    repo_root: &Path,
) -> Result<ModelPreparationRecord, String> {
    prepare_target_with_progress(repo_root, PreparationTarget::LlamaEmbedding)
}

fn prepare_target_with_progress(
    repo_root: &Path,
    target: PreparationTarget,
) -> Result<ModelPreparationRecord, String> {
    target.log_prepare_start(repo_root)?;
    let submission = target.submit(repo_root)?;
    let preparation = if submission.already_ready() {
        submission.preparation
    } else if submission.should_execute() {
        target.execute(repo_root)?
    } else {
        target.log_wait_message();
        target.wait(repo_root)?
    };

    target.log_prepare_ready();
    Ok(preparation)
}

fn mark_llama_embedding_preparation_failed(
    repo_root: &Path,
    error: String,
) -> Result<ModelPreparationRecord, String> {
    let finished_at = now_rfc3339()?;
    PreparationTarget::LlamaEmbedding.update(&IndexStore::new(repo_root), |record| {
        record.mark_failed(finished_at.clone(), error.clone());
    })
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
