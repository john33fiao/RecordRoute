use super::{
    MODEL_PREPARATION_HEARTBEAT_INTERVAL, MODEL_PREPARATION_STALE_THRESHOLD_SECS,
    MODEL_PREPARATION_WAIT_POLL_INTERVAL, ModelPrepareDisposition, ModelPrepareSubmission,
    ModelStatusEntry, ModelStatusSnapshot, now_rfc3339,
};
use crate::index::{IndexStore, ModelKind, ModelPreparationRecord, ModelPreparationStatus};
use crate::llama::{ModelSource, Toolchain as LlamaToolchain};
use crate::whisper::Toolchain as WhisperToolchain;
use std::path::{Path, PathBuf};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreparationTarget {
    Model(ModelKind),
    LlamaEmbedding,
}

impl PreparationTarget {
    fn update(
        self,
        index_store: &IndexStore,
        update: impl FnOnce(&mut ModelPreparationRecord),
    ) -> Result<ModelPreparationRecord, String> {
        match self {
            PreparationTarget::Model(model) => index_store.update_model_preparation(model, update),
            PreparationTarget::LlamaEmbedding => {
                index_store.update_llama_embedding_preparation(update)
            }
        }
    }

    fn idle_error(self) -> String {
        match self {
            PreparationTarget::Model(model) => {
                format!("{} model preparation is not running", model.as_str())
            }
            PreparationTarget::LlamaEmbedding => {
                "llama embedding model preparation is not running".to_string()
            }
        }
    }

    fn completed_not_ready_error(self) -> String {
        match self {
            PreparationTarget::Model(model) => {
                format!("{} model is not ready after preparation", model.as_str())
            }
            PreparationTarget::LlamaEmbedding => {
                "llama embedding model is not ready after preparation".to_string()
            }
        }
    }

    fn failed_default_error(self) -> String {
        match self {
            PreparationTarget::Model(model) => {
                format!("{} model preparation failed", model.as_str())
            }
            PreparationTarget::LlamaEmbedding => {
                "llama embedding model preparation failed".to_string()
            }
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
    let target = PreparationTarget::Model(model);
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
    execute_preparation_record(repo_root, PreparationTarget::Model(model), |root| {
        ensure_model_with_toolchain(root, model)
    })
}

pub fn wait_for_model_preparation(
    repo_root: &Path,
    model: ModelKind,
) -> Result<ModelPreparationRecord, String> {
    wait_for_preparation_state(
        repo_root,
        PreparationTarget::Model(model),
        |root| {
            let entry = collect_model_status_snapshot(root)?.entry(model).clone();
            Ok(PreparationWaitState {
                ready: entry.ready,
                error: entry.error,
                preparation: entry.preparation,
            })
        },
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
    Ok(ModelStatusSnapshot {
        whisper: collect_whisper_status_entry(repo_root, preparations.whisper),
        llama: collect_llama_status_entry(
            repo_root,
            preparations.llama,
            preparations.llama_embedding,
        ),
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
        return Ok((
            preparation,
            ModelPrepareDisposition::AlreadyReady,
            false,
        ));
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
    ensure: impl Fn(&Path) -> Result<(), String>,
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
                        let _ = target.update(
                            &IndexStore::new(&repo_root_for_heartbeat),
                            |record| {
                                if record.status == ModelPreparationStatus::Running {
                                    record.touch(heartbeat.clone());
                                }
                            },
                        );
                    }
                }
            }
        }
    });

    let result = ensure(repo_root);
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
    load_state: impl Fn(&Path) -> Result<PreparationWaitState, String>,
    resubmit: impl Fn(&Path) -> Result<ModelPrepareSubmission, String>,
    execute: impl Fn(&Path) -> Result<ModelPreparationRecord, String>,
) -> Result<ModelPreparationRecord, String> {
    loop {
        let state = load_state(repo_root)?;
        if state.ready {
            return Ok(state.preparation);
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
                return Err(
                    state
                        .error
                        .unwrap_or_else(|| target.completed_not_ready_error()),
                );
            }
        }
    }
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
    execute_preparation_record(
        repo_root,
        PreparationTarget::LlamaEmbedding,
        ensure_llama_embedding_model_with_toolchain,
    )
}

fn wait_for_llama_embedding_model_prepared(
    repo_root: &Path,
) -> Result<ModelPreparationRecord, String> {
    wait_for_preparation_state(
        repo_root,
        PreparationTarget::LlamaEmbedding,
        |root| {
            let preparation = IndexStore::new(root).model_preparations()?.llama_embedding;
            let entry = collect_model_status_snapshot(root)?.llama;
            Ok(PreparationWaitState {
                ready: entry.embedding_ready,
                error: entry.embedding_error,
                preparation,
            })
        },
        submit_llama_embedding_preparation,
        execute_llama_embedding_preparation,
    )
}

fn prepare_whisper_model_with_progress(repo_root: &Path) -> Result<ModelPreparationRecord, String> {
    let toolchain = WhisperToolchain::discover(repo_root)?;
    eprintln!(
        "Preparing whisper model at {}...",
        toolchain.model_path.display()
    );

    let preparation = prepare_model_with_progress(repo_root, ModelKind::Whisper)?;
    eprintln!("Whisper model ready.");
    Ok(preparation)
}

fn prepare_llama_summary_model_with_progress(
    repo_root: &Path,
) -> Result<ModelPreparationRecord, String> {
    let toolchain = LlamaToolchain::discover(repo_root)?;
    eprintln!(
        "Preparing llama summary model from {}...",
        describe_model_source(&toolchain.model_source, &toolchain.cached_model_path)
    );

    let preparation = prepare_model_with_progress(repo_root, ModelKind::Llama)?;
    eprintln!("Llama summary model ready.");
    Ok(preparation)
}

fn prepare_llama_embedding_model_with_progress(
    repo_root: &Path,
) -> Result<ModelPreparationRecord, String> {
    let toolchain = LlamaToolchain::discover(repo_root)?;
    eprintln!(
        "Preparing llama embedding model from {}...",
        describe_model_source(
            &toolchain.embedding_model_source,
            &toolchain.embedding_cached_model_path,
        )
    );

    let submission = submit_llama_embedding_preparation(repo_root)?;
    let preparation = if submission.already_ready() {
        submission.preparation
    } else if submission.should_execute() {
        execute_llama_embedding_preparation(repo_root)?
    } else {
        eprintln!("Llama embedding model preparation already running. Waiting...");
        wait_for_llama_embedding_model_prepared(repo_root)?
    };

    eprintln!("Llama embedding model ready.");
    Ok(preparation)
}

fn prepare_model_with_progress(
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

    eprintln!(
        "{} model preparation already running. Waiting...",
        model.as_str()
    );
    wait_for_model_preparation(repo_root, model)
}

fn describe_model_source(source: &ModelSource, cached_path: &Option<PathBuf>) -> String {
    match (source, cached_path) {
        (ModelSource::LocalPath(path), _) => path.display().to_string(),
        (ModelSource::HuggingFaceRepo(repo), Some(cache_path)) => {
            format!("{repo} -> {}", cache_path.display())
        }
        (ModelSource::HuggingFaceRepo(repo), None) => repo.clone(),
    }
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
    PreparationTarget::LlamaEmbedding.update(&IndexStore::new(repo_root), |record| {
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
