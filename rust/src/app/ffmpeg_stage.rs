use super::{
    FfmpegJobDisposition, FfmpegJobSubmission, RunSummary, WAIT_FOR_RUNNING_JOB_POLL_INTERVAL,
    build_run_id, now_rfc3339, path_to_string,
};
use crate::ffmpeg::{
    ConversionOutputs, SplitMonoOutput, Toolchain as FfmpegToolchain, probe_audio_input,
    run_conversion,
};
use crate::index::{IndexStore, JobOutputs, JobProbe, JobRecord, JobSplitOutput, JobStatus};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

pub fn run_with_repo_root(repo_root: &Path, input: &Path) -> Result<RunSummary, String> {
    let submission = submit_ffmpeg_job(repo_root, input)?;

    match submission.disposition {
        FfmpegJobDisposition::Reused => run_summary_from_completed_job(submission.job),
        FfmpegJobDisposition::Submitted => {
            let job =
                execute_ffmpeg_job(repo_root, &submission.job.job_id, &submission.input_path)?;
            run_summary_from_completed_job(job)
        }
        FfmpegJobDisposition::Deduplicated => {
            let job = wait_for_ffmpeg_job_completion(repo_root, &submission.job.job_id)?;
            run_summary_from_completed_job(job)
        }
    }
}

pub fn submit_ffmpeg_job(repo_root: &Path, input: &Path) -> Result<FfmpegJobSubmission, String> {
    let input_path = normalize_input_path(input)?;
    let index_store = IndexStore::new(repo_root);

    if let Some(job) = index_store.find_reusable_completed_job(&input_path)? {
        return Ok(FfmpegJobSubmission {
            job,
            input_path,
            disposition: FfmpegJobDisposition::Reused,
        });
    }

    if let Some(job) = index_store.find_running_job_by_source(&input_path)? {
        return Ok(FfmpegJobSubmission {
            job,
            input_path,
            disposition: FfmpegJobDisposition::Deduplicated,
        });
    }

    FfmpegToolchain::discover(repo_root)?;
    index_store.ensure_db_dir()?;

    let started_at = now_rfc3339()?;
    let job_id = build_run_id()?;
    let job_dir = index_store.job_dir(&job_id);
    fs::create_dir_all(&job_dir).map_err(|error| {
        format!(
            "failed to create job directory {}: {error}",
            job_dir.display()
        )
    })?;

    let job = JobRecord::new(job_id, started_at, input_path.clone(), job_dir);
    index_store.insert_job(job.clone())?;

    Ok(FfmpegJobSubmission {
        job,
        input_path,
        disposition: FfmpegJobDisposition::Submitted,
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
    let input_path = match normalize_input_path(input_path) {
        Ok(path) => path,
        Err(error) => {
            job.mark_failed(now_rfc3339()?, error.clone())?;
            index_store.update_job(job_id, |_| job.clone())?;
            return Err(error);
        }
    };
    if input_path != PathBuf::from(&job.source_path) {
        let error = format!("job input path mismatch for {job_id}");
        job.mark_failed(now_rfc3339()?, error.clone())?;
        index_store.update_job(job_id, |_| job.clone())?;
        return Err(error);
    }
    let job_dir = PathBuf::from(&job.job_dir);

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
            job.mark_completed(now_rfc3339()?, build_job_outputs(&planned_outputs))?;
            index_store.update_job(job_id, |_| job.clone())?;
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
            JobStatus::Running => thread::sleep(WAIT_FOR_RUNNING_JOB_POLL_INTERVAL),
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
        merged_mono_wav: Some(path_to_string(&outputs.merged_mono_wav)),
        split_mono_wavs: outputs
            .split_mono_wavs
            .iter()
            .map(|output| JobSplitOutput {
                channel_index: output.channel_index,
                path: path_to_string(&output.path),
            })
            .collect(),
    }
}

fn run_summary_from_completed_job(job: JobRecord) -> Result<RunSummary, String> {
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

    Ok(RunSummary {
        job_id: job.job_id,
        job_dir: PathBuf::from(job.job_dir),
        outputs: ConversionOutputs {
            merged_mono_wav: PathBuf::from(merged_mono_wav),
            split_mono_wavs: job
                .outputs
                .split_mono_wavs
                .into_iter()
                .map(|output| SplitMonoOutput {
                    channel_index: output.channel_index,
                    path: PathBuf::from(output.path),
                })
                .collect(),
        },
    })
}

fn normalize_input_path(input: &Path) -> Result<PathBuf, String> {
    if !input.exists() {
        return Err(format!("input file not found: {}", input.display()));
    }

    if !input.is_file() {
        return Err(format!("input path is not a file: {}", input.display()));
    }

    fs::canonicalize(input)
        .map_err(|error| format!("failed to resolve input path {}: {error}", input.display()))
}
