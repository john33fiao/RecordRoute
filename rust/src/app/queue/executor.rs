use super::{
    DispatchState, IndexStore, Path, PathBuf, QueueEntry, TaskType, artifacts, embedding_stage,
    ffmpeg_stage, now_rfc3339, stt_stage, summary_stage,
};
#[cfg(not(test))]
use crate::audio_store::AudioStore;
use crate::index::IndexFile;
use crate::index::{QueuePayload, TaskStatus};
use std::collections::HashSet;
#[cfg(not(test))]
use std::thread;

const AUDIO_PRELOAD_WINDOW_SIZE: usize = 3;

pub fn recover_interrupted_active_entry(repo_root: &Path) -> Result<(), String> {
    let queued_at = now_rfc3339()?;
    IndexStore::new(repo_root).with_index_mut(|index| {
        let running_entry = index
            .task_queue
            .active_batch
            .as_mut()
            .and_then(|active_batch| active_batch.running.take());
        let Some(entry) = running_entry else {
            return Ok(());
        };

        super::scheduler::requeue_entry(index, &entry, queued_at.clone())?;
        if let Some(active_batch) = index.task_queue.active_batch.as_mut() {
            active_batch.entries.insert(0, entry);
        }
        Ok(())
    })
}

pub fn dispatch_one(repo_root: &Path, state: &mut DispatchState) -> Result<bool, String> {
    dispatch_one_internal(repo_root, state, true)
}

fn dispatch_one_without_pause(repo_root: &Path, state: &mut DispatchState) -> Result<bool, String> {
    dispatch_one_internal(repo_root, state, false)
}

fn dispatch_one_internal(
    repo_root: &Path,
    state: &mut DispatchState,
    honor_pause: bool,
) -> Result<bool, String> {
    let started_at = now_rfc3339()?;
    let Some(work) =
        super::scheduler::reserve_next_entry(repo_root, state, started_at, honor_pause)?
    else {
        return Ok(false);
    };

    refresh_audio_preload_window(repo_root);
    let execution_result = execute_work_item(repo_root, &work.entry);
    let clear_result = super::scheduler::clear_running_entry(repo_root, &work.entry);

    if let Err(error) = clear_result {
        eprintln!("{error}");
    }
    refresh_audio_preload_window(repo_root);

    state.record_execution(work.entry.category);

    if let Err(error) = execution_result {
        eprintln!("{error}");
    }

    Ok(true)
}

pub fn dispatch_until_task_terminal(
    repo_root: &Path,
    job_id: &str,
    task_type: TaskType,
) -> Result<(), String> {
    let mut state = DispatchState::default();

    loop {
        let job = IndexStore::new(repo_root)
            .find_job(job_id)?
            .ok_or_else(|| format!("job not found in index: {job_id}"))?;
        let task = job
            .task(task_type)
            .cloned()
            .ok_or_else(|| format!("task not found for job {job_id}: {}", task_type.as_str()))?;

        match task.status {
            TaskStatus::Queued | TaskStatus::Running => {
                if !dispatch_one_without_pause(repo_root, &mut state)? {
                    return Err(format!(
                        "queue stalled before {} task completed for job {job_id}",
                        task_type.as_str()
                    ));
                }
            }
            TaskStatus::Completed => return Ok(()),
            TaskStatus::Failed => {
                return Err(task.last_error.unwrap_or_else(|| {
                    format!("{} task failed for job {job_id}", task_type.as_str())
                }));
            }
        }
    }
}

fn execute_work_item(repo_root: &Path, entry: &QueueEntry) -> Result<(), String> {
    match &entry.payload {
        QueuePayload::Ffmpeg { input_path } => {
            ffmpeg_stage::execute_ffmpeg_job(repo_root, &entry.job_id, Path::new(input_path))?;
            Ok(())
        }
        QueuePayload::Stt {
            audio_files,
            language,
            keywords,
        } => {
            let paths = if audio_files.is_empty() {
                artifacts::listed_or_discovered_audio_files(&IndexStore::new(repo_root), &entry.job_id)?
            } else {
                audio_files.iter().map(PathBuf::from).collect::<Vec<_>>()
            };
            stt_stage::execute_stt_job(repo_root, &entry.job_id, &paths, language, keywords)?;
            Ok(())
        }
        QueuePayload::Summary { force_regenerate } => {
            summary_stage::execute_summary_job(repo_root, &entry.job_id, *force_regenerate)?;
            Ok(())
        }
        QueuePayload::Embedding => {
            embedding_stage::execute_summary_embedding_job(repo_root, &entry.job_id)?;
            Ok(())
        }
    }
}

fn refresh_audio_preload_window(repo_root: &Path) {
    match collect_audio_preload_storage_keys(repo_root) {
        Ok(storage_keys) if !storage_keys.is_empty() => {
            #[cfg(test)]
            {
                return;
            }

            #[cfg(not(test))]
            let repo_root = repo_root.to_path_buf();
            #[cfg(not(test))]
            {
                thread::spawn(move || {
                    if let Err(error) = materialize_storage_keys(&repo_root, &storage_keys) {
                        eprintln!("{error}");
                    }
                });
            }
        }
        Ok(_) => {}
        Err(error) => eprintln!("{error}"),
    }
}

fn collect_audio_preload_entries(index: &IndexFile) -> Result<Vec<QueueEntry>, String> {
    let Some(active_batch) = index.task_queue.active_batch.as_ref() else {
        return Ok(Vec::new());
    };

    let mut entries = Vec::with_capacity(AUDIO_PRELOAD_WINDOW_SIZE);
    if let Some(running) = active_batch.running.as_ref()
        && supports_audio_preload(running)
    {
        entries.push(running.clone());
    }

    for entry in &active_batch.entries {
        if entries.len() >= AUDIO_PRELOAD_WINDOW_SIZE {
            break;
        }
        if supports_audio_preload(entry) {
            entries.push(entry.clone());
        }
    }

    Ok(entries)
}

fn supports_audio_preload(entry: &QueueEntry) -> bool {
    matches!(
        entry.payload,
        QueuePayload::Ffmpeg { .. } | QueuePayload::Stt { .. }
    )
}

fn collect_audio_preload_storage_keys(repo_root: &Path) -> Result<Vec<String>, String> {
    let index_store = IndexStore::new(repo_root);
    let entries = index_store.with_index_read(collect_audio_preload_entries)?;
    let mut warmed = HashSet::new();
    let mut storage_keys = Vec::new();

    for entry in entries {
        for storage_key in preload_storage_keys_for_entry(&index_store, &entry)? {
            if warmed.insert(storage_key.clone()) {
                storage_keys.push(storage_key);
            }
        }
    }

    Ok(storage_keys)
}

fn preload_storage_keys_for_entry(
    index_store: &IndexStore,
    entry: &QueueEntry,
) -> Result<Vec<String>, String> {
    match &entry.payload {
        QueuePayload::Ffmpeg { input_path } => Ok(vec![input_path.clone()]),
        QueuePayload::Stt { audio_files, .. } => {
            let requested = audio_files.iter().map(PathBuf::from).collect::<Vec<_>>();
            Ok(artifacts::resolve_audio_artifacts(index_store, &entry.job_id, &requested)?
                .into_iter()
                .map(|artifact| artifact.storage_key)
                .collect())
        }
        QueuePayload::Summary { .. } | QueuePayload::Embedding => Ok(Vec::new()),
    }
}

#[cfg(not(test))]
fn materialize_storage_keys(repo_root: &Path, storage_keys: &[String]) -> Result<(), String> {
    let audio_store = AudioStore::new(repo_root)?;
    for storage_key in storage_keys {
        let _ = audio_store.materialize_to_cache(storage_key);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        collect_audio_preload_entries, preload_storage_keys_for_entry, AUDIO_PRELOAD_WINDOW_SIZE,
    };
    use crate::app::queue::{build_ffmpeg_entry, build_stt_entry_with_options, build_summary_entry};
    use crate::index::{ActiveQueueBatch, IndexFile, IndexStore, QueueCategory};
    use crate::test_support::{mark_job_completed_with_audio, test_job};
    use std::fs;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    #[test]
    fn collect_audio_preload_entries_limits_window_to_running_and_next_two() {
        let mut index = IndexFile::empty();
        index.task_queue.active_batch = Some(ActiveQueueBatch {
            category: QueueCategory::Ffmpeg,
            running: Some(build_ffmpeg_entry(
                "job-0",
                Path::new("sources/job-0/source.wav"),
                "2026-01-01T00:00:00Z".to_string(),
            )),
            entries: vec![
                build_ffmpeg_entry(
                    "job-1",
                    Path::new("sources/job-1/source.wav"),
                    "2026-01-01T00:00:01Z".to_string(),
                ),
                build_ffmpeg_entry(
                    "job-2",
                    Path::new("sources/job-2/source.wav"),
                    "2026-01-01T00:00:02Z".to_string(),
                ),
                build_ffmpeg_entry(
                    "job-3",
                    Path::new("sources/job-3/source.wav"),
                    "2026-01-01T00:00:03Z".to_string(),
                ),
            ],
        });

        let entries = collect_audio_preload_entries(&index).expect("collect preload entries");
        assert_eq!(entries.len(), AUDIO_PRELOAD_WINDOW_SIZE);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.job_id.as_str())
                .collect::<Vec<_>>(),
            vec!["job-0", "job-1", "job-2"]
        );
    }

    #[test]
    fn collect_audio_preload_entries_ignores_non_audio_batches() {
        let mut index = IndexFile::empty();
        index.task_queue.active_batch = Some(ActiveQueueBatch {
            category: QueueCategory::Llm,
            running: Some(build_summary_entry(
                "job-summary",
                false,
                "2026-01-01T00:00:00Z".to_string(),
            )),
            entries: vec![build_summary_entry(
                "job-summary-2",
                false,
                "2026-01-01T00:00:01Z".to_string(),
            )],
        });

        let entries = collect_audio_preload_entries(&index).expect("collect preload entries");
        assert!(entries.is_empty());
    }

    #[test]
    fn preload_storage_keys_for_stt_resolves_selected_artifacts() {
        let repo_root = temp_workspace();
        let store = IndexStore::new(&repo_root);
        let mut job = test_job(
            "job-stt",
            "2026-01-01T00:00:00Z",
            "sources/job-stt/source.wav",
            "hash-job-stt",
            "job-stt.wav",
        );
        mark_job_completed_with_audio(
            &store,
            &mut job,
            "2026-01-01T00:00:01Z",
            &["channel_01.wav", "mono_mix.wav"],
        )
        .expect("mark completed with audio");
        store.insert_job(job).expect("insert job");

        let entry = build_stt_entry_with_options(
            "job-stt",
            &[PathBuf::from("channel_01.wav")],
            "ko",
            &[],
            "2026-01-01T00:00:02Z".to_string(),
        );

        let storage_keys =
            preload_storage_keys_for_entry(&store, &entry).expect("resolve preload storage keys");
        assert_eq!(storage_keys, vec!["jobs/job-stt/channel_01.wav".to_string()]);
    }

    fn temp_workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!("recordroute-executor-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).expect("temp workspace");
        path
    }
}
