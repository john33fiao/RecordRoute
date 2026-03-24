use crate::ffmpeg::{ConversionOutputs, Toolchain, probe_audio_input, run_conversion};
use crate::index::{IndexStore, JobOutputs, JobProbe, JobRecord, JobSplitOutput};
use std::ffi::OsString;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use uuid::Uuid;

const RUN_ID_FORMAT: &[time::format_description::FormatItem<'static>] =
    format_description!("[year][month][day]T[hour][minute][second]");

pub fn main_cli() -> Result<(), String> {
    let repo_root = repo_root()?;
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    let input = resolve_input_path(&args, &mut reader, &mut writer)?;
    let summary = run_with_repo_root(&repo_root, &input)?;

    writeln!(writer, "job_id: {}", summary.job_id).map_err(|error| error.to_string())?;
    writeln!(writer, "job_dir: {}", summary.job_dir.display())
        .map_err(|error| error.to_string())?;
    writeln!(
        writer,
        "merged_mono_wav: {}",
        summary.outputs.merged_mono_wav.display()
    )
    .map_err(|error| error.to_string())?;
    for split in &summary.outputs.split_mono_wavs {
        writeln!(
            writer,
            "channel_{:02}: {}",
            split.channel_index,
            split.path.display()
        )
        .map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub fn resolve_input_path(
    args: &[OsString],
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
) -> Result<PathBuf, String> {
    match args {
        [] => {
            write!(writer, "Input audio path: ").map_err(|error| error.to_string())?;
            writer.flush().map_err(|error| error.to_string())?;

            let mut line = String::new();
            reader
                .read_line(&mut line)
                .map_err(|error| format!("failed to read input path: {error}"))?;

            let trimmed = line.trim();
            if trimmed.is_empty() {
                return Err("input path is required".to_string());
            }

            Ok(PathBuf::from(trimmed))
        }
        [path] => Ok(PathBuf::from(path)),
        _ => Err("expected exactly one input path".to_string()),
    }
}

pub fn run_with_repo_root(repo_root: &Path, input: &Path) -> Result<RunSummary, String> {
    let input_path = normalize_input_path(input)?;
    let toolchain = Toolchain::discover(repo_root)?;
    let index_store = IndexStore::new(repo_root);
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

    let mut job = JobRecord::new(
        job_id.clone(),
        started_at,
        input_path.clone(),
        job_dir.clone(),
    );
    index_store.insert_job(job.clone())?;

    let probe = match probe_audio_input(&toolchain, &input_path) {
        Ok(probe) => probe,
        Err(error) => {
            job.mark_failed(now_rfc3339()?, error.clone());
            index_store.update_job(&job_id, |_| job.clone())?;
            return Err(error);
        }
    };

    job.probe = JobProbe {
        channels: Some(probe.channels),
        channel_layout: probe.channel_layout.clone(),
    };

    let planned_outputs = ConversionOutputs::new(&job_dir, probe.channels);

    match run_conversion(&toolchain, &input_path, probe.channels, &planned_outputs) {
        Ok(()) => {
            job.mark_completed(
                now_rfc3339()?,
                JobOutputs {
                    merged_mono_wav: Some(path_to_string(&planned_outputs.merged_mono_wav)),
                    split_mono_wavs: planned_outputs
                        .split_mono_wavs
                        .iter()
                        .map(|output| JobSplitOutput {
                            channel_index: output.channel_index,
                            path: path_to_string(&output.path),
                        })
                        .collect(),
                },
            );
            index_store.update_job(&job_id, |_| job.clone())?;

            Ok(RunSummary {
                job_id,
                job_dir,
                outputs: planned_outputs,
            })
        }
        Err(error) => {
            planned_outputs.cleanup_partial_files();
            job.mark_failed(now_rfc3339()?, error.clone());
            index_store.update_job(&job_id, |_| job.clone())?;
            Err(error)
        }
    }
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

fn repo_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to resolve repository root".to_string())
}

fn build_run_id() -> Result<String, String> {
    let timestamp = OffsetDateTime::now_utc()
        .format(RUN_ID_FORMAT)
        .map_err(|error| format!("failed to format timestamp: {error}"))?;
    Ok(format!("{timestamp}_{}", Uuid::now_v7()))
}

fn now_rfc3339() -> Result<String, String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| format!("failed to format timestamp: {error}"))
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[derive(Debug)]
pub struct RunSummary {
    pub job_id: String,
    pub job_dir: PathBuf,
    pub outputs: ConversionOutputs,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffmpeg::Toolchain;
    use std::fs::{self, File};
    use std::io::Cursor;

    #[test]
    fn resolves_single_path_argument() {
        let args = vec![OsString::from("/tmp/input.mp3")];
        let mut reader = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();

        let path = resolve_input_path(&args, &mut reader, &mut output).expect("path should parse");

        assert_eq!(path, PathBuf::from("/tmp/input.mp3"));
        assert!(output.is_empty());
    }

    #[test]
    fn prompts_for_missing_path() {
        let args = Vec::<OsString>::new();
        let mut reader = Cursor::new(b"/tmp/from-prompt.wav\n".to_vec());
        let mut output = Vec::new();

        let path =
            resolve_input_path(&args, &mut reader, &mut output).expect("prompt should parse");

        assert_eq!(path, PathBuf::from("/tmp/from-prompt.wav"));
        assert_eq!(
            String::from_utf8(output).expect("utf8"),
            "Input audio path: "
        );
    }

    #[test]
    fn rejects_multiple_paths() {
        let args = vec![OsString::from("one"), OsString::from("two")];
        let mut reader = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();

        let error = resolve_input_path(&args, &mut reader, &mut output).expect_err("should fail");

        assert_eq!(error, "expected exactly one input path");
    }

    #[test]
    fn run_id_contains_timestamp_and_uuid() {
        let run_id = build_run_id().expect("run id");
        let (timestamp, uuid) = run_id.split_once('_').expect("split run id");

        assert_eq!(timestamp.len(), 15);
        assert_eq!(&timestamp[8..9], "T");
        assert!(Uuid::parse_str(uuid).is_ok());
    }

    #[test]
    fn end_to_end_flow_uses_fake_toolchain() {
        let repo_root = temp_workspace();
        let scripts_dir = repo_root.join("scripts");
        let build_bin = repo_root
            .join(".build/ffmpeg")
            .join(crate::ffmpeg::target_dir_name())
            .join("install/bin");
        let input = repo_root.join("fixture.wav");
        let ffmpeg_log = repo_root.join("ffmpeg-args.log");
        fs::create_dir_all(&scripts_dir).expect("scripts dir");
        fs::create_dir_all(&build_bin).expect("toolchain dir");
        write_build_script(&scripts_dir.join("build_ffmpeg.sh"));
        write_fake_ffprobe(&build_bin.join("ffprobe"), 2, Some("stereo"));
        write_fake_ffmpeg(&build_bin.join("ffmpeg"), &ffmpeg_log);
        write_test_wav(&input, 2);

        let summary = run_with_repo_root(&repo_root, &input).expect("run should succeed");

        assert!(summary.outputs.merged_mono_wav.exists());
        assert_eq!(summary.outputs.split_mono_wavs.len(), 2);
        assert!(summary.outputs.split_mono_wavs[0].path.exists());
        assert!(summary.outputs.split_mono_wavs[1].path.exists());

        let log = fs::read_to_string(ffmpeg_log).expect("ffmpeg log");
        assert!(log.contains("-filter_complex"));
        assert!(log.contains(&crate::ffmpeg::build_filter_complex(2)));

        let index = fs::read_to_string(repo_root.join("db/index.json")).expect("index");
        let parsed: serde_json::Value = serde_json::from_str(&index).expect("valid json");
        assert_eq!(parsed["version"], 1);
        let jobs = parsed["jobs"].as_array().expect("jobs array");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0]["status"], "completed");
        assert_eq!(jobs[0]["probe"]["channels"], 2);
        assert_eq!(
            jobs[0]["outputs"]["split_mono_wavs"]
                .as_array()
                .expect("array")
                .len(),
            2
        );
    }

    #[test]
    fn failure_marks_job_failed_and_cleans_partial_outputs() {
        let repo_root = temp_workspace();
        let scripts_dir = repo_root.join("scripts");
        let build_bin = repo_root
            .join(".build/ffmpeg")
            .join(crate::ffmpeg::target_dir_name())
            .join("install/bin");
        let input = repo_root.join("fixture.wav");
        fs::create_dir_all(&scripts_dir).expect("scripts dir");
        fs::create_dir_all(&build_bin).expect("toolchain dir");
        write_build_script(&scripts_dir.join("build_ffmpeg.sh"));
        write_fake_ffprobe(&build_bin.join("ffprobe"), 2, Some("stereo"));
        write_failing_ffmpeg(&build_bin.join("ffmpeg"));
        write_test_wav(&input, 2);

        let error = run_with_repo_root(&repo_root, &input).expect_err("run should fail");

        assert!(error.contains("ffmpeg conversion failed"));

        let index = fs::read_to_string(repo_root.join("db/index.json")).expect("index");
        let parsed: serde_json::Value = serde_json::from_str(&index).expect("valid json");
        let jobs = parsed["jobs"].as_array().expect("jobs array");
        assert_eq!(jobs[0]["status"], "failed");
        assert!(
            jobs[0]["error_message"]
                .as_str()
                .expect("error")
                .contains("ffmpeg conversion failed")
        );

        let job_dir = PathBuf::from(jobs[0]["job_dir"].as_str().expect("job dir"));
        assert!(job_dir.exists());
        assert!(!job_dir.join("channel_01.wav").exists());
        assert!(!job_dir.join("channel_02.wav").exists());
        assert!(!job_dir.join("mono_mix.wav").exists());
    }

    #[test]
    fn missing_toolchain_reports_bootstrap_path() {
        let repo_root = temp_workspace();
        let input = repo_root.join("fixture.wav");
        fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
        write_build_script(&repo_root.join("scripts/build_ffmpeg.sh"));
        write_test_wav(&input, 1);

        let error =
            run_with_repo_root(&repo_root, &input).expect_err("toolchain should be required");

        assert!(error.contains("scripts/build_ffmpeg.sh"));
        assert!(!repo_root.join("db/index.json").exists());
    }

    fn temp_workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!("recordroute-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).expect("temp workspace");
        path
    }

    fn write_build_script(path: &Path) {
        fs::write(path, "#!/bin/sh\nexit 0\n").expect("build script");
        make_executable(path);
    }

    fn write_fake_ffprobe(path: &Path, channels: u32, channel_layout: Option<&str>) {
        let json = match channel_layout {
            Some(layout) => format!(
                "{{\"streams\":[{{\"channels\":{channels},\"channel_layout\":\"{layout}\"}}]}}"
            ),
            None => format!("{{\"streams\":[{{\"channels\":{channels}}}]}}"),
        };
        let script = format!(
            "#!/bin/sh\nprintf '%s' '{}'\n",
            json.replace('\'', "'\"'\"'")
        );
        fs::write(path, script).expect("ffprobe script");
        make_executable(path);
    }

    fn write_fake_ffmpeg(path: &Path, log_path: &Path) {
        let log = log_path.display();
        let script = format!(
            "#!/bin/sh\n: > '{log}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\n  case \"$arg\" in\n    *.wav)\n      mkdir -p \"$(dirname \"$arg\")\"\n      : > \"$arg\"\n      ;;\n  esac\ndone\n"
        );
        fs::write(path, script).expect("ffmpeg script");
        make_executable(path);
    }

    fn write_failing_ffmpeg(path: &Path) {
        let script = "#!/bin/sh\nlast=''\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    *.wav)\n      last=\"$arg\"\n      ;;\n  esac\ndone\nif [ -n \"$last\" ]; then\n  mkdir -p \"$(dirname \"$last\")\"\n  : > \"$last\"\nfi\nprintf 'synthetic ffmpeg failure' >&2\nexit 1\n";
        fs::write(path, script).expect("ffmpeg script");
        make_executable(path);
    }

    fn write_test_wav(path: &Path, channels: u16) {
        let mut file = File::create(path).expect("fixture wav");
        let sample_rate: u32 = 16_000;
        let bits_per_sample: u16 = 16;
        let samples_per_channel: u32 = 16;
        let bytes_per_sample = u32::from(bits_per_sample / 8);
        let data_size = samples_per_channel * u32::from(channels) * bytes_per_sample;
        let byte_rate = sample_rate * u32::from(channels) * bytes_per_sample;
        let block_align = channels * (bits_per_sample / 8);

        use std::io::Write as _;
        file.write_all(b"RIFF").expect("riff");
        file.write_all(&(36 + data_size).to_le_bytes())
            .expect("chunk size");
        file.write_all(b"WAVE").expect("wave");
        file.write_all(b"fmt ").expect("fmt");
        file.write_all(&16u32.to_le_bytes())
            .expect("fmt chunk size");
        file.write_all(&1u16.to_le_bytes()).expect("pcm");
        file.write_all(&channels.to_le_bytes()).expect("channels");
        file.write_all(&sample_rate.to_le_bytes()).expect("rate");
        file.write_all(&byte_rate.to_le_bytes()).expect("byte rate");
        file.write_all(&block_align.to_le_bytes())
            .expect("block align");
        file.write_all(&bits_per_sample.to_le_bytes())
            .expect("bits");
        file.write_all(b"data").expect("data");
        file.write_all(&data_size.to_le_bytes()).expect("data size");
        file.write_all(&vec![0u8; data_size as usize])
            .expect("samples");
    }

    fn make_executable(path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("permissions");
        }
    }

    #[test]
    fn toolchain_discovers_test_target() {
        let repo_root = temp_workspace();
        let scripts_dir = repo_root.join("scripts");
        let build_bin = repo_root
            .join(".build/ffmpeg")
            .join(crate::ffmpeg::target_dir_name())
            .join("install/bin");
        fs::create_dir_all(&scripts_dir).expect("scripts dir");
        fs::create_dir_all(&build_bin).expect("toolchain dir");
        write_build_script(&scripts_dir.join("build_ffmpeg.sh"));
        write_fake_ffprobe(&build_bin.join("ffprobe"), 1, Some("mono"));
        write_fake_ffmpeg(&build_bin.join("ffmpeg"), &repo_root.join("ffmpeg.log"));

        let toolchain = Toolchain::discover(&repo_root).expect("toolchain");

        assert_eq!(toolchain.ffmpeg_path, build_bin.join("ffmpeg"));
        assert_eq!(toolchain.ffprobe_path, build_bin.join("ffprobe"));
    }
}
