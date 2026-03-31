mod app;
mod audio_store;
mod error;
mod ffmpeg;
mod index;
mod launcher;
mod llama;
mod runtime_root;
mod server;
mod storage;
mod tool_runtime;
mod whisper;

pub use app::main_cli;

pub fn main_server() -> Result<(), String> {
    let runtime_root = runtime_root::resolve_runtime_root()?;
    app::load_repo_env(&runtime_root)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create tokio runtime: {error}"))?;
    runtime.block_on(server::serve_with_runtime_root(runtime_root))
}

pub fn main_launcher() -> Result<(), String> {
    launcher::run_launcher()
}

#[cfg(test)]
pub(crate) mod test_support {
    use crate::index::{
        AudioArtifactRecord, IndexStore, JobOutputs, JobRecord, JobSplitOutput, SourceKind,
        SplitStrategy, SummaryRecord, TranscriptRecord,
    };
    use std::ffi::OsString;
    use std::fs;
    use std::path::Path;
    use std::sync::{Mutex, OnceLock};

    pub(crate) fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    pub(crate) struct EnvVarGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvVarGuard {
        pub(crate) fn capture(key: &'static str) -> Self {
            Self {
                key,
                previous: std::env::var_os(key),
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => unsafe { std::env::set_var(self.key, value) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    pub(crate) fn test_job(
        job_id: &str,
        started_at: &str,
        source_ref: &str,
        source_hash: &str,
        source_file_name: &str,
    ) -> JobRecord {
        JobRecord::new_with_source(
            job_id.to_string(),
            started_at.to_string(),
            source_ref.to_string(),
            SourceKind::LocalFile,
            source_hash.to_string(),
            source_file_name.to_string(),
        )
    }

    pub(crate) fn mark_job_completed_with_audio(
        store: &IndexStore,
        job: &mut JobRecord,
        finished_at: &str,
        audio_file_names: &[&str],
    ) -> Result<(), String> {
        let outputs = seed_audio_artifacts(store, &job.job_id, audio_file_names)?;
        job.split_strategy =
            if outputs.split_mono_wavs.is_empty() && outputs.merged_mono_wav.is_some() {
                SplitStrategy::MergedMonoOnly
            } else {
                SplitStrategy::PerChannelPlusMergedMono
            };
        job.mark_completed(finished_at.to_string(), outputs)
    }

    pub(crate) fn seed_audio_artifacts(
        store: &IndexStore,
        job_id: &str,
        audio_file_names: &[&str],
    ) -> Result<JobOutputs, String> {
        let job_dir = store.job_dir(job_id);
        fs::create_dir_all(&job_dir).map_err(|error| {
            format!(
                "failed to create test job dir {}: {error}",
                job_dir.display()
            )
        })?;

        let mut merged_mono_wav = None;
        let mut split_mono_wavs = Vec::new();

        for file_name in audio_file_names {
            let path = job_dir.join(file_name);
            fs::write(&path, format!("audio:{file_name}")).map_err(|error| {
                format!("failed to seed audio artifact {}: {error}", path.display())
            })?;
            store.upsert_audio_artifact(&AudioArtifactRecord {
                job_id: job_id.to_string(),
                logical_name: (*file_name).to_string(),
                storage_key: format!("jobs/{job_id}/{file_name}"),
            })?;

            if *file_name == "mono_mix.wav" {
                merged_mono_wav = Some((*file_name).to_string());
                continue;
            }

            let channel_index = parse_channel_index(file_name);
            split_mono_wavs.push(JobSplitOutput {
                channel_index,
                path: (*file_name).to_string(),
            });
        }

        split_mono_wavs.sort_by_key(|output| output.channel_index);

        Ok(JobOutputs {
            merged_mono_wav,
            split_mono_wavs,
        })
    }

    pub(crate) fn seed_transcripts(
        store: &IndexStore,
        job_id: &str,
        transcripts: &[(&str, &str)],
    ) -> Result<(), String> {
        let transcript_dir = store.audio_store()?.spool_root().join("stt").join(job_id);
        fs::create_dir_all(&transcript_dir).map_err(|error| {
            format!(
                "failed to create test transcript dir {}: {error}",
                transcript_dir.display()
            )
        })?;

        for (transcript_id, text) in transcripts {
            let file_name = format!("{transcript_id}.txt");
            store.upsert_transcript(&TranscriptRecord {
                job_id: job_id.to_string(),
                transcript_id: (*transcript_id).to_string(),
                file_name: file_name.clone(),
                text: (*text).to_string(),
            })?;
            let transcript_path = transcript_dir.join(&file_name);
            fs::write(&transcript_path, text).map_err(|error| {
                format!(
                    "failed to seed transcript file {}: {error}",
                    transcript_path.display()
                )
            })?;
        }
        Ok(())
    }

    pub(crate) fn seed_summary(store: &IndexStore, job_id: &str, text: &str) -> Result<(), String> {
        seed_summary_with_one_line(store, job_id, text, None)
    }

    pub(crate) fn seed_summary_with_one_line(
        store: &IndexStore,
        job_id: &str,
        text: &str,
        one_line_summary: Option<&str>,
    ) -> Result<(), String> {
        store.upsert_summary(&SummaryRecord {
            job_id: job_id.to_string(),
            file_name: "result.md".to_string(),
            text: text.to_string(),
            one_line_summary: one_line_summary.map(str::to_string),
        })
    }

    fn parse_channel_index(file_name: &str) -> u32 {
        let stem = Path::new(file_name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        stem.strip_prefix("channel_")
            .and_then(|suffix| suffix.parse::<u32>().ok())
            .unwrap_or(0)
    }
}
