use super::output::extract_summary_text;
use super::*;
use crate::ffmpeg::build_script_path;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[test]
fn discovers_toolchain_with_default_hugging_face_model() {
    let _guard = crate::test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe { std::env::remove_var(MODEL_ENV_VAR) };

    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    fs::create_dir_all(&build_bin).expect("build bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    write_build_script(&build_script_path(&repo_root, "llama"));
    write_executable(
        &fake_llama_cli_path(&build_bin),
        "#!/bin/sh\nexit 0\n",
        "@echo off\nexit /b 0\n",
    );

    let toolchain = Toolchain::discover(&repo_root).expect("toolchain");

    assert_eq!(
        toolchain.model_source,
        ModelSource::HuggingFaceRepo(DEFAULT_MODEL_REPOSITORY.to_string())
    );
    assert_eq!(
        toolchain.cached_model_path,
        Some(
            repo_root
                .join("models/llama/hf")
                .join("ggml-org__gemma-3-4b-it-GGUF.gguf")
        )
    );
}

#[test]
fn resolves_existing_relative_model_path_from_env() {
    let _guard = crate::test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let model_path = repo_root.join("models/llama/custom.gguf");
    fs::create_dir_all(&build_bin).expect("build bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(model_path.parent().expect("parent")).expect("models dir");
    fs::write(&model_path, "model").expect("model");
    write_build_script(&build_script_path(&repo_root, "llama"));
    write_executable(
        &fake_llama_cli_path(&build_bin),
        "#!/bin/sh\nexit 0\n",
        "@echo off\nexit /b 0\n",
    );

    unsafe { std::env::set_var(MODEL_ENV_VAR, "models/llama/custom.gguf") };
    let toolchain = Toolchain::discover(&repo_root).expect("toolchain");
    unsafe { std::env::remove_var(MODEL_ENV_VAR) };

    assert_eq!(toolchain.model_source, ModelSource::LocalPath(model_path));
    assert_eq!(toolchain.cached_model_path, None);
}

#[test]
fn run_summary_generation_uses_preferred_backend_first() {
    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let prompt_file = repo_root.join("summary/prompt.txt");
    let output_file = repo_root.join("summary/output.txt");
    let log = repo_root.join("llama-summary.log");
    let model_path = repo_root.join("models/llama/local.gguf");
    fs::create_dir_all(&build_bin).expect("build bin");
    fs::create_dir_all(model_path.parent().expect("model parent")).expect("models dir");
    fs::create_dir_all(prompt_file.parent().expect("prompt parent")).expect("prompt dir");
    fs::write(&model_path, "model").expect("model");
    fs::write(&prompt_file, "prompt").expect("prompt");
    write_executable(
        &fake_llama_cli_path(&build_bin),
        &format!(
            "#!/bin/sh\n: > '{log}'\nfor arg in \"$@\"; do\n  printf 'ARG=%s\\n' \"$arg\" >> '{log}'\ndone\nprintf 'GGML_METAL=%s\\n' \"${{GGML_METAL-}}\" >> '{log}'\nprintf 'GGML_METAL_DEVICES=%s\\n' \"${{GGML_METAL_DEVICES-}}\" >> '{log}'\nprintf 'synthetic summary'\n",
            log = log.display()
        ),
        &format!(
            "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\n> \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto after\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo ARG=!arg!\nshift\ngoto loop\n:after\n>> \"{log}\" echo GGML_METAL=%GGML_METAL%\n>> \"{log}\" echo GGML_METAL_DEVICES=%GGML_METAL_DEVICES%\n<nul set /p =synthetic summary\nexit /b 0\n",
            log = log.display()
        ),
    );

    let toolchain = Toolchain {
        llama_cli_path: fake_llama_cli_path(&build_bin),
        llama_embedding_path: crate::ffmpeg::fake_command_path(&build_bin, "llama-embedding"),
        build_script_path: build_script_path(&repo_root, "llama"),
        model_source: ModelSource::LocalPath(model_path.clone()),
        cached_model_path: None,
        embedding_model_source: ModelSource::LocalPath(model_path.clone()),
        embedding_cached_model_path: None,
    };

    run_summary_generation(&toolchain, &prompt_file, &output_file).expect("summary");

    assert_eq!(
        fs::read_to_string(output_file).expect("summary output"),
        "synthetic summary"
    );

    let log = fs::read_to_string(log).expect("llama log");
    assert!(log.contains("ARG=--single-turn"));
    assert!(log.contains("ARG=-m"));
    assert!(log.contains(&format!("ARG={}", model_path.display())));
    assert!(!log.contains("ARG=-ngl"));
    assert!(!log.contains("ARG=--device"));
    assert!(!log.contains("ARG=none"));
    assert!(!log.contains("ARG=--no-op-offload"));
    assert!(!log.contains("ARG=--no-kv-offload"));
    assert!(!log.contains("ARG=--no-mmproj-offload"));
    assert!(!log.contains("GGML_METAL=0"));
    assert!(!log.contains("GGML_METAL_DEVICES=0"));
}

#[test]
fn run_summary_embedding_parses_nested_array_output_without_log_disable() {
    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let log = repo_root.join("llama-embedding.log");
    let model_path = repo_root.join("models/llama/local.gguf");
    fs::create_dir_all(&build_bin).expect("build bin");
    fs::create_dir_all(model_path.parent().expect("model parent")).expect("models dir");
    fs::write(&model_path, "model").expect("model");
    write_executable(
        &crate::ffmpeg::fake_command_path(&build_bin, "llama-embedding"),
        &format!(
            "#!/bin/sh\n: > '{log}'\nfor arg in \"$@\"; do\n  printf 'ARG=%s\\n' \"$arg\" >> '{log}'\ndone\nprintf '[[0.25,0.75]]'\n",
            log = log.display()
        ),
        &format!(
            "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\n> \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto after\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo ARG=!arg!\nshift\ngoto loop\n:after\n<nul set /p =[[0.25,0.75]]\nexit /b 0\n",
            log = log.display()
        ),
    );

    let toolchain = Toolchain {
        llama_cli_path: fake_llama_cli_path(&build_bin),
        llama_embedding_path: crate::ffmpeg::fake_command_path(&build_bin, "llama-embedding"),
        build_script_path: build_script_path(&repo_root, "llama"),
        model_source: ModelSource::LocalPath(model_path.clone()),
        cached_model_path: None,
        embedding_model_source: ModelSource::LocalPath(model_path),
        embedding_cached_model_path: None,
    };

    let vector = run_summary_embedding(&toolchain, "hello embedding").expect("embedding");

    assert_eq!(vector, vec![0.25, 0.75]);

    let log = fs::read_to_string(log).expect("embedding log");
    assert!(log.contains("ARG=--embd-output-format"));
    assert!(log.contains("ARG=array"));
    assert!(!log.contains("ARG=--log-disable"));
}

#[test]
fn run_summary_embedding_surfaces_empty_stdout_details() {
    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let model_path = repo_root.join("models/llama/local.gguf");
    fs::create_dir_all(&build_bin).expect("build bin");
    fs::create_dir_all(model_path.parent().expect("model parent")).expect("models dir");
    fs::write(&model_path, "model").expect("model");
    write_executable(
        &crate::ffmpeg::fake_command_path(&build_bin, "llama-embedding"),
        "#!/bin/sh\nprintf 'no embedding emitted\\n' >&2\nexit 0\n",
        "@echo off\necho no embedding emitted 1>&2\nexit /b 0\n",
    );

    let toolchain = Toolchain {
        llama_cli_path: fake_llama_cli_path(&build_bin),
        llama_embedding_path: crate::ffmpeg::fake_command_path(&build_bin, "llama-embedding"),
        build_script_path: build_script_path(&repo_root, "llama"),
        model_source: ModelSource::LocalPath(model_path.clone()),
        cached_model_path: None,
        embedding_model_source: ModelSource::LocalPath(model_path),
        embedding_cached_model_path: None,
    };

    let error = run_summary_embedding(&toolchain, "hello embedding").expect_err("error");

    assert!(error.contains("completed without producing embedding output"));
    assert!(error.contains("no embedding emitted"));
}

#[cfg(any(target_os = "macos", windows))]
#[test]
fn run_summary_embedding_retries_on_cpu_after_backend_failure() {
    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let log = repo_root.join("llama-embedding-retry.log");
    let count = repo_root.join("llama-embedding-retry.count");
    let model_path = repo_root.join("models/llama/local.gguf");
    let failure_marker = if cfg!(target_os = "macos") {
        "metal backend failure"
    } else {
        "cuda backend failure"
    };
    fs::create_dir_all(&build_bin).expect("build bin");
    fs::create_dir_all(model_path.parent().expect("model parent")).expect("models dir");
    fs::write(&model_path, "model").expect("model");
    write_executable(
        &crate::ffmpeg::fake_command_path(&build_bin, "llama-embedding"),
        &format!(
            "#!/bin/sh\ncount=0\nif [ -f '{count}' ]; then\n  count=$(cat '{count}')\nfi\ncount=$((count + 1))\nprintf '%s' \"$count\" > '{count}'\nlog='{log}'\ncpu='0'\nprintf 'CALL=%s\\n' \"$count\" >> \"$log\"\nprintf 'GGML_METAL=%s\\n' \"${{GGML_METAL-}}\" >> \"$log\"\nprintf 'GGML_METAL_DEVICES=%s\\n' \"${{GGML_METAL_DEVICES-}}\" >> \"$log\"\nfor arg in \"$@\"; do\n  printf 'CALL_%s_ARG=%s\\n' \"$count\" \"$arg\" >> \"$log\"\n  case \"$arg\" in\n    -ngl|--device|none|--no-op-offload|--no-kv-offload)\n      cpu='1'\n      ;;\n  esac\ndone\nif [ \"$count\" = '1' ]; then\n  if [ \"$cpu\" = '1' ]; then\n    printf 'unexpected cpu fallback on first attempt\\n' >&2\n    exit 2\n  fi\n  printf 'error: {failure}\\n' >&2\n  exit 1\nfi\nif [ \"$cpu\" != '1' ]; then\n  printf 'error: second attempt still used gpu backend\\n' >&2\n  exit 3\nfi\nprintf '[[0.5,0.25]]'\n",
            count = count.display(),
            log = log.display(),
            failure = failure_marker,
        ),
        &format!(
            "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nset \"count=0\"\nif exist \"{count}\" set /p count=<\"{count}\"\nset /a count+=1\n> \"{count}\" <nul set /p =!count!\nset \"cpu=0\"\n>> \"{log}\" echo CALL=!count!\n>> \"{log}\" echo GGML_METAL=%GGML_METAL%\n>> \"{log}\" echo GGML_METAL_DEVICES=%GGML_METAL_DEVICES%\n:loop\nif \"%~1\"==\"\" goto after\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo CALL_!count!_ARG=!arg!\nif /I \"!arg!\"==\"-ngl\" set \"cpu=1\"\nif /I \"!arg!\"==\"--device\" set \"cpu=1\"\nif /I \"!arg!\"==\"none\" set \"cpu=1\"\nif /I \"!arg!\"==\"--no-op-offload\" set \"cpu=1\"\nif /I \"!arg!\"==\"--no-kv-offload\" set \"cpu=1\"\nshift\ngoto loop\n:after\nif \"!count!\"==\"1\" (\n  if \"!cpu!\"==\"1\" (\n    echo unexpected cpu fallback on first attempt 1>&2\n    exit /b 2\n  )\n  echo error: {failure} 1>&2\n  exit /b 1\n)\nif not \"!cpu!\"==\"1\" (\n  echo error: second attempt still used gpu backend 1>&2\n  exit /b 3\n)\n<nul set /p =[[0.5,0.25]]\nexit /b 0\n",
            count = count.display(),
            log = log.display(),
            failure = failure_marker,
        ),
    );

    let toolchain = Toolchain {
        llama_cli_path: fake_llama_cli_path(&build_bin),
        llama_embedding_path: crate::ffmpeg::fake_command_path(&build_bin, "llama-embedding"),
        build_script_path: build_script_path(&repo_root, "llama"),
        model_source: ModelSource::LocalPath(model_path.clone()),
        cached_model_path: None,
        embedding_model_source: ModelSource::LocalPath(model_path),
        embedding_cached_model_path: None,
    };

    let vector = run_summary_embedding(&toolchain, "hello embedding").expect("embedding");

    assert_eq!(vector, vec![0.5, 0.25]);

    let log = fs::read_to_string(log).expect("retry log");
    assert!(log.contains("CALL=1"));
    assert!(log.contains("CALL=2"));
    assert!(!log.contains("CALL_1_ARG=-ngl"));
    assert!(log.contains("CALL_2_ARG=-ngl"));
    assert!(log.contains("CALL_2_ARG=0"));
    assert!(log.contains("CALL_2_ARG=--device"));
    assert!(log.contains("CALL_2_ARG=none"));
    assert!(log.contains("CALL_2_ARG=--no-op-offload"));
    assert!(log.contains("CALL_2_ARG=--no-kv-offload"));
    assert!(!log.contains("CALL_2_ARG=--no-mmproj-offload"));
    if cfg!(target_os = "macos") {
        assert!(log.contains("GGML_METAL=0"));
        assert!(log.contains("GGML_METAL_DEVICES=0"));
    }
}

#[cfg(any(target_os = "macos", windows))]
#[test]
fn run_summary_generation_retries_on_cpu_after_backend_failure() {
    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let prompt_file = repo_root.join("summary/prompt.txt");
    let output_file = repo_root.join("summary/output.txt");
    let log = repo_root.join("llama-retry.log");
    let count = repo_root.join("llama-retry.count");
    let model_path = repo_root.join("models/llama/local.gguf");
    let failure_marker = if cfg!(target_os = "macos") {
        "metal backend failure"
    } else {
        "cuda backend failure"
    };
    fs::create_dir_all(&build_bin).expect("build bin");
    fs::create_dir_all(model_path.parent().expect("model parent")).expect("models dir");
    fs::create_dir_all(prompt_file.parent().expect("prompt parent")).expect("prompt dir");
    fs::write(&model_path, "model").expect("model");
    fs::write(&prompt_file, "prompt").expect("prompt");
    write_executable(
        &fake_llama_cli_path(&build_bin),
        &format!(
            "#!/bin/sh\ncount=0\nif [ -f '{count}' ]; then\n  count=$(cat '{count}')\nfi\ncount=$((count + 1))\nprintf '%s' \"$count\" > '{count}'\ncpu='0'\nlog='{log}'\nprintf 'CALL=%s\\n' \"$count\" >> \"$log\"\nprintf 'GGML_METAL=%s\\n' \"${{GGML_METAL-}}\" >> \"$log\"\nprintf 'GGML_METAL_DEVICES=%s\\n' \"${{GGML_METAL_DEVICES-}}\" >> \"$log\"\nfor arg in \"$@\"; do\n  printf 'CALL_%s_ARG=%s\\n' \"$count\" \"$arg\" >> \"$log\"\n  case \"$arg\" in\n    -ngl|--device|none|--no-op-offload|--no-kv-offload|--no-mmproj-offload)\n      cpu='1'\n      ;;\n  esac\ndone\nif [ \"$count\" = '1' ]; then\n  if [ \"$cpu\" = '1' ]; then\n    printf 'unexpected cpu fallback on first attempt\\n' >&2\n    exit 2\n  fi\n  printf 'error: {failure}\\n' >&2\n  exit 1\nfi\nif [ \"$cpu\" != '1' ]; then\n  printf 'error: second attempt still used gpu backend\\n' >&2\n  exit 3\nfi\nprintf 'cpu summary'\n",
            count = count.display(),
            log = log.display(),
            failure = failure_marker,
        ),
        &format!(
            "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nset \"count=0\"\nif exist \"{count}\" set /p count=<\"{count}\"\nset /a count+=1\n> \"{count}\" <nul set /p =!count!\nset \"cpu=0\"\n>> \"{log}\" echo CALL=!count!\n>> \"{log}\" echo GGML_METAL=%GGML_METAL%\n>> \"{log}\" echo GGML_METAL_DEVICES=%GGML_METAL_DEVICES%\n:loop\nif \"%~1\"==\"\" goto after\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo CALL_!count!_ARG=!arg!\nif /I \"!arg!\"==\"-ngl\" set \"cpu=1\"\nif /I \"!arg!\"==\"--device\" set \"cpu=1\"\nif /I \"!arg!\"==\"none\" set \"cpu=1\"\nif /I \"!arg!\"==\"--no-op-offload\" set \"cpu=1\"\nif /I \"!arg!\"==\"--no-kv-offload\" set \"cpu=1\"\nif /I \"!arg!\"==\"--no-mmproj-offload\" set \"cpu=1\"\nshift\ngoto loop\n:after\nif \"!count!\"==\"1\" (\n  if \"!cpu!\"==\"1\" (\n    echo unexpected cpu fallback on first attempt 1>&2\n    exit /b 2\n  )\n  echo error: {failure} 1>&2\n  exit /b 1\n)\nif not \"!cpu!\"==\"1\" (\n  echo error: second attempt still used gpu backend 1>&2\n  exit /b 3\n)\n<nul set /p =cpu summary\nexit /b 0\n",
            count = count.display(),
            log = log.display(),
            failure = failure_marker,
        ),
    );

    let toolchain = Toolchain {
        llama_cli_path: fake_llama_cli_path(&build_bin),
        llama_embedding_path: crate::ffmpeg::fake_command_path(&build_bin, "llama-embedding"),
        build_script_path: build_script_path(&repo_root, "llama"),
        model_source: ModelSource::LocalPath(model_path.clone()),
        cached_model_path: None,
        embedding_model_source: ModelSource::LocalPath(model_path),
        embedding_cached_model_path: None,
    };

    run_summary_generation(&toolchain, &prompt_file, &output_file).expect("summary");

    assert_eq!(
        fs::read_to_string(output_file).expect("summary output"),
        "cpu summary"
    );

    let log = fs::read_to_string(log).expect("retry log");
    assert!(log.contains("CALL=1"));
    assert!(log.contains("CALL=2"));
    assert!(!log.contains("CALL_1_ARG=-ngl"));
    assert!(log.contains("CALL_2_ARG=-ngl"));
    assert!(log.contains("CALL_2_ARG=0"));
    assert!(log.contains("CALL_2_ARG=--device"));
    assert!(log.contains("CALL_2_ARG=none"));
    assert!(log.contains("CALL_2_ARG=--no-op-offload"));
    assert!(log.contains("CALL_2_ARG=--no-kv-offload"));
    assert!(log.contains("CALL_2_ARG=--no-mmproj-offload"));
    if cfg!(target_os = "macos") {
        assert!(log.contains("GGML_METAL=0"));
        assert!(log.contains("GGML_METAL_DEVICES=0"));
    }
}

#[test]
fn downloads_missing_hugging_face_model_to_repo_cache() {
    let _guard = crate::test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe { std::env::remove_var(MODEL_ENV_VAR) };

    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let llama_log = repo_root.join("llama-download.log");
    fs::create_dir_all(&build_bin).expect("build bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    write_build_script(&build_script_path(&repo_root, "llama"));
    write_executable(
        &fake_llama_cli_path(&build_bin),
        &format!(
            "#!/bin/sh\n: > '{log}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\ndone\nprintf 'LLAMA_CACHE=%s\\n' \"${{LLAMA_CACHE:-}}\" >> '{log}'\nmkdir -p \"$LLAMA_CACHE\"\nprintf 'synthetic model' > \"$LLAMA_CACHE/downloaded-model.gguf\"\n",
            log = llama_log.display()
        ),
        &format!(
            "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\n> \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto after\n>> \"{log}\" echo %~1\nshift\ngoto loop\n:after\n>> \"{log}\" echo LLAMA_CACHE=!LLAMA_CACHE!\nif not exist \"!LLAMA_CACHE!\" mkdir \"!LLAMA_CACHE!\"\n> \"!LLAMA_CACHE!\\downloaded-model.gguf\" <nul set /p =synthetic model\nexit /b 0\n",
            log = llama_log.display()
        ),
    );

    let toolchain = Toolchain::discover(&repo_root).expect("toolchain");
    toolchain.ensure_model().expect("model download");

    let cache_path = toolchain
        .cached_model_path
        .clone()
        .expect("cached model path");
    assert!(cache_path.is_file());
    assert_eq!(
        toolchain.runtime_model_source(),
        ModelSource::LocalPath(cache_path.clone())
    );

    let log = fs::read_to_string(llama_log).expect("llama log");
    assert!(log.contains("-hf"));
    assert!(log.contains(DEFAULT_MODEL_REPOSITORY));
    assert!(log.contains("LLAMA_CACHE="));
}

#[test]
fn downloads_hugging_face_model_from_nested_cache_layout() {
    let _guard = crate::test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe { std::env::remove_var(MODEL_ENV_VAR) };

    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let llama_log = repo_root.join("llama-nested-download.log");
    fs::create_dir_all(&build_bin).expect("build bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    write_build_script(&build_script_path(&repo_root, "llama"));
    write_executable(
        &fake_llama_cli_path(&build_bin),
        &format!(
            "#!/bin/sh\n: > '{log}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\ndone\nprintf 'LLAMA_CACHE=%s\\n' \"${{LLAMA_CACHE:-}}\" >> '{log}'\nnested=\"$LLAMA_CACHE/models--ggml-org--gemma-3-4b-it-GGUF/snapshots/main\"\nmkdir -p \"$nested\"\nprintf 'synthetic nested model' > \"$nested/model.gguf\"\n",
            log = llama_log.display()
        ),
        &format!(
            "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\n> \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto after\n>> \"{log}\" echo %~1\nshift\ngoto loop\n:after\n>> \"{log}\" echo LLAMA_CACHE=!LLAMA_CACHE!\nset \"nested=!LLAMA_CACHE!\\models--ggml-org--gemma-3-4b-it-GGUF\\snapshots\\main\"\nif not exist \"!nested!\" mkdir \"!nested!\"\n> \"!nested!\\model.gguf\" <nul set /p =synthetic nested model\nexit /b 0\n",
            log = llama_log.display()
        ),
    );

    let toolchain = Toolchain::discover(&repo_root).expect("toolchain");
    toolchain.ensure_model().expect("nested model download");

    let cache_path = toolchain
        .cached_model_path
        .clone()
        .expect("cached model path");
    assert!(cache_path.is_file());
    assert_eq!(
        fs::read_to_string(cache_path).expect("cached model"),
        "synthetic nested model"
    );
}

#[test]
fn downloads_embedding_model_from_hf_home_cache_layout() {
    let _guard = crate::test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        std::env::remove_var(MODEL_ENV_VAR);
        std::env::remove_var(EMBEDDING_MODEL_ENV_VAR);
    }

    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let hf_home = repo_root.join("hf-home");
    let llama_log = repo_root.join("llama-hf-home-download.log");
    fs::create_dir_all(&build_bin).expect("build bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    write_build_script(&build_script_path(&repo_root, "llama"));

    unsafe { std::env::set_var("HF_HOME", &hf_home) };
    write_executable(
        &fake_llama_cli_path(&build_bin),
        &format!(
            "#!/bin/sh\n: > '{log}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\ndone\nprintf 'HF_HOME=%s\\n' \"${{HF_HOME:-}}\" >> '{log}'\nhub=\"$HF_HOME/hub/models--Qwen--Qwen3-Embedding-4B-GGUF/snapshots/main\"\nmkdir -p \"$hub\"\nprintf 'synthetic hf home model' > \"$hub/Qwen3-Embedding-4B-Q4_K_M.gguf\"\n",
            log = llama_log.display()
        ),
        &format!(
            "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\n> \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto after\n>> \"{log}\" echo %~1\nshift\ngoto loop\n:after\nset \"hf_home=!HF_HOME!\"\nif \"!hf_home:~0,4!\"==\"\\\\?\\\" set \"hf_home=!hf_home:~4!\"\n>> \"{log}\" echo HF_HOME=!hf_home!\nset \"hub=!hf_home!\\hub\\models--Qwen--Qwen3-Embedding-4B-GGUF\\snapshots\\main\"\nif not exist \"!hub!\" mkdir \"!hub!\"\n> \"!hub!\\Qwen3-Embedding-4B-Q4_K_M.gguf\" <nul set /p =synthetic hf home model\nexit /b 0\n",
            log = llama_log.display()
        ),
    );

    let toolchain = Toolchain::discover(&repo_root).expect("toolchain");
    let download_result = toolchain.ensure_embedding_model();
    unsafe { std::env::remove_var("HF_HOME") };
    download_result.expect("embedding model download");

    let cache_path = toolchain
        .embedding_cached_model_path
        .clone()
        .expect("embedding cached model path");
    assert!(cache_path.is_file());
    assert_eq!(
        fs::read_to_string(cache_path).expect("cached embedding model"),
        "synthetic hf home model"
    );

    let log = fs::read_to_string(llama_log).expect("llama log");
    assert!(log.contains(DEFAULT_EMBEDDING_MODEL_REPOSITORY));
    assert!(log.contains("/exit"));
    assert!(log.contains("HF_HOME="));
}

#[test]
fn reports_missing_llama_toolchain() {
    let repo_root = temp_workspace();
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");

    let error = Toolchain::discover(&repo_root).expect_err("toolchain should fail");

    assert!(
        error.contains(
            build_script_path(&repo_root, "llama")
                .to_string_lossy()
                .as_ref()
        )
    );
}

#[test]
fn extracts_summary_text_from_cli_stdout_with_truncated_prompt_echo() {
    let stdout = "Loading model...\n\n> 당신은 회의 녹취를 정리하는 한국어 회의록 작성 도우미다.\n- 출력은 반드시 한국어 Markdown으로 작� ... (truncated)\n\n## 회의록\n\n**개요**\n내용\n\n**핵심 논의**\n항목\n\n[ Prompt: 10.0 t/s | Generation: 20.0 t/s ]\n\nExiting...\n";

    let extracted = extract_summary_text(stdout, "길어서 일치하지 않는 원본 프롬프트");

    assert_eq!(extracted, "## 개요\n내용\n\n## 핵심 논의\n항목");
}

#[test]
fn extracts_markdown_summary_without_meeting_title() {
    let stdout = "Assistant preface\n\n## 개요\n내용\n\n## 핵심 논의\n항목";

    let extracted = extract_summary_text(stdout, "");

    assert_eq!(extracted, "## 개요\n내용\n\n## 핵심 논의\n항목");
}

fn temp_workspace() -> PathBuf {
    let path = std::env::temp_dir().join(format!("recordroute-llama-{}", Uuid::now_v7()));
    fs::create_dir_all(&path).expect("temp workspace");
    path
}

fn fake_llama_cli_path(build_bin: &Path) -> PathBuf {
    crate::ffmpeg::fake_command_path(build_bin, "llama-cli")
}

fn write_build_script(path: &Path) {
    write_executable(path, "#!/bin/sh\nexit 0\n", "@echo off\nexit /b 0\n");
}

fn write_executable(path: &Path, unix_content: &str, windows_content: &str) {
    let content = if cfg!(windows) {
        windows_content.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        unix_content.to_string()
    };
    fs::write(path, content).expect("write executable");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("permissions");
    }
}
