use super::discovery::hf_download_cache_dir;
use super::output::{extract_summary_text, parse_embedding_output};
use super::{DEFAULT_PREDICT_TOKENS, LLAMA_CACHE_ENV_VAR, ModelSource, Toolchain};
use crate::tool_runtime::{
    apply_cpu_fallback_env, command_output_details, run_with_cpu_fallback, should_retry_with_cpu,
};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LlamaRuntimeBackend {
    Preferred,
    CpuFallback,
}

pub(crate) fn run_summary_embedding(
    toolchain: &Toolchain,
    input: &str,
) -> Result<Vec<f32>, String> {
    run_with_cpu_fallback(
        || run_summary_embedding_once(toolchain, input, LlamaRuntimeBackend::Preferred),
        || run_summary_embedding_once(toolchain, input, LlamaRuntimeBackend::CpuFallback),
        should_retry_llama_on_cpu,
    )
}

fn run_summary_embedding_once(
    toolchain: &Toolchain,
    input: &str,
    backend: LlamaRuntimeBackend,
) -> Result<Vec<f32>, String> {
    let mut command = Command::new(&toolchain.llama_embedding_path);
    command
        .arg("--pooling")
        .arg("mean")
        .arg("--embd-normalize")
        .arg("2")
        .arg("--embd-output-format")
        .arg("array");
    configure_embedding_runtime_backend(&mut command, backend);
    command.arg("-p").arg(input);

    match toolchain.runtime_embedding_model_source() {
        ModelSource::LocalPath(path) => {
            command.arg("-m").arg(path);
        }
        ModelSource::HuggingFaceRepo(repo) => {
            command.env(
                LLAMA_CACHE_ENV_VAR,
                hf_download_cache_dir(&toolchain.build_script_path, &repo),
            );
            command.arg("-hf").arg(repo);
        }
    }

    let output = command.output().map_err(|error| {
        format!(
            "failed to execute llama-embedding {}: {error}",
            toolchain.llama_embedding_path.display()
        )
    })?;
    if !output.status.success() {
        return Err(format!(
            "llama embedding failed: {}",
            command_output_details(&output)
        ));
    }

    parse_embedding_output(&output)
}

pub(crate) fn run_summary_generation(
    toolchain: &Toolchain,
    prompt_file: &Path,
    output_file: &Path,
) -> Result<(), String> {
    if !prompt_file.is_file() {
        return Err(format!(
            "summary prompt file not found: {}",
            prompt_file.display()
        ));
    }

    if let Some(parent) = output_file.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create summary output directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let _ = fs::remove_file(output_file);
    let output = run_with_cpu_fallback(
        || run_summary_generation_once(toolchain, prompt_file, LlamaRuntimeBackend::Preferred),
        || run_summary_generation_once(toolchain, prompt_file, LlamaRuntimeBackend::CpuFallback),
        should_retry_llama_on_cpu,
    )?;

    let prompt = fs::read_to_string(prompt_file).unwrap_or_default();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let summary_text = extract_summary_text(&stdout, &prompt);

    if summary_text.is_empty() {
        return Err(format!(
            "llama-cli completed without producing summary text for {}",
            prompt_file.display()
        ));
    }

    fs::write(output_file, summary_text).map_err(|error| {
        format!(
            "failed to write summary output {}: {error}",
            output_file.display()
        )
    })
}

fn run_summary_generation_once(
    toolchain: &Toolchain,
    prompt_file: &Path,
    backend: LlamaRuntimeBackend,
) -> Result<Output, String> {
    let mut command = Command::new(&toolchain.llama_cli_path);
    command
        .arg("--single-turn")
        .arg("--simple-io")
        .arg("--no-display-prompt")
        .arg("--log-disable")
        .arg("-n")
        .arg(DEFAULT_PREDICT_TOKENS);
    configure_runtime_backend(&mut command, backend);

    match toolchain.runtime_model_source() {
        ModelSource::LocalPath(path) => {
            command.arg("-m").arg(path);
        }
        ModelSource::HuggingFaceRepo(repo) => {
            command.env(
                LLAMA_CACHE_ENV_VAR,
                hf_download_cache_dir(&toolchain.build_script_path, &repo),
            );
            command.arg("-hf").arg(repo);
        }
    }

    let output = command
        .arg("-f")
        .arg(prompt_file)
        .output()
        .map_err(|error| {
            format!(
                "failed to execute llama-cli {}: {error}",
                toolchain.llama_cli_path.display()
            )
        })?;

    if output.status.success() {
        Ok(output)
    } else {
        Err(summary_generation_error(prompt_file, &output))
    }
}

fn summary_generation_error(prompt_file: &Path, output: &Output) -> String {
    format!(
        "llama summary generation failed for {}: {}",
        prompt_file.display(),
        command_output_details(output)
    )
}

pub(super) fn configure_runtime_backend(command: &mut Command, backend: LlamaRuntimeBackend) {
    match backend {
        LlamaRuntimeBackend::CpuFallback => {
            command
                .arg("-ngl")
                .arg("0")
                .arg("--device")
                .arg("none")
                .arg("--no-op-offload")
                .arg("--no-kv-offload")
                .arg("--no-mmproj-offload");

            apply_cpu_fallback_env(command);
        }
        LlamaRuntimeBackend::Preferred => {}
    }
}

fn configure_embedding_runtime_backend(command: &mut Command, backend: LlamaRuntimeBackend) {
    match backend {
        LlamaRuntimeBackend::CpuFallback => {
            command
                .arg("-ngl")
                .arg("0")
                .arg("--device")
                .arg("none")
                .arg("--no-op-offload")
                .arg("--no-kv-offload");

            apply_cpu_fallback_env(command);
        }
        LlamaRuntimeBackend::Preferred => {}
    }
}

pub(super) fn should_retry_llama_on_cpu(error: &str) -> bool {
    should_retry_with_cpu(
        error,
        &["metal", "ggml-metal", "ggml_metal", "mtl"],
        &["cuda", "cublas", "ggml-cuda", "ggml_cuda", "nvidia"],
    )
}
