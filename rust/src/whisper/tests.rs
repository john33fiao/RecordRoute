use super::*;
use crate::ffmpeg::build_script_path;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[test]
fn discovers_toolchain_with_default_model_path() {
    let _guard = crate::test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe { std::env::remove_var(MODEL_ENV_VAR) };
    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    fs::create_dir_all(&build_bin).expect("build bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    write_build_script(&build_script_path(&repo_root, "whisper"));
    write_executable(
        &fake_whisper_cli_path(&build_bin),
        "#!/bin/sh\nexit 0\n",
        "@echo off\nexit /b 0\n",
    );

    let toolchain = Toolchain::discover(&repo_root).expect("toolchain");

    assert_eq!(
        toolchain.model_path,
        repo_root.join("models/whisper/ggml-base.bin")
    );
    assert_eq!(
        toolchain.whisper_cli_path,
        fake_whisper_cli_path(&build_bin)
    );
}

#[test]
fn discover_uses_env_override_for_relative_model_path() {
    let _guard = crate::test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    fs::create_dir_all(&build_bin).expect("build bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    write_build_script(&build_script_path(&repo_root, "whisper"));
    write_executable(
        &fake_whisper_cli_path(&build_bin),
        "#!/bin/sh\nexit 0\n",
        "@echo off\nexit /b 0\n",
    );

    unsafe { std::env::set_var(MODEL_ENV_VAR, "custom/ggml-base.bin") };
    let toolchain = Toolchain::discover(&repo_root).expect("toolchain");
    unsafe { std::env::remove_var(MODEL_ENV_VAR) };

    assert_eq!(toolchain.model_path, repo_root.join("custom/ggml-base.bin"));
}

#[test]
fn discover_normalizes_shorthand_relative_model_path() {
    let _guard = crate::test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    fs::create_dir_all(&build_bin).expect("build bin");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    write_build_script(&build_script_path(&repo_root, "whisper"));
    write_executable(
        &fake_whisper_cli_path(&build_bin),
        "#!/bin/sh\nexit 0\n",
        "@echo off\nexit /b 0\n",
    );

    unsafe { std::env::set_var(MODEL_ENV_VAR, "models/whisper/large-v3-turbo") };
    let toolchain = Toolchain::discover(&repo_root).expect("toolchain");
    unsafe { std::env::remove_var(MODEL_ENV_VAR) };

    assert_eq!(
        toolchain.model_path,
        repo_root.join("models/whisper/ggml-large-v3-turbo.bin")
    );
}

#[test]
fn reports_missing_whisper_toolchain() {
    let repo_root = temp_workspace();
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");

    let error = Toolchain::discover(&repo_root).expect_err("toolchain should fail");

    assert!(
        error.contains(
            build_script_path(&repo_root, "whisper")
                .to_string_lossy()
                .as_ref()
        )
    );
}

#[test]
fn downloads_missing_model_to_configured_location() {
    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let source_dir = repo_root.join("source-models");
    fs::create_dir_all(&build_bin).expect("build bin");
    fs::create_dir_all(&source_dir).expect("models dir");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    write_build_script(&build_script_path(&repo_root, "whisper"));
    write_executable(
        &fake_whisper_cli_path(&build_bin),
        "#!/bin/sh\nexit 0\n",
        "@echo off\nexit /b 0\n",
    );
    fs::write(source_dir.join("ggml-base.bin"), "cached-model").expect("source model");

    unsafe { std::env::set_var(MODEL_SOURCE_DIR_ENV_VAR, &source_dir) };
    let toolchain = Toolchain::discover(&repo_root).expect("toolchain");
    toolchain.ensure_model().expect("model download");
    unsafe { std::env::remove_var(MODEL_SOURCE_DIR_ENV_VAR) };

    assert!(toolchain.model_path.is_file());
    assert_eq!(
        fs::read_to_string(&toolchain.model_path).expect("model content"),
        "cached-model"
    );
}

#[test]
fn run_transcription_creates_text_output() {
    let _guard = crate::test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let input = repo_root.join("sample.wav");
    let output = repo_root.join("stt/sample.txt");
    let log = repo_root.join("whisper.log");
    fs::create_dir_all(&build_bin).expect("build bin");
    write_test_audio(&input);
    write_executable(
        &fake_whisper_cli_path(&build_bin),
        &format!(
            "#!/bin/sh\nnext=''\nout=''\nlog='{}'\n: > \"$log\"\nprintf 'GGML_METAL=%s\\n' \"${{GGML_METAL-}}\" >> \"$log\"\nprintf 'GGML_METAL_DEVICES=%s\\n' \"${{GGML_METAL_DEVICES-}}\" >> \"$log\"\nfor arg in \"$@\"; do\n  printf 'ARG=%s\\n' \"$arg\" >> \"$log\"\n  if [ \"$next\" = 'of' ]; then\n    out=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -of)\n      next='of'\n      ;;\n  esac\ndone\nmkdir -p \"$(dirname \"$out\")\"\nprintf 'synthetic transcript' > \"$out.txt\"\n",
            log.display()
        ),
        &format!(
            "@echo off\r\nsetlocal EnableExtensions EnableDelayedExpansion\r\nset \"out=\"\r\nset \"next=\"\r\n> \"{log}\" echo GGML_METAL=%GGML_METAL%\r\n>> \"{log}\" echo GGML_METAL_DEVICES=%GGML_METAL_DEVICES%\r\n:loop\r\nif \"%~1\"==\"\" goto done\r\n>> \"{log}\" echo ARG=%~1\r\nif /I \"!next!\"==\"of\" (\r\n  set \"out=%~1\"\r\n  set \"next=\"\r\n) else if /I \"%~1\"==\"-of\" (\r\n  set \"next=of\"\r\n)\r\nshift\r\ngoto loop\r\n:done\r\nif defined out (\r\n  for %%I in (\"!out!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\r\n  > \"!out!.txt\" <nul set /p =synthetic transcript\r\n)\r\nexit /b 0\r\n",
            log = log.display()
        ),
    );

    let toolchain = Toolchain {
        whisper_cli_path: fake_whisper_cli_path(&build_bin),
        build_script_path: build_script_path(&repo_root, "whisper"),
        model_path: repo_root.join("models/whisper/ggml-base.bin"),
    };
    fs::create_dir_all(repo_root.join("models/whisper")).expect("models dir");
    fs::write(&toolchain.model_path, "model").expect("model file");

    run_transcription(&toolchain, &input, &output).expect("transcription");

    assert_eq!(
        fs::read_to_string(output).expect("transcript"),
        "synthetic transcript"
    );

    let log = fs::read_to_string(log).expect("whisper log");
    assert!(log.contains("ARG=-m"));
    assert!(log.contains("ARG=-l"));
    assert!(log.contains("ARG=auto"));
    assert!(!log.contains("ARG=-ng"));
    assert!(!log.contains("GGML_METAL=0"));
    assert!(!log.contains("GGML_METAL_DEVICES=0"));
}

#[test]
fn run_transcription_deduplicates_consecutive_transcript_lines() {
    let _guard = crate::test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let input = repo_root.join("sample.wav");
    let output = repo_root.join("stt/sample.txt");
    fs::create_dir_all(&build_bin).expect("build bin");
    write_test_audio(&input);
    write_executable(
        &fake_whisper_cli_path(&build_bin),
        "#!/bin/sh\nnext=''\nout=''\nfor arg in \"$@\"; do\n  if [ \"$next\" = 'of' ]; then\n    out=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -of)\n      next='of'\n      ;;\n  esac\ndone\nmkdir -p \"$(dirname \"$out\")\"\nprintf 'intro\\nrepeat me\\nrepeat me\\n\\nrepeat me\\noutro\\n' > \"$out.txt\"\n",
        "@echo off\r\nsetlocal EnableExtensions EnableDelayedExpansion\r\nset \"out=\"\r\nset \"next=\"\r\n:loop\r\nif \"%~1\"==\"\" goto done\r\nif /I \"!next!\"==\"of\" (\r\n  set \"out=%~1\"\r\n  set \"next=\"\r\n) else if /I \"%~1\"==\"-of\" (\r\n  set \"next=of\"\r\n)\r\nshift\r\ngoto loop\r\n:done\r\nif defined out (\r\n  for %%I in (\"!out!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\r\n  > \"!out!.txt\" echo intro\r\n  >> \"!out!.txt\" echo repeat me\r\n  >> \"!out!.txt\" echo repeat me\r\n  >> \"!out!.txt\" echo.\r\n  >> \"!out!.txt\" echo repeat me\r\n  >> \"!out!.txt\" echo outro\r\n)\r\nexit /b 0\r\n",
    );

    let toolchain = Toolchain {
        whisper_cli_path: fake_whisper_cli_path(&build_bin),
        build_script_path: build_script_path(&repo_root, "whisper"),
        model_path: repo_root.join("models/whisper/ggml-base.bin"),
    };
    fs::create_dir_all(repo_root.join("models/whisper")).expect("models dir");
    fs::write(&toolchain.model_path, "model").expect("model file");

    run_transcription(&toolchain, &input, &output).expect("transcription");

    assert_eq!(
        fs::read_to_string(output)
            .expect("transcript")
            .replace("\r\n", "\n"),
        "intro\nrepeat me\n\nrepeat me\noutro\n"
    );
}

#[cfg(any(target_os = "macos", windows))]
#[test]
fn run_transcription_retries_on_cpu_after_backend_failure() {
    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let input = repo_root.join("sample.wav");
    let output = repo_root.join("stt/sample.txt");
    let log = repo_root.join("whisper-retry.log");
    let count = repo_root.join("whisper-retry.count");
    let failure_marker = if cfg!(target_os = "macos") {
        "metal backend failure"
    } else {
        "cuda backend failure"
    };
    fs::create_dir_all(&build_bin).expect("build bin");
    write_test_audio(&input);
    write_executable(
        &fake_whisper_cli_path(&build_bin),
        &format!(
            "#!/bin/sh\ncount=0\nif [ -f '{count}' ]; then\n  count=$(cat '{count}')\nfi\ncount=$((count + 1))\nprintf '%s' \"$count\" > '{count}'\nnext=''\nout=''\ncpu='0'\nlog='{log}'\nprintf 'CALL=%s\\n' \"$count\" >> \"$log\"\nprintf 'GGML_METAL=%s\\n' \"${{GGML_METAL-}}\" >> \"$log\"\nprintf 'GGML_METAL_DEVICES=%s\\n' \"${{GGML_METAL_DEVICES-}}\" >> \"$log\"\nfor arg in \"$@\"; do\n  printf 'CALL_%s_ARG=%s\\n' \"$count\" \"$arg\" >> \"$log\"\n  if [ \"$next\" = 'of' ]; then\n    out=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -of)\n      next='of'\n      ;;\n    -ng)\n      cpu='1'\n      ;;\n  esac\ndone\nif [ \"$count\" = '1' ]; then\n  if [ \"$cpu\" = '1' ]; then\n    printf 'unexpected cpu fallback on first attempt\\n' >&2\n    exit 2\n  fi\n  printf 'error: {failure}\\n' >&2\n  exit 1\nfi\nif [ \"$cpu\" != '1' ]; then\n  printf 'error: second attempt still used gpu backend\\n' >&2\n  exit 3\nfi\nmkdir -p \"$(dirname \"$out\")\"\nprintf 'cpu fallback transcript' > \"$out.txt\"\n",
            count = count.display(),
            log = log.display(),
            failure = failure_marker,
        ),
        &format!(
            "@echo off\r\nsetlocal EnableExtensions EnableDelayedExpansion\r\nset \"count=0\"\r\nif exist \"{count}\" set /p count=<\"{count}\"\r\nset /a count+=1\r\n> \"{count}\" <nul set /p =!count!\r\nset \"out=\"\r\nset \"next=\"\r\nset \"cpu=0\"\r\n>> \"{log}\" echo CALL=!count!\r\n>> \"{log}\" echo GGML_METAL=%GGML_METAL%\r\n>> \"{log}\" echo GGML_METAL_DEVICES=%GGML_METAL_DEVICES%\r\n:loop\r\nif \"%~1\"==\"\" goto after\r\nset \"arg=%~1\"\r\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\r\n>> \"{log}\" echo CALL_!count!_ARG=!arg!\r\nif /I \"!next!\"==\"of\" (\r\n  set \"out=!arg!\"\r\n  set \"next=\"\r\n) else (\r\n  if /I \"!arg!\"==\"-of\" set \"next=of\"\r\n  if /I \"!arg!\"==\"-ng\" set \"cpu=1\"\r\n)\r\nshift\r\ngoto loop\r\n:after\r\nif \"!count!\"==\"1\" (\r\n  if \"!cpu!\"==\"1\" (\r\n    echo unexpected cpu fallback on first attempt 1>&2\r\n    exit /b 2\r\n  )\r\n  echo error: {failure} 1>&2\r\n  exit /b 1\r\n)\r\nif not \"!cpu!\"==\"1\" (\r\n  echo error: second attempt still used gpu backend 1>&2\r\n  exit /b 3\r\n)\r\nif defined out (\r\n  for %%I in (\"!out!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\r\n  > \"!out!.txt\" <nul set /p =cpu fallback transcript\r\n)\r\nexit /b 0\r\n",
            count = count.display(),
            log = log.display(),
            failure = failure_marker,
        ),
    );

    let toolchain = Toolchain {
        whisper_cli_path: fake_whisper_cli_path(&build_bin),
        build_script_path: build_script_path(&repo_root, "whisper"),
        model_path: repo_root.join("models/whisper/ggml-base.bin"),
    };
    fs::create_dir_all(repo_root.join("models/whisper")).expect("models dir");
    fs::write(&toolchain.model_path, "model").expect("model file");

    run_transcription(&toolchain, &input, &output).expect("transcription");

    assert_eq!(
        fs::read_to_string(output).expect("transcript"),
        "cpu fallback transcript"
    );

    let log = fs::read_to_string(log).expect("retry log");
    assert!(log.contains("CALL=1"));
    assert!(log.contains("CALL=2"));
    assert!(!log.contains("CALL_1_ARG=-ng"));
    assert!(log.contains("CALL_2_ARG=-ng"));
    if cfg!(target_os = "macos") {
        assert!(log.contains("GGML_METAL=0"));
        assert!(log.contains("GGML_METAL_DEVICES=0"));
    }
}

#[test]
fn run_transcription_refreshes_invalid_managed_model_once() {
    let _guard = crate::test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let repo_root = temp_workspace();
    let build_bin = repo_root
        .join(".build/whisper")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let input = repo_root.join("sample.wav");
    let output = repo_root.join("stt/sample.txt");
    let model_path = repo_root.join("models/whisper/ggml-base.bin");
    fs::create_dir_all(&build_bin).expect("build bin");
    let source_dir = repo_root.join("source-models");
    fs::create_dir_all(&source_dir).expect("download dir");
    fs::create_dir_all(repo_root.join("models/whisper")).expect("model dir");
    write_test_audio(&input);
    fs::write(&model_path, "broken").expect("broken model");
    fs::write(source_dir.join("ggml-base.bin"), "healthy").expect("healthy model");
    write_executable(
        &fake_whisper_cli_path(&build_bin),
        &format!(
            "#!/bin/sh\nnext=''\nout=''\nfor arg in \"$@\"; do\n  if [ \"$next\" = 'of' ]; then\n    out=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -of)\n      next='of'\n      ;;\n  esac\ndone\nif [ \"$(cat '{}')\" != 'healthy' ]; then\n  printf 'error: failed to initialize whisper context\\n' >&2\n  exit 1\nfi\nmkdir -p \"$(dirname \"$out\")\"\nprintf 'recovered transcript' > \"$out.txt\"\n",
            model_path.display()
        ),
        &format!(
            "@echo off\r\nsetlocal EnableExtensions EnableDelayedExpansion\r\nset \"out=\"\r\nset \"next=\"\r\nfor /f \"usebackq delims=\" %%A in (\"{model}\") do set \"model_state=%%A\"\r\nif /I not \"!model_state!\"==\"healthy\" (\r\necho error: failed to initialize whisper context 1>&2\r\nexit /b 1\r\n)\r\n:loop\r\nif \"%~1\"==\"\" goto done\r\nif /I \"!next!\"==\"of\" (\r\n  set \"out=%~1\"\r\n  set \"next=\"\r\n) else if /I \"%~1\"==\"-of\" (\r\n  set \"next=of\"\r\n)\r\nshift\r\ngoto loop\r\n:done\r\nif defined out (\r\n  for %%I in (\"!out!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\r\n  > \"!out!.txt\" <nul set /p =recovered transcript\r\n)\r\nexit /b 0\r\n",
            model = model_path.display()
        ),
    );
    let toolchain = Toolchain {
        whisper_cli_path: fake_whisper_cli_path(&build_bin),
        build_script_path: build_script_path(&repo_root, "whisper"),
        model_path,
    };

    unsafe { std::env::set_var(MODEL_SOURCE_DIR_ENV_VAR, &source_dir) };
    run_transcription(&toolchain, &input, &output).expect("transcription");
    unsafe { std::env::remove_var(MODEL_SOURCE_DIR_ENV_VAR) };

    assert_eq!(
        fs::read_to_string(output).expect("transcript"),
        "recovered transcript"
    );
    assert_eq!(
        fs::read_to_string(&toolchain.model_path).expect("refreshed model"),
        "healthy"
    );
}

fn temp_workspace() -> PathBuf {
    let path = std::env::temp_dir().join(format!("recordroute-whisper-{}", Uuid::now_v7()));
    fs::create_dir_all(&path).expect("temp workspace");
    path
}

fn fake_whisper_cli_path(build_bin: &Path) -> PathBuf {
    crate::ffmpeg::fake_command_path(build_bin, "whisper-cli")
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
    fs::write(path, content).expect("script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("permissions");
    }
}

fn write_test_audio(path: &Path) {
    fs::write(path, b"RIFFsyntheticWAVE").expect("audio file");
}
