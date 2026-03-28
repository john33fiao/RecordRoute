use super::{
    FfmpegJobDisposition, FfmpegJobSubmission, RunSummary, WAIT_FOR_RUNNING_JOB_POLL_INTERVAL,
    build_run_id, now_rfc3339, queue, stages, submit_stt_job,
};
use crate::audio_store::{AudioStore, ImportedSource};
use crate::error::{AppError, AppResult, dependency_unavailable_or_internal};
use crate::ffmpeg::{
    ConversionOutputs, SplitMonoOutput, Toolchain as FfmpegToolchain, probe_audio_input,
    run_conversion,
};
use crate::index::{
    AudioArtifactRecord, IndexStore, JobOutputs, JobProbe, JobRecord, JobSplitOutput, JobStatus,
    SourceKind,
};
use std::fs;
use std::path::Path;
use std::thread;

pub fn run_with_repo_root(repo_root: &Path, input: &Path) -> Result<RunSummary, String> {
    let submission = submit_ffmpeg_job(repo_root, input).map_err(|error| error.to_string())?;

    match submission.disposition {
        FfmpegJobDisposition::Reused => run_summary_from_completed_job(repo_root, submission.job),
        FfmpegJobDisposition::Submitted => {
            queue::dispatch_until_task_terminal(
                repo_root,
                &submission.job.job_id,
                crate::index::TaskType::Ffmpeg,
            )?;
            let job = IndexStore::new(repo_root)
                .find_job(&submission.job.job_id)?
                .ok_or_else(|| format!("job not found in index: {}", submission.job.job_id))?;
            run_summary_from_completed_job(repo_root, job)
        }
        FfmpegJobDisposition::Deduplicated => {
            let job = wait_for_ffmpeg_job_completion(repo_root, &submission.job.job_id)?;
            run_summary_from_completed_job(repo_root, job)
        }
    }
}

pub fn submit_ffmpeg_job(repo_root: &Path, input: &Path) -> AppResult<FfmpegJobSubmission> {
    let audio_store = AudioStore::new(repo_root).map_err(AppError::internal)?;
    if !input.is_file() {
        return Err(AppError::bad_request(format!(
            "input file not found: {}",
            input.display()
        )));
    }
    let imported = audio_store
        .publish_source(input, None, SourceKind::LocalFile)
        .map_err(AppError::internal)?;
    submit_ffmpeg_job_from_imported_source(repo_root, imported)
}

pub fn submit_ffmpeg_job_from_imported_source(
    repo_root: &Path,
    imported: ImportedSource,
) -> AppResult<FfmpegJobSubmission> {
    let index_store = IndexStore::new(repo_root);
    let source_path = index_store.source_path(&imported.source_ref);

    if let Some(job) = index_store
        .find_reusable_completed_job_by_source_hash(&imported.source_content_sha256)
        .map_err(AppError::internal)?
    {
        return Ok(FfmpegJobSubmission {
            job,
            input_path: source_path,
            disposition: FfmpegJobDisposition::Reused,
            queue: None,
        });
    }

    if let Some(job) = index_store
        .find_inflight_job_by_source_hash(&imported.source_content_sha256)
        .map_err(AppError::internal)?
    {
        let queue = if job.status == JobStatus::Queued {
            index_store
                .with_index_read(|index| {
                    Ok(queue::find_ticket(
                        index,
                        &job.job_id,
                        crate::index::TaskType::Ffmpeg,
                    ))
                })
                .map_err(AppError::internal)?
        } else {
            None
        };
        return Ok(FfmpegJobSubmission {
            job,
            input_path: source_path,
            disposition: FfmpegJobDisposition::Deduplicated,
            queue,
        });
    }

    FfmpegToolchain::discover(repo_root).map_err(dependency_unavailable_or_internal)?;
    index_store.ensure_db_dir().map_err(AppError::internal)?;

    let started_at = now_rfc3339().map_err(AppError::internal)?;
    let job_id = build_run_id().map_err(AppError::internal)?;
    let job_dir = index_store.job_dir(&job_id);
    fs::create_dir_all(&job_dir).map_err(|error| {
        AppError::internal(format!(
            "failed to create job directory {}: {error}",
            job_dir.display()
        ))
    })?;

    let job = JobRecord::new_with_source(
        job_id,
        started_at.clone(),
        imported.source_ref.clone(),
        imported.source_kind,
        imported.source_content_sha256,
        imported.source_file_name,
    );
    let entry = queue::build_ffmpeg_entry(&job.job_id, Path::new(&job.source_ref), started_at);
    let (job, ticket) = index_store
        .with_index_mut(|index| {
            index.jobs.push(job.clone());
            let ticket = queue::enqueue_entry(index, entry);
            Ok((job.clone(), ticket))
        })
        .map_err(AppError::internal)?;

    Ok(FfmpegJobSubmission {
        job,
        input_path: source_path,
        disposition: FfmpegJobDisposition::Submitted,
        queue: Some(ticket),
    })
}

pub fn execute_ffmpeg_job(
    repo_root: &Path,
    job_id: &str,
    input_path: &Path,
) -> Result<JobRecord, String> {
    let index_store = IndexStore::new(repo_root);
    let toolchain = FfmpegToolchain::discover(repo_root)?;
    let mut job = index_store
        .find_job(job_id)?
        .ok_or_else(|| format!("job not found in index: {job_id}"))?;
    let source_ref = input_path.to_string_lossy().into_owned();
    if source_ref != job.source_ref {
        let error = format!("job input path mismatch for {job_id}");
        job.mark_failed(now_rfc3339()?, error.clone())?;
        index_store.update_job(job_id, |_| job.clone())?;
        return Err(error);
    }
    let job_dir = index_store.job_dir(job_id);
    let input_path = index_store
        .audio_store()?
        .materialize_to_cache(&job.source_ref)?;

    let probe = match probe_audio_input(&toolchain, &input_path) {
        Ok(probe) => probe,
        Err(error) => {
            job.mark_failed(now_rfc3339()?, error.clone())?;
            index_store.update_job(job_id, |_| job.clone())?;
            return Err(error);
        }
    };

    job.probe = JobProbe {
        channels: Some(probe.channels),
        channel_layout: probe.channel_layout.clone(),
    };
    index_store.update_job(job_id, |_| job.clone())?;

    let planned_outputs = ConversionOutputs::new(&job_dir, probe.channels);

    match run_conversion(&toolchain, &input_path, probe.channels, &planned_outputs) {
        Ok(()) => {
            record_audio_outputs(&index_store, job_id, &planned_outputs)?;
            job.mark_completed(now_rfc3339()?, build_job_outputs(&planned_outputs))?;
            index_store.update_job(job_id, |_| job.clone())?;
            if let Err(error) = submit_stt_job(repo_root, job_id, None) {
                let _ = stages::record_followup_submission_failure(
                    repo_root,
                    job_id,
                    crate::index::TaskType::Stt,
                    error.to_string(),
                );
            }
            Ok(job)
        }
        Err(error) => {
            planned_outputs.cleanup_partial_files();
            job.mark_failed(now_rfc3339()?, error.clone())?;
            index_store.update_job(job_id, |_| job.clone())?;
            Err(error)
        }
    }
}

fn wait_for_ffmpeg_job_completion(repo_root: &Path, job_id: &str) -> Result<JobRecord, String> {
    let index_store = IndexStore::new(repo_root);

    loop {
        let job = index_store
            .find_job(job_id)?
            .ok_or_else(|| format!("job not found in index: {job_id}"))?;

        match job.status {
            JobStatus::Queued | JobStatus::Running => {
                thread::sleep(WAIT_FOR_RUNNING_JOB_POLL_INTERVAL)
            }
            JobStatus::Completed => return Ok(job),
            JobStatus::Failed => {
                return Err(job
                    .error_message
                    .unwrap_or_else(|| format!("ffmpeg job failed: {job_id}")));
            }
        }
    }
}

fn build_job_outputs(outputs: &ConversionOutputs) -> JobOutputs {
    JobOutputs {
        merged_mono_wav: outputs
            .merged_mono_wav
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string),
        split_mono_wavs: outputs
            .split_mono_wavs
            .iter()
            .map(|output| JobSplitOutput {
                channel_index: output.channel_index,
                path: output
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_string(),
            })
            .collect(),
    }
}

fn record_audio_outputs(
    index_store: &IndexStore,
    job_id: &str,
    outputs: &ConversionOutputs,
) -> Result<(), String> {
    let merged_name = outputs
        .merged_mono_wav
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "merged output does not have a valid file name: {}",
                outputs.merged_mono_wav.display()
            )
        })?;
    index_store.upsert_audio_artifact(&AudioArtifactRecord {
        job_id: job_id.to_string(),
        logical_name: merged_name.to_string(),
        storage_key: format!("jobs/{job_id}/{merged_name}"),
    })?;
    for output in &outputs.split_mono_wavs {
        let logical_name = output
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                format!(
                    "split output does not have a valid file name: {}",
                    output.path.display()
                )
            })?;
        index_store.upsert_audio_artifact(&AudioArtifactRecord {
            job_id: job_id.to_string(),
            logical_name: logical_name.to_string(),
            storage_key: format!("jobs/{job_id}/{logical_name}"),
        })?;
    }
    Ok(())
}

fn run_summary_from_completed_job(repo_root: &Path, job: JobRecord) -> Result<RunSummary, String> {
    let merged_mono_wav = job.outputs.merged_mono_wav.clone().ok_or_else(|| {
        format!(
            "reusable completed job is missing merged output path: {}",
            job.job_id
        )
    })?;
    if job.outputs.split_mono_wavs.is_empty() {
        return Err(format!(
            "reusable completed job is missing split output paths: {}",
            job.job_id
        ));
    }

    let job_dir = IndexStore::new(repo_root).job_dir(&job.job_id);

    Ok(RunSummary {
        job_id: job.job_id,
        job_dir: job_dir.clone(),
        outputs: ConversionOutputs {
            merged_mono_wav: job_dir.join(merged_mono_wav),
            split_mono_wavs: job
                .outputs
                .split_mono_wavs
                .into_iter()
                .map(|output| SplitMonoOutput {
                    channel_index: output.channel_index,
                    path: job_dir.join(output.path),
                })
                .collect(),
        },
    })
}
