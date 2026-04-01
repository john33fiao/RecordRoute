use super::now_rfc3339;
use crate::index::{IndexStore, JobRecord, SourceKind, TaskQueueState, TaskStatus, TaskType};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsOverview {
    pub generated_at: String,
    pub upload_job_count: usize,
    pub all_job_count: usize,
    pub stages: Vec<StageOverview>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageOverview {
    pub stage: TaskType,
    pub label: String,
    pub completed_count: usize,
    pub in_progress_count: usize,
    pub unprocessed_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StageBucket {
    Completed,
    InProgress,
    Unprocessed,
    Excluded,
}

pub fn collect_stats_overview(repo_root: &Path) -> Result<StatsOverview, String> {
    let store = IndexStore::new(repo_root);
    let jobs = store.list_jobs()?;
    let active_queue_jobs_by_stage =
        collect_active_queue_jobs_by_stage(store.task_queue()?.normalized());
    let upload_jobs = jobs
        .iter()
        .filter(|job| job.source_kind == SourceKind::Upload)
        .cloned()
        .collect::<Vec<_>>();

    let mut stages = Vec::new();
    for (stage, label) in stage_definitions() {
        let mut completed_count = 0usize;
        let mut in_progress_count = 0usize;
        let mut unprocessed_count = 0usize;

        for job in &upload_jobs {
            match classify_stage_bucket(&store, job, stage, &active_queue_jobs_by_stage)? {
                StageBucket::Completed => completed_count = completed_count.saturating_add(1),
                StageBucket::InProgress => in_progress_count = in_progress_count.saturating_add(1),
                StageBucket::Unprocessed => unprocessed_count = unprocessed_count.saturating_add(1),
                StageBucket::Excluded => {}
            }
        }

        stages.push(StageOverview {
            stage,
            label: label.to_string(),
            completed_count,
            in_progress_count,
            unprocessed_count,
        });
    }

    Ok(StatsOverview {
        generated_at: now_rfc3339()?,
        upload_job_count: upload_jobs.len(),
        all_job_count: jobs.len(),
        stages,
    })
}

fn stage_definitions() -> [(TaskType, &'static str); 4] {
    [
        (TaskType::Ffmpeg, "오디오 분리"),
        (TaskType::Stt, "전사"),
        (TaskType::Summary, "요약"),
        (TaskType::Embedding, "임베딩"),
    ]
}

fn classify_stage_bucket(
    store: &IndexStore,
    job: &JobRecord,
    stage: TaskType,
    active_queue_jobs_by_stage: &HashMap<TaskType, HashSet<String>>,
) -> Result<StageBucket, String> {
    if stage_completed(store, job, stage)? {
        return Ok(StageBucket::Completed);
    }

    Ok(match job.task(stage).map(|task| task.status) {
        Some(TaskStatus::Queued | TaskStatus::Running)
            if stage_has_active_queue_entry(active_queue_jobs_by_stage, &job.job_id, stage) =>
        {
            StageBucket::InProgress
        }
        Some(TaskStatus::Failed) => StageBucket::Excluded,
        Some(TaskStatus::Queued | TaskStatus::Running | TaskStatus::Completed) | None => {
            StageBucket::Unprocessed
        }
    })
}

fn collect_active_queue_jobs_by_stage(
    queue_state: TaskQueueState,
) -> HashMap<TaskType, HashSet<String>> {
    let mut active_queue_jobs_by_stage = HashMap::new();

    if let Some(active_batch) = queue_state.active_batch {
        if let Some(running) = active_batch.running {
            record_active_queue_job(
                &mut active_queue_jobs_by_stage,
                running.task_type,
                &running.job_id,
            );
        }

        for entry in active_batch.entries {
            record_active_queue_job(
                &mut active_queue_jobs_by_stage,
                entry.task_type,
                &entry.job_id,
            );
        }
    }

    for batch in queue_state.pending_batches {
        for entry in batch.entries {
            record_active_queue_job(
                &mut active_queue_jobs_by_stage,
                entry.task_type,
                &entry.job_id,
            );
        }
    }

    active_queue_jobs_by_stage
}

fn record_active_queue_job(
    active_queue_jobs_by_stage: &mut HashMap<TaskType, HashSet<String>>,
    stage: TaskType,
    job_id: &str,
) {
    active_queue_jobs_by_stage
        .entry(stage)
        .or_default()
        .insert(job_id.to_string());
}

fn stage_has_active_queue_entry(
    active_queue_jobs_by_stage: &HashMap<TaskType, HashSet<String>>,
    job_id: &str,
    stage: TaskType,
) -> bool {
    active_queue_jobs_by_stage
        .get(&stage)
        .is_some_and(|job_ids| job_ids.contains(job_id))
}

fn stage_completed(store: &IndexStore, job: &JobRecord, stage: TaskType) -> Result<bool, String> {
    match stage {
        TaskType::Ffmpeg => {
            Ok(job.outputs.merged_mono_wav.is_some() || !job.outputs.split_mono_wavs.is_empty())
        }
        TaskType::Stt => Ok(store.count_transcripts(&job.job_id)? > 0),
        TaskType::Summary => Ok(store.get_summary(&job.job_id)?.is_some()),
        TaskType::Embedding => Ok(store.get_summary_embedding(&job.job_id)?.is_some()),
    }
}
