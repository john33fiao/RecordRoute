use super::Toolchain;
use super::download::{refresh_managed_model, should_refresh_managed_model};
use super::output::{postprocess_transcript, transcript_output_prefix};
use crate::tool_runtime::{
    apply_cpu_fallback_env, command_output_details, run_with_cpu_fallback, should_retry_with_cpu,
};
use std::fs;
use std::path::Path;
use std::process::Command;

pub(crate) fn run_transcription(
    toolchain: &Toolchain,
    input: &Path,
    output_text: &Path,
) -> Result<(), String> {
    if !input.is_file() {
        return Err(format!(
            "input audio file not found for transcription: {}",
            input.display()
        ));
    }

    let output_prefix = transcript_output_prefix(output_text)?;
    if let Some(parent) = output_text.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create transcription output directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let _ = fs::remove_file(output_text);

    match attempt_transcription(toolchain, input, &output_prefix) {
        Ok(()) => {}
        Err(error) if should_refresh_managed_model(toolchain, &error) => {
            refresh_managed_model(toolchain)?;
            attempt_transcription(toolchain, input, &output_prefix).map_err(|retry_error| {
                format!(
                    "{retry_error} (after refreshing managed model cache {})",
                    toolchain.model_path.display()
                )
            })?;
        }
        Err(error) => return Err(error),
    }

    if output_text.is_file() {
        postprocess_transcript(output_text)?;
        return Ok(());
    }

    Err(format!(
        "whisper-cli completed without creating transcript {}",
        output_text.display()
    ))
}

fn attempt_transcription(
    toolchain: &Toolchain,
    input: &Path,
    output_prefix: &Path,
) -> Result<(), String> {
    run_with_cpu_fallback(
        || {
            execute_transcription(
                toolchain,
                input,
                output_prefix,
                WhisperRuntimeBackend::Preferred,
            )
        },
        || {
            execute_transcription(
                toolchain,
                input,
                output_prefix,
                WhisperRuntimeBackend::CpuFallback,
            )
        },
        should_retry_transcription_on_cpu,
    )
}

fn execute_transcription(
    toolchain: &Toolchain,
    input: &Path,
    output_prefix: &Path,
    backend: WhisperRuntimeBackend,
) -> Result<(), String> {
    let mut command = Command::new(&toolchain.whisper_cli_path);
    command
        .arg("-m")
        .arg(&toolchain.model_path)
        .arg("-f")
        .arg(input)
        .arg("-l")
        .arg("auto")
        .arg("-otxt")
        .arg("-np")
        .arg("-of")
        .arg(output_prefix);
    configure_runtime_backend(&mut command, backend);

    let output = command.output().map_err(|error| {
        format!(
            "failed to execute whisper-cli {}: {error}",
            toolchain.whisper_cli_path.display()
        )
    })?;

    if output.status.success() {
        return Ok(());
    }

    Err(format!(
        "whisper transcription failed for {}: {}",
        input.display(),
        command_output_details(&output)
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WhisperRuntimeBackend {
    Preferred,
    CpuFallback,
}

fn configure_runtime_backend(command: &mut Command, backend: WhisperRuntimeBackend) {
    match backend {
        WhisperRuntimeBackend::CpuFallback => {
            command.arg("-ng");
            apply_cpu_fallback_env(command);
        }
        WhisperRuntimeBackend::Preferred => {}
    }
}

fn should_retry_transcription_on_cpu(error: &str) -> bool {
    should_retry_with_cpu(
        error,
        &[
            "metal",
            "ggml-metal",
            "ggml_metal",
            "mtl",
            "failed to initialize whisper context",
        ],
        &["cuda", "cublas", "ggml-cuda", "ggml_cuda", "nvidia"],
    )
}
