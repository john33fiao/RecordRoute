use super::{DispatchState, IndexStore, Path, PathBuf, QueueEntry, scheduler};
use crate::audio_store::AudioStore;
use crate::index::{IndexFile, QueuePayload};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

const AUDIO_CACHE_LOOKAHEAD: usize = 2;

#[derive(Debug, Clone, Default)]
struct AudioCacheRefreshPlan {
    warm_keys: Vec<String>,
    keep_keys: HashSet<String>,
}

#[derive(Debug, Default)]
struct AudioCacheCoordinatorState {
    previous_audio_keys: HashSet<String>,
    dispatch_state: DispatchState,
    pending_refresh: Option<AudioCacheRefreshPlan>,
    worker_running: bool,
}

#[derive(Debug)]
struct AudioCacheCoordinator {
    repo_root: PathBuf,
    state: Mutex<AudioCacheCoordinatorState>,
}

impl AudioCacheCoordinator {
    fn new(repo_root: PathBuf) -> Self {
        Self {
            repo_root,
            state: Mutex::new(AudioCacheCoordinatorState::default()),
        }
    }

    fn sync_dispatch_state(&self, dispatch_state: &DispatchState) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.dispatch_state = dispatch_state.clone();
    }

    fn snapshot_runtime(&self) -> (HashSet<String>, DispatchState) {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (
            state.previous_audio_keys.clone(),
            state.dispatch_state.clone(),
        )
    }

    fn replace_previous_audio_keys(&self, previous_audio_keys: HashSet<String>) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.previous_audio_keys = previous_audio_keys;
    }

    fn queue_refresh(self: &Arc<Self>, refresh: AudioCacheRefreshPlan) {
        let should_spawn = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.pending_refresh = Some(refresh);
            if state.worker_running {
                false
            } else {
                state.worker_running = true;
                true
            }
        };

        if !should_spawn {
            return;
        }

        let coordinator = Arc::clone(self);
        thread::spawn(move || coordinator.run_worker());
    }

    fn run_worker(self: Arc<Self>) {
        loop {
            let refresh = {
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                match state.pending_refresh.take() {
                    Some(refresh) => refresh,
                    None => {
                        state.worker_running = false;
                        return;
                    }
                }
            };

            if let Err(error) = self.apply_refresh(refresh) {
                eprintln!("{error}");
            }
        }
    }

    fn apply_refresh(&self, refresh: AudioCacheRefreshPlan) -> Result<(), String> {
        let audio_store = AudioStore::new(&self.repo_root)?;
        audio_store.apply_cache_policy(&refresh.warm_keys, &refresh.keep_keys)
    }

    #[cfg(test)]
    fn is_idle(&self) -> bool {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        !state.worker_running && state.pending_refresh.is_none()
    }
}

pub(crate) fn refresh_audio_cache(repo_root: &Path) {
    let coordinator = coordinator_for(repo_root);
    let (previous_audio_keys, dispatch_state) = coordinator.snapshot_runtime();
    match build_refresh_plan(repo_root, &previous_audio_keys, &dispatch_state) {
        Ok(refresh) => coordinator.queue_refresh(refresh),
        Err(error) => eprintln!("{error}"),
    }
}

pub(crate) fn sync_audio_cache_dispatch_state(repo_root: &Path, dispatch_state: &DispatchState) {
    coordinator_for(repo_root).sync_dispatch_state(dispatch_state);
}

pub(crate) fn record_previous_audio_keys(
    repo_root: &Path,
    entry: &QueueEntry,
) -> Result<(), String> {
    if !supports_audio_cache(entry) {
        return Ok(());
    }
    let index_store = IndexStore::new(repo_root);
    let previous_audio_keys = resolve_storage_keys_for_entry(&index_store, entry)?
        .into_iter()
        .collect::<HashSet<_>>();
    coordinator_for(repo_root).replace_previous_audio_keys(previous_audio_keys);
    Ok(())
}

fn build_refresh_plan(
    repo_root: &Path,
    previous_audio_keys: &HashSet<String>,
    dispatch_state: &DispatchState,
) -> Result<AudioCacheRefreshPlan, String> {
    let index_store = IndexStore::new(repo_root);
    let index = index_store.read_index()?;
    let mut warm_keys = Vec::new();
    let mut warm_seen = HashSet::new();
    let mut keep_keys = previous_audio_keys.clone();

    if let Some(running) = current_running_audio_entry(&index) {
        for storage_key in resolve_storage_keys_for_entry(&index_store, running)? {
            push_unique_key(&storage_key, &mut warm_keys, &mut warm_seen);
            keep_keys.insert(storage_key);
        }
    }

    for entry in
        collect_future_audio_entries(repo_root, &index, dispatch_state, AUDIO_CACHE_LOOKAHEAD)?
    {
        for storage_key in resolve_storage_keys_for_entry(&index_store, &entry)? {
            push_unique_key(&storage_key, &mut warm_keys, &mut warm_seen);
            keep_keys.insert(storage_key);
        }
    }

    Ok(AudioCacheRefreshPlan {
        warm_keys,
        keep_keys,
    })
}

fn current_running_audio_entry(index: &IndexFile) -> Option<&QueueEntry> {
    index
        .task_queue
        .active_batch
        .as_ref()
        .and_then(|batch| batch.running.as_ref())
        .filter(|entry| supports_audio_cache(entry))
}

fn collect_future_audio_entries(
    repo_root: &Path,
    index: &IndexFile,
    dispatch_state: &DispatchState,
    limit: usize,
) -> Result<Vec<QueueEntry>, String> {
    let mut simulated_index = index.clone();
    if let Some(running) = simulated_index
        .task_queue
        .active_batch
        .as_ref()
        .and_then(|batch| batch.running.clone())
    {
        scheduler::clear_running_entry_in_index(&mut simulated_index, &running);
    }

    let mut simulated_state = dispatch_state.clone();
    let mut future_audio_entries = Vec::new();
    while future_audio_entries.len() < limit {
        let Some(entry) = scheduler::reserve_next_entry_in_index(
            repo_root,
            &mut simulated_index,
            &mut simulated_state,
            false,
        )?
        else {
            break;
        };

        scheduler::clear_running_entry_in_index(&mut simulated_index, &entry);
        simulated_state.record_execution(entry.category);

        if supports_audio_cache(&entry) {
            future_audio_entries.push(entry);
        }
    }

    Ok(future_audio_entries)
}

fn supports_audio_cache(entry: &QueueEntry) -> bool {
    matches!(
        entry.payload,
        QueuePayload::Ffmpeg { .. } | QueuePayload::Stt { .. }
    )
}

fn resolve_storage_keys_for_entry(
    index_store: &IndexStore,
    entry: &QueueEntry,
) -> Result<Vec<String>, String> {
    match &entry.payload {
        QueuePayload::Ffmpeg { input_path } => Ok(vec![input_path.clone()]),
        QueuePayload::Stt { audio_files, .. } => Ok(audio_files
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>()
            .pipe(|requested| {
                crate::app::artifacts::resolve_audio_artifacts(
                    index_store,
                    &entry.job_id,
                    &requested,
                )
            })?
            .into_iter()
            .map(|artifact| artifact.storage_key)
            .collect()),
        QueuePayload::Summary { .. } | QueuePayload::Embedding => Ok(Vec::new()),
    }
}

fn push_unique_key(storage_key: &str, ordered: &mut Vec<String>, seen: &mut HashSet<String>) {
    if seen.insert(storage_key.to_string()) {
        ordered.push(storage_key.to_string());
    }
}

fn coordinator_for(repo_root: &Path) -> Arc<AudioCacheCoordinator> {
    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, Arc<AudioCacheCoordinator>>>> =
        OnceLock::new();
    let registry = REGISTRY.get_or_init(|| Mutex::new(HashMap::new()));
    let mut coordinators = registry
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    coordinators
        .entry(repo_root.to_path_buf())
        .or_insert_with(|| Arc::new(AudioCacheCoordinator::new(repo_root.to_path_buf())))
        .clone()
}

#[cfg(test)]
pub(crate) fn wait_for_audio_cache_refresh(repo_root: &Path) {
    let coordinator = coordinator_for(repo_root);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !coordinator.is_idle() {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for audio cache refresh"
        );
        thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(test)]
pub(crate) fn previous_audio_keys(repo_root: &Path) -> Vec<String> {
    let coordinator = coordinator_for(repo_root);
    let (previous_audio_keys, _) = coordinator.snapshot_runtime();
    let mut keys = previous_audio_keys.into_iter().collect::<Vec<_>>();
    keys.sort();
    keys
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}

#[cfg(test)]
mod tests {
    use super::{
        build_refresh_plan, collect_future_audio_entries, previous_audio_keys,
        record_previous_audio_keys, wait_for_audio_cache_refresh,
    };
    use crate::app::queue::{
        DispatchState, build_ffmpeg_entry, build_stt_entry_with_options, build_summary_entry,
        sync_audio_cache_dispatch_state,
    };
    use crate::index::{ActiveQueueBatch, IndexStore, JobRecord, QueueBatch, QueueCategory};
    use crate::test_support::{mark_job_completed_with_audio, test_job};
    use std::collections::HashSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    #[test]
    fn collect_future_audio_entries_honors_burst_rotation_into_pending_batches() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        for job_id in ["job-1", "job-2", "job-3"] {
            store
                .insert_job(JobRecord::new(
                    job_id.to_string(),
                    "2026-01-01T00:00:00Z".to_string(),
                    PathBuf::from(format!("/tmp/{job_id}.wav")),
                    store.job_dir(job_id),
                ))
                .expect("insert ffmpeg job");
        }

        store
            .with_index_mut(|index| {
                index.task_queue.burst_limit = 1;
                index.task_queue.active_batch = Some(ActiveQueueBatch {
                    category: QueueCategory::Ffmpeg,
                    running: None,
                    entries: vec![
                        build_ffmpeg_entry(
                            "job-1",
                            Path::new("sources/job-1/source.wav"),
                            "2026-01-01T00:00:00Z".to_string(),
                        ),
                        build_ffmpeg_entry(
                            "job-2",
                            Path::new("sources/job-2/source.wav"),
                            "2026-01-01T00:00:01Z".to_string(),
                        ),
                    ],
                });
                index.task_queue.pending_batches.push(QueueBatch {
                    category: QueueCategory::Llm,
                    entries: vec![build_summary_entry(
                        "job-3",
                        false,
                        "2026-01-01T00:00:02Z".to_string(),
                    )],
                });
                Ok(())
            })
            .expect("seed queue");

        let index = store.read_index().expect("read index");
        let dispatch_state = DispatchState {
            last_category: Some(QueueCategory::Ffmpeg),
            burst_count: 1,
        };

        let future_audio_entries =
            collect_future_audio_entries(&repo_root, &index, &dispatch_state, 2).expect("peek");
        assert_eq!(
            future_audio_entries
                .iter()
                .map(|entry| entry.job_id.as_str())
                .collect::<Vec<_>>(),
            vec!["job-1", "job-2"]
        );
    }

    #[test]
    fn collect_future_audio_entries_ignores_paused_flag_and_skips_non_audio_batches() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        store
            .insert_job(JobRecord::new(
                "job-audio".to_string(),
                "2026-01-01T00:00:00Z".to_string(),
                PathBuf::from("/tmp/job-audio.wav"),
                store.job_dir("job-audio"),
            ))
            .expect("insert audio job");

        store
            .with_index_mut(|index| {
                index.task_queue.paused = true;
                index.task_queue.active_batch = Some(ActiveQueueBatch {
                    category: QueueCategory::Llm,
                    running: None,
                    entries: vec![build_summary_entry(
                        "job-summary",
                        false,
                        "2026-01-01T00:00:00Z".to_string(),
                    )],
                });
                index.task_queue.pending_batches.push(QueueBatch {
                    category: QueueCategory::Ffmpeg,
                    entries: vec![build_ffmpeg_entry(
                        "job-audio",
                        Path::new("sources/job-audio/source.wav"),
                        "2026-01-01T00:00:01Z".to_string(),
                    )],
                });
                Ok(())
            })
            .expect("seed paused queue");

        let index = store.read_index().expect("read index");
        let future_audio_entries =
            collect_future_audio_entries(&repo_root, &index, &DispatchState::default(), 2)
                .expect("peek");
        assert_eq!(
            future_audio_entries
                .iter()
                .map(|entry| entry.job_id.as_str())
                .collect::<Vec<_>>(),
            vec!["job-audio"]
        );
    }

    #[test]
    fn build_refresh_plan_keeps_previous_and_warms_running_plus_next_audio_entries() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);

        let mut stt_job = test_job(
            "job-stt",
            "2026-01-01T00:00:00Z",
            "sources/job-stt/source.wav",
            "hash-job-stt",
            "job-stt.wav",
        );
        mark_job_completed_with_audio(
            &store,
            &mut stt_job,
            "2026-01-01T00:00:01Z",
            &["mono_mix.wav"],
        )
        .expect("seed stt job");
        store.insert_job(stt_job).expect("insert stt job");

        store
            .with_index_mut(|index| {
                index.task_queue.active_batch = Some(ActiveQueueBatch {
                    category: QueueCategory::Ffmpeg,
                    running: Some(build_ffmpeg_entry(
                        "job-running",
                        Path::new("sources/job-running/source.wav"),
                        "2026-01-01T00:00:00Z".to_string(),
                    )),
                    entries: vec![build_stt_entry_with_options(
                        "job-stt",
                        &[PathBuf::from("mono_mix.wav")],
                        "ko",
                        &[],
                        "2026-01-01T00:00:01Z".to_string(),
                    )],
                });
                Ok(())
            })
            .expect("seed running queue");

        let refresh = build_refresh_plan(
            &repo_root,
            &HashSet::from(["sources/previous/source.wav".to_string()]),
            &DispatchState::default(),
        )
        .expect("build refresh");

        assert_eq!(
            refresh.warm_keys,
            vec![
                "sources/job-running/source.wav".to_string(),
                "jobs/job-stt/mono_mix.wav".to_string()
            ]
        );
        assert!(refresh.keep_keys.contains("sources/previous/source.wav"));
    }

    #[test]
    fn previous_audio_keys_are_isolated_per_repo_root() {
        let repo_one = temp_workspace();
        let repo_two = temp_workspace();
        let entry = build_ffmpeg_entry(
            "job-1",
            Path::new("sources/hash/source.wav"),
            "2026-01-01T00:00:00Z".to_string(),
        );

        record_previous_audio_keys(&repo_one, &entry).expect("record previous repo one");
        sync_audio_cache_dispatch_state(&repo_one, &DispatchState::default());
        sync_audio_cache_dispatch_state(&repo_two, &DispatchState::default());
        wait_for_audio_cache_refresh(&repo_one);
        wait_for_audio_cache_refresh(&repo_two);

        assert_eq!(
            previous_audio_keys(&repo_one),
            vec!["sources/hash/source.wav".to_string()]
        );
        assert!(previous_audio_keys(&repo_two).is_empty());
    }

    fn temp_workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!("recordroute-audio-cache-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).expect("temp workspace");
        path
    }
}
