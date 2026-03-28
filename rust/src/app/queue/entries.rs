use super::QueueTicket;
use crate::index::{
    IndexFile, QueueBatch, QueueCategory, QueueEntry, QueuePayload, TaskQueueState, TaskType,
};
use crate::whisper::{normalize_keywords, normalize_language, transcription_language_from_env};
use std::path::{Path, PathBuf};

pub fn build_ffmpeg_entry(job_id: &str, input_path: &Path, queued_at: String) -> QueueEntry {
    QueueEntry {
        job_id: job_id.to_string(),
        task_type: TaskType::Ffmpeg,
        category: QueueCategory::Ffmpeg,
        queued_at: queued_at.clone(),
        payload: QueuePayload::Ffmpeg {
            input_path: input_path.to_string_lossy().into_owned(),
        },
    }
}

pub fn build_stt_entry(job_id: &str, audio_files: &[PathBuf], queued_at: String) -> QueueEntry {
    let language = transcription_language_from_env();
    build_stt_entry_with_options(job_id, audio_files, &language, &[], queued_at)
}

pub fn build_stt_entry_with_options(
    job_id: &str,
    audio_files: &[PathBuf],
    language: &str,
    keywords: &[String],
    queued_at: String,
) -> QueueEntry {
    QueueEntry {
        job_id: job_id.to_string(),
        task_type: TaskType::Stt,
        category: QueueCategory::Stt,
        queued_at: queued_at.clone(),
        payload: QueuePayload::Stt {
            audio_files: audio_files
                .iter()
                .map(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default()
                        .to_string()
                })
                .collect(),
            language: normalize_language(language).unwrap_or_else(transcription_language_from_env),
            keywords: normalize_keywords(keywords),
        },
    }
}

pub fn build_summary_entry(job_id: &str, force_regenerate: bool, queued_at: String) -> QueueEntry {
    QueueEntry {
        job_id: job_id.to_string(),
        task_type: TaskType::Summary,
        category: QueueCategory::Llm,
        queued_at: queued_at.clone(),
        payload: QueuePayload::Summary { force_regenerate },
    }
}

pub fn build_embedding_entry(job_id: &str, queued_at: String) -> QueueEntry {
    QueueEntry {
        job_id: job_id.to_string(),
        task_type: TaskType::Embedding,
        category: QueueCategory::Embed,
        queued_at: queued_at.clone(),
        payload: QueuePayload::Embedding,
    }
}

pub fn enqueue_entry(index: &mut IndexFile, entry: QueueEntry) -> QueueTicket {
    let state = &mut index.task_queue;
    if state.burst_limit == 0 {
        state.burst_limit = TaskQueueState::default().burst_limit;
    }

    if let Some(active_batch) = state.active_batch.as_mut()
        && active_batch.category == entry.category
    {
        active_batch.entries.push(entry.clone());
        return QueueTicket {
            category: entry.category,
            position: active_batch.entries.len(),
            queued_at: entry.queued_at,
        };
    }

    let active_len = state
        .active_batch
        .as_ref()
        .map(|batch| batch.entries.len())
        .unwrap_or(0);
    let mut prefix_len = active_len;
    for batch in &mut state.pending_batches {
        if batch.category == entry.category {
            batch.entries.push(entry.clone());
            return QueueTicket {
                category: entry.category,
                position: prefix_len + batch.entries.len(),
                queued_at: entry.queued_at,
            };
        }
        prefix_len = prefix_len.saturating_add(batch.entries.len());
    }

    state.pending_batches.push(QueueBatch {
        category: entry.category,
        entries: vec![entry.clone()],
    });
    QueueTicket {
        category: entry.category,
        position: prefix_len + 1,
        queued_at: entry.queued_at,
    }
}

pub fn find_ticket(index: &IndexFile, job_id: &str, task_type: TaskType) -> Option<QueueTicket> {
    let mut position = 1usize;

    if let Some(active_batch) = index.task_queue.active_batch.as_ref() {
        for entry in &active_batch.entries {
            if entry.job_id == job_id && entry.task_type == task_type {
                return Some(QueueTicket {
                    category: entry.category,
                    position,
                    queued_at: entry.queued_at.clone(),
                });
            }
            position = position.saturating_add(1);
        }
    }

    for batch in &index.task_queue.pending_batches {
        for entry in &batch.entries {
            if entry.job_id == job_id && entry.task_type == task_type {
                return Some(QueueTicket {
                    category: entry.category,
                    position,
                    queued_at: entry.queued_at.clone(),
                });
            }
            position = position.saturating_add(1);
        }
    }

    None
}

pub fn find_task_entry(index: &IndexFile, job_id: &str, task_type: TaskType) -> Option<QueueEntry> {
    if let Some(active_batch) = index.task_queue.active_batch.as_ref() {
        if let Some(running) = active_batch.running.as_ref()
            && running.job_id == job_id
            && running.task_type == task_type
        {
            return Some(running.clone());
        }

        for entry in &active_batch.entries {
            if entry.job_id == job_id && entry.task_type == task_type {
                return Some(entry.clone());
            }
        }
    }

    for batch in &index.task_queue.pending_batches {
        for entry in &batch.entries {
            if entry.job_id == job_id && entry.task_type == task_type {
                return Some(entry.clone());
            }
        }
    }

    None
}

pub fn update_queued_entry(
    index: &mut IndexFile,
    job_id: &str,
    task_type: TaskType,
    mut update: impl FnMut(&mut QueueEntry),
) -> bool {
    if let Some(active_batch) = index.task_queue.active_batch.as_mut() {
        for entry in &mut active_batch.entries {
            if entry.job_id == job_id && entry.task_type == task_type {
                update(entry);
                return true;
            }
        }
    }

    for batch in &mut index.task_queue.pending_batches {
        for entry in &mut batch.entries {
            if entry.job_id == job_id && entry.task_type == task_type {
                update(entry);
                return true;
            }
        }
    }

    false
}
