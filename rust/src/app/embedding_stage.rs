use super::{StageJobDisposition, StageJobSubmission, artifacts, now_rfc3339, stages};
use crate::index::{IndexStore, JobRecord, SummaryEmbeddingRecord, TaskStatus, TaskType};
use crate::llama::{Toolchain as LlamaToolchain, embedding_model_id, run_summary_embedding};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

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
) -> Result<StageJobSubmission, String> {
    let index_store = IndexStore::new(repo_root);
    let mut job = index_store
        .find_job(job_id)?
        .ok_or_else(|| format!("job not found: {job_id}"))?;
    let (summary_file, summary_dir) = summary_path_for_job(&job)?;
    if !summary_file.is_file() {
        return Err(format!("summary not found: {}", summary_file.display()));
    }
    if let Some(task) = job.task(TaskType::Embedding)
        && task.status == TaskStatus::Running
    {
        return Ok(StageJobSubmission {
            job,
            disposition: StageJobDisposition::Deduplicated,
            planned_audio_files: Vec::new(),
        });
    }

    if !is_embedding_stale(repo_root, &job)? {
        return Ok(StageJobSubmission {
            job,
            disposition: StageJobDisposition::Reused,
            planned_audio_files: Vec::new(),
        });
    }

    if !summary_dir.is_dir() {
        fs::create_dir_all(&summary_dir).map_err(|e| {
            format!(
                "failed to create summary directory {}: {e}",
                summary_dir.display()
            )
        })?;
    }

    job.upsert_running_task(TaskType::Embedding, now_rfc3339()?);
    index_store.update_job(job_id, |_| job.clone())?;
    Ok(StageJobSubmission {
        job,
        disposition: StageJobDisposition::Submitted,
        planned_audio_files: Vec::new(),
    })
}

pub fn execute_summary_embedding_job(repo_root: &Path, job_id: &str) -> Result<JobRecord, String> {
    let index_store = IndexStore::new(repo_root);
    let job = index_store
        .find_job(job_id)?
        .ok_or_else(|| format!("job not found: {job_id}"))?;

    let result = (|| -> Result<SummaryEmbeddingRecord, String> {
        let (summary_file, summary_dir) = summary_path_for_job(&job)?;
        let summary_text = fs::read_to_string(&summary_file)
            .map_err(|e| format!("failed to read summary {}: {e}", summary_file.display()))?;
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
            vector,
        };
        let sidecar_path = artifacts::summary_embedding_output_path(&summary_dir);
        fs::write(
            &sidecar_path,
            serde_json::to_vec_pretty(&sidecar).map_err(|e| e.to_string())?,
        )
        .map_err(|e| {
            format!(
                "failed to write embedding sidecar {}: {e}",
                sidecar_path.display()
            )
        })?;
        Ok(SummaryEmbeddingRecord {
            model_id,
            text_sha256,
            dimension: sidecar.dimension,
            normalized: sidecar.normalized,
            created_at,
            file_path: sidecar_path.to_string_lossy().into_owned(),
        })
    })();

    match result {
        Ok(metadata) => {
            let mut updated =
                stages::finalize_task_success(repo_root, job, TaskType::Embedding, now_rfc3339()?)?;
            updated.summary_embedding = Some(metadata);
            IndexStore::new(repo_root).update_job(job_id, |_| updated.clone())?;
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
            let _ = execute_summary_embedding_job(repo_root, &job.job_id);
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
) -> Result<Vec<SummarySearchResult>, String> {
    let toolchain = LlamaToolchain::discover(repo_root)?;
    let q = run_summary_embedding(&toolchain, query)?;
    if q.is_empty() {
        return Ok(Vec::new());
    }

    let mut rows = Vec::new();
    for job in IndexStore::new(repo_root).list_completed_jobs()? {
        let Some(metadata) = job.summary_embedding.clone() else {
            continue;
        };
        let Ok(sidecar) = read_sidecar(&PathBuf::from(&metadata.file_path)) else {
            continue;
        };
        if sidecar.model_id != embedding_model_id(repo_root) || sidecar.dimension != q.len() {
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
            summary_excerpt: summary_excerpt(repo_root, &job)?,
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
    let (summary_file, _) = summary_path_for_job(job)?;
    if !summary_file.is_file() {
        return Ok(true);
    }
    let summary_text = fs::read_to_string(summary_file).map_err(|e| e.to_string())?;
    let expected_sha = summary_text_sha256(&summary_text);
    if expected_sha != metadata.text_sha256 {
        return Ok(true);
    }
    if metadata.model_id != embedding_model_id(repo_root) {
        return Ok(true);
    }
    let sidecar = match read_sidecar(&PathBuf::from(&metadata.file_path)) {
        Ok(s) => s,
        Err(_) => return Ok(true),
    };
    Ok(sidecar.text_sha256 != expected_sha
        || sidecar.model_id != metadata.model_id
        || sidecar.dimension != metadata.dimension)
}

fn read_sidecar(path: &Path) -> Result<EmbeddingSidecar, String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("failed to read sidecar {}: {e}", path.display()))?;
    serde_json::from_str::<EmbeddingSidecar>(&raw)
        .map_err(|e| format!("failed to parse sidecar {}: {e}", path.display()))
}

fn summary_path_for_job(job: &JobRecord) -> Result<(PathBuf, PathBuf), String> {
    let summary_dir = PathBuf::from(&job.job_dir).join("summary");
    let summary_file = artifacts::ensure_summary_output_path(&summary_dir, &job.source_file_name)?;
    Ok((summary_file, summary_dir))
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
    let (summary_file, _) = summary_path_for_job(job)?;
    let _ = repo_root;
    let raw = fs::read_to_string(&summary_file).map_err(|e| e.to_string())?;
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
