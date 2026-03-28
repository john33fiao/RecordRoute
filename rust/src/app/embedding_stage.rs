use super::{StageJobDisposition, StageJobSubmission, artifacts, now_rfc3339, queue, stages};
use crate::error::{AppError, AppResult, dependency_unavailable_or_internal};
use crate::index::{
    IndexStore, JobRecord, SummaryEmbeddingRecord, SummaryEmbeddingVectorRecord, TaskStatus,
    TaskType,
};
use crate::llama::{Toolchain as LlamaToolchain, embedding_model_id, run_summary_embedding};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmbeddingSidecar {
    pub model_id: String,
    pub text_sha256: String,
    pub dimension: usize,
    pub normalized: bool,
    pub created_at: String,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SummarySearchResult {
    pub job_id: String,
    pub score: f32,
    pub source_file_name: String,
    pub summary_file_name: String,
    pub summary_excerpt: String,
}

pub fn submit_summary_embedding_job(
    repo_root: &Path,
    job_id: &str,
) -> AppResult<StageJobSubmission> {
    let index_store = IndexStore::new(repo_root);
    let job = index_store
        .find_job(job_id)
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found(format!("job not found: {job_id}")))?;
    if index_store
        .get_summary(job_id)
        .map_err(AppError::internal)?
        .is_none()
    {
        return Err(AppError::bad_request(format!(
            "summary not found for job: {job_id}"
        )));
    }
    if let Some(task) = job.task(TaskType::Embedding)
        && matches!(task.status, TaskStatus::Queued | TaskStatus::Running)
    {
        let queue = if task.status == TaskStatus::Queued {
            stages::queued_task_ticket(&index_store, &job.job_id, TaskType::Embedding)
                .map_err(AppError::internal)?
        } else {
            None
        };
        return Ok(StageJobSubmission {
            job,
            disposition: StageJobDisposition::Deduplicated,
            planned_audio_files: Vec::new(),
            queue,
        });
    }

    if !is_embedding_stale(repo_root, &job).map_err(AppError::internal)? {
        return Ok(StageJobSubmission {
            job,
            disposition: StageJobDisposition::Reused,
            planned_audio_files: Vec::new(),
            queue: None,
        });
    }

    let queued_at = now_rfc3339().map_err(AppError::internal)?;
    let entry = queue::build_embedding_entry(job_id, queued_at.clone());
    let (job, ticket) =
        stages::enqueue_existing_task(&index_store, job_id, TaskType::Embedding, queued_at, entry)
            .map_err(AppError::internal)?;
    Ok(StageJobSubmission {
        job,
        disposition: StageJobDisposition::Submitted,
        planned_audio_files: Vec::new(),
        queue: Some(ticket),
    })
}

pub fn execute_summary_embedding_job(repo_root: &Path, job_id: &str) -> Result<JobRecord, String> {
    let index_store = IndexStore::new(repo_root);
    let job = index_store
        .find_job(job_id)?
        .ok_or_else(|| format!("job not found: {job_id}"))?;

    let result = (|| -> Result<SummaryEmbeddingVectorRecord, String> {
        let summary_text = index_store
            .get_summary(job_id)?
            .ok_or_else(|| format!("summary not found for job: {job_id}"))?
            .text;
        let text_sha256 = summary_text_sha256(&summary_text);
        let toolchain = LlamaToolchain::discover(repo_root)?;
        let vector = run_summary_embedding(&toolchain, &summary_text)?;
        if vector.is_empty() {
            return Err("embedding vector is empty".to_string());
        }
        let created_at = now_rfc3339()?;
        let model_id = embedding_model_id(repo_root);
        let sidecar = EmbeddingSidecar {
            model_id: model_id.clone(),
            text_sha256: text_sha256.clone(),
            dimension: vector.len(),
            normalized: true,
            created_at: created_at.clone(),
            vector: vector.clone(),
        };
        Ok(SummaryEmbeddingVectorRecord {
            metadata: SummaryEmbeddingRecord {
                model_id,
                text_sha256,
                dimension: sidecar.dimension,
                normalized: sidecar.normalized,
                created_at,
            },
            vector,
        })
    })();

    match result {
        Ok(record) => {
            let updated = IndexStore::new(repo_root).commit_summary_embedding_success(
                job_id,
                now_rfc3339()?,
                &record,
            )?;
            Ok(updated)
        }
        Err(error) => {
            stages::finalize_task_failure(
                repo_root,
                job,
                TaskType::Embedding,
                now_rfc3339()?,
                error.clone(),
            )?;
            Err(error)
        }
    }
}

pub fn backfill_summary_embeddings(repo_root: &Path) -> Result<Vec<(String, bool)>, String> {
    let jobs = IndexStore::new(repo_root).list_completed_jobs()?;
    let mut output = Vec::new();
    for job in jobs {
        let submission = match submit_summary_embedding_job(repo_root, &job.job_id) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if submission.should_execute() {
            let _ =
                queue::dispatch_until_task_terminal(repo_root, &job.job_id, TaskType::Embedding);
            output.push((job.job_id, true));
        } else {
            output.push((job.job_id, false));
        }
    }
    Ok(output)
}

pub fn search_summaries(
    repo_root: &Path,
    query: &str,
    limit: usize,
    min_score: Option<f32>,
) -> AppResult<Vec<SummarySearchResult>> {
    let toolchain =
        LlamaToolchain::discover(repo_root).map_err(dependency_unavailable_or_internal)?;
    let q = run_summary_embedding(&toolchain, query).map_err(dependency_unavailable_or_internal)?;
    if q.is_empty() {
        return Ok(Vec::new());
    }

    let mut rows = Vec::new();
    for job in IndexStore::new(repo_root)
        .list_completed_jobs()
        .map_err(AppError::internal)?
    {
        let Some(_metadata) = job.summary_embedding.clone() else {
            continue;
        };
        let Ok(Some(sidecar)) = IndexStore::new(repo_root).get_summary_embedding(&job.job_id)
        else {
            continue;
        };
        if sidecar.metadata.model_id != embedding_model_id(repo_root)
            || sidecar.metadata.dimension != q.len()
        {
            continue;
        }
        let score = dot(&q, &sidecar.vector);
        if let Some(min) = min_score
            && score < min
        {
            continue;
        }
        rows.push(SummarySearchResult {
            job_id: job.job_id.clone(),
            score,
            source_file_name: job.source_file_name.clone(),
            summary_file_name: artifacts::summary_file_name().to_string(),
            summary_excerpt: summary_excerpt(repo_root, &job).map_err(AppError::internal)?,
        });
    }

    rows.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.truncate(limit);
    Ok(rows)
}

pub fn is_embedding_stale(repo_root: &Path, job: &JobRecord) -> Result<bool, String> {
    let Some(metadata) = job.summary_embedding.as_ref() else {
        return Ok(true);
    };
    let store = IndexStore::new(repo_root);
    let Some(summary) = store.get_summary(&job.job_id)? else {
        return Ok(true);
    };
    let summary_text = summary.text;
    let expected_sha = summary_text_sha256(&summary_text);
    if expected_sha != metadata.text_sha256 {
        return Ok(true);
    }
    if metadata.model_id != embedding_model_id(repo_root) {
        return Ok(true);
    }
    let sidecar = match store.get_summary_embedding(&job.job_id)? {
        Some(s) => s,
        None => return Ok(true),
    };
    Ok(sidecar.metadata.text_sha256 != expected_sha
        || sidecar.metadata.model_id != metadata.model_id
        || sidecar.metadata.dimension != metadata.dimension)
}

fn summary_text_sha256(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>()
}

fn summary_excerpt(repo_root: &Path, job: &JobRecord) -> Result<String, String> {
    let raw = IndexStore::new(repo_root)
        .get_summary(&job.job_id)?
        .ok_or_else(|| format!("summary not found for job: {}", job.job_id))?
        .text;
    let one_line = raw
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if one_line.chars().count() <= 200 {
        Ok(one_line)
    } else {
        Ok(format!(
            "{}…",
            one_line.chars().take(200).collect::<String>()
        ))
    }
}
