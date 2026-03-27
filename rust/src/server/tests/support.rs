use super::super::types::{ModelStatusEntryResponse, ModelStatusResponse};
use crate::index::{JobRecord, JobStatus, ModelKind, ModelPreparationStatus};
use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request};
use axum::response::Response;
use http_body_util::BodyExt;
use serde::Deserialize;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tower::util::ServiceExt;
use uuid::Uuid;
pub(super) fn temp_workspace() -> PathBuf {
    let path = std::env::temp_dir().join(format!("recordroute-server-{}", Uuid::now_v7()));
    fs::create_dir_all(&path).expect("temp workspace");
    path
}

pub(super) fn build_script_path(repo_root: &Path, tool: &str) -> PathBuf {
    crate::ffmpeg::build_script_path(repo_root, tool)
}

pub(super) fn fake_command_path(base_dir: &Path, name: &str) -> PathBuf {
    crate::ffmpeg::fake_command_path(base_dir, name)
}

pub(super) fn write_build_script(path: &Path) {
    write_platform_script(path, "#!/bin/sh\nexit 0\n", "@echo off\nexit /b 0\n");
}

pub(super) fn write_simple_command(path: &Path) {
    write_platform_script(path, "#!/bin/sh\nexit 0\n", "@echo off\nexit /b 0\n");
}

pub(super) fn write_fake_llama_download_command(path: &Path, log_path: &Path) {
    let log = log_path.display();
    let unix_script = format!(
        "#!/bin/sh\n: > '{log}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\ndone\nprintf 'LLAMA_CACHE=%s\\n' \"${{LLAMA_CACHE:-}}\" >> '{log}'\nmkdir -p \"$LLAMA_CACHE\"\nprintf 'synthetic model' > \"$LLAMA_CACHE/downloaded-model.gguf\"\n"
    );
    let windows_script = format!(
        "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\n> \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto after\n>> \"{log}\" echo %~1\nshift\ngoto loop\n:after\n>> \"{log}\" echo LLAMA_CACHE=!LLAMA_CACHE!\nif not exist \"!LLAMA_CACHE!\" mkdir \"!LLAMA_CACHE!\"\n> \"!LLAMA_CACHE!\\downloaded-model.gguf\" <nul set /p =synthetic model\nexit /b 0\n"
    );
    write_platform_script(path, &unix_script, &windows_script);
}

pub(super) fn write_fake_ffprobe(path: &Path, channels: u32, channel_layout: Option<&str>) {
    let json = match channel_layout {
        Some(layout) => {
            format!("{{\"streams\":[{{\"channels\":{channels},\"channel_layout\":\"{layout}\"}}]}}")
        }
        None => format!("{{\"streams\":[{{\"channels\":{channels}}}]}}"),
    };
    let unix_script = format!(
        "#!/bin/sh\nprintf '%s' '{}'\n",
        json.replace('\'', "'\"'\"'")
    );
    let windows_script = format!("@echo off\necho {json}\n");
    write_platform_script(path, &unix_script, &windows_script);
}

pub(super) fn write_fake_ffmpeg(path: &Path) {
    let unix_script = "#!/bin/sh\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    *.wav)\n      mkdir -p \"$(dirname \"$arg\")\"\n      : > \"$arg\"\n      ;;\n  esac\ndone\n";
    let windows_script = "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\nfor %%I in (\"!arg!\") do (\n  if /I \"%%~xI\"==\".wav\" (\n    if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n    > \"%%~fI\" type nul\n  )\n)\nshift\ngoto loop\n:done\nexit /b 0\n";
    write_platform_script(path, unix_script, windows_script);
}

pub(super) fn write_blocking_ffmpeg(path: &Path, gate_path: &Path) {
    let gate = gate_path.display();
    let unix_script = format!(
        "#!/bin/sh\nwhile [ -f '{gate}' ]; do\n  sleep 0.05\ndone\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    *.wav)\n      mkdir -p \"$(dirname \"$arg\")\"\n      : > \"$arg\"\n      ;;\n  esac\ndone\n"
    );
    let windows_script = format!(
        "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\n:wait\nif exist \"{gate}\" (\n  powershell -NoProfile -Command \"Start-Sleep -Milliseconds 50\" >nul 2>&1\n  goto wait\n)\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\nfor %%I in (\"!arg!\") do (\n  if /I \"%%~xI\"==\".wav\" (\n    if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n    > \"%%~fI\" type nul\n  )\n)\nshift\ngoto loop\n:done\nexit /b 0\n"
    );
    write_platform_script(path, &unix_script, &windows_script);
}

pub(super) fn write_failing_ffmpeg(path: &Path) {
    let unix_script = "#!/bin/sh\nlast=''\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    *.wav)\n      last=\"$arg\"\n      ;;\n  esac\ndone\nif [ -n \"$last\" ]; then\n  mkdir -p \"$(dirname \"$last\")\"\n  : > \"$last\"\nfi\nprintf 'synthetic ffmpeg failure' >&2\nexit 1\n";
    let windows_script = "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nset \"last=\"\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\nfor %%I in (\"!arg!\") do if /I \"%%~xI\"==\".wav\" set \"last=%%~fI\"\nshift\ngoto loop\n:done\nif defined last (\n  for %%I in (\"!last!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n  > \"!last!\" type nul\n)\necho synthetic ffmpeg failure 1>&2\nexit /b 1\n";
    write_platform_script(path, unix_script, windows_script);
}

pub(super) fn write_failing_llama_cli(path: &Path) {
    let unix_script = "#!/bin/sh\nprintf 'synthetic llama failure' >&2\nexit 1\n";
    let windows_script = "@echo off\necho synthetic llama failure 1>&2\nexit /b 1\n";
    write_platform_script(path, unix_script, windows_script);
}

pub(super) fn whisper_download_script_path(repo_root: &Path) -> PathBuf {
    if cfg!(windows) {
        repo_root.join("modules/whisper.cpp/models/download-ggml-model.cmd")
    } else {
        repo_root.join("modules/whisper.cpp/models/download-ggml-model.sh")
    }
}

pub(super) fn write_fake_download_script(path: &Path, log_path: &Path) {
    let log = log_path.display();
    let unix_script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{log}'\nmkdir -p \"$2\"\n: > \"$2/ggml-$1.bin\"\n"
    );
    let windows_script = format!(
        "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nset \"model=%~1\"\nset \"out_dir=%~2\"\nif \"!out_dir:~0,4!\"==\"\\\\?\\\" set \"out_dir=!out_dir:~4!\"\n> \"{log}\" echo(!model! !out_dir!\nif not exist \"!out_dir!\" mkdir \"!out_dir!\"\n> \"!out_dir!\\ggml-!model!.bin\" type nul\nexit /b 0\n"
    );
    write_platform_script(path, &unix_script, &windows_script);
}

pub(super) fn write_test_wav(path: &Path, channels: u16) {
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

pub(super) fn write_platform_script(path: &Path, unix_content: &str, windows_content: &str) {
    let content = if cfg!(windows) {
        windows_content.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        unix_content.to_string()
    };
    fs::write(path, content).expect("script");
    make_executable(path);
}

pub(super) fn make_executable(_path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("permissions");
    }
}

pub(super) fn post_upload_request(uri: &str, filename: &str, file_bytes: &[u8]) -> Request<Body> {
    let boundary = "recordroute-boundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(file_bytes);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .expect("upload request")
}

pub(super) fn post_json_request(uri: &str, body: &serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request")
}

pub(super) fn post_empty_request(uri: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .body(Body::empty())
        .expect("request")
}

pub(super) fn get_request(uri: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .expect("request")
}

pub(super) async fn read_json<T>(response: Response) -> T
where
    T: for<'de> Deserialize<'de>,
{
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect")
        .to_bytes();

    serde_json::from_slice(&body).unwrap_or_else(|error| {
        panic!(
            "failed to parse JSON for status {}: {}",
            status,
            String::from_utf8_lossy(&body)
                .into_owned()
                .replace('\n', "\\n")
                + &format!(" ({error})")
        )
    })
}

pub(super) async fn wait_for_job_completion(app: &Router, job_id: &str) -> JobRecord {
    let job = wait_for_job_terminal_state(app, job_id).await;
    assert_eq!(job.status, JobStatus::Completed);
    job
}

pub(super) async fn wait_for_model_preparation_state(
    app: &Router,
    model: ModelKind,
) -> ModelStatusEntryResponse {
    let deadline = Instant::now() + Duration::from_secs(5);

    loop {
        let response = app
            .clone()
            .oneshot(get_request("/models/status"))
            .await
            .expect("models status response");
        let status: ModelStatusResponse = read_json(response).await;
        let entry = match model {
            ModelKind::Whisper => status.whisper,
            ModelKind::Llama => status.llama,
        };

        if model == ModelKind::Whisper
            && entry.preparation.status != ModelPreparationStatus::Running
        {
            return entry;
        }

        if model == ModelKind::Llama
            && entry.preparation.status != ModelPreparationStatus::Running
            && (entry.preparation.status == ModelPreparationStatus::Failed
                || entry.embedding_ready
                || entry.embedding_error.is_some()
                || !entry.embedding_available)
        {
            return entry;
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for {} model preparation",
            model.as_str()
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

pub(super) async fn wait_for_job_terminal_state(app: &Router, job_id: &str) -> JobRecord {
    let deadline = Instant::now() + Duration::from_secs(5);

    loop {
        let response = app
            .clone()
            .oneshot(get_request(&format!("/jobs/{job_id}")))
            .await
            .expect("job status response");
        let job: JobRecord = read_json(response).await;

        if job.status != JobStatus::Running {
            return job;
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for job {job_id}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}
