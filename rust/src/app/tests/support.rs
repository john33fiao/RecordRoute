use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;
pub(super) fn temp_workspace() -> PathBuf {
    let path = std::env::temp_dir().join(format!("recordroute-{}", Uuid::now_v7()));
    fs::create_dir_all(&path).expect("temp workspace");
    path
}

pub(super) fn build_script_path(repo_root: &Path, tool: &str) -> PathBuf {
    crate::ffmpeg::build_script_path(repo_root, tool)
}

pub(super) fn fake_command_path(base_dir: &Path, name: &str) -> PathBuf {
    crate::ffmpeg::fake_command_path(base_dir, name)
}

pub(super) fn whisper_download_script_path(repo_root: &Path) -> PathBuf {
    if cfg!(windows) {
        repo_root.join("modules/whisper.cpp/models/download-ggml-model.cmd")
    } else {
        repo_root.join("modules/whisper.cpp/models/download-ggml-model.sh")
    }
}

pub(super) fn write_build_script(path: &Path) {
    write_platform_script(path, "#!/bin/sh\nexit 0\n", "@echo off\nexit /b 0\n");
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

pub(super) fn write_fake_ffmpeg(path: &Path, log_path: &Path) {
    let log = log_path.display();
    let unix_script = format!(
        "#!/bin/sh\n: > '{log}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\n  case \"$arg\" in\n    *.wav)\n      mkdir -p \"$(dirname \"$arg\")\"\n      : > \"$arg\"\n      ;;\n  esac\ndone\n"
    );
    let windows_script = format!(
        "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\n> \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo(!arg!\nfor %%I in (\"!arg!\") do (\n  if /I \"%%~xI\"==\".wav\" (\n    if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n    > \"%%~fI\" type nul\n  )\n)\nshift\ngoto loop\n:done\nexit /b 0\n"
    );
    write_platform_script(path, &unix_script, &windows_script);
}

pub(super) fn write_counting_ffmpeg(path: &Path, log_path: &Path, count_path: &Path) {
    let log = log_path.display();
    let count = count_path.display();
    let unix_script = format!(
        "#!/bin/sh\ncount=0\nif [ -f '{count}' ]; then\n  count=$(cat '{count}')\nfi\ncount=$((count + 1))\nprintf '%s' \"$count\" > '{count}'\n: > '{log}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\n  case \"$arg\" in\n    *.wav)\n      mkdir -p \"$(dirname \"$arg\")\"\n      : > \"$arg\"\n      ;;\n  esac\ndone\n"
    );
    let windows_script = format!(
        "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nset /a count=0\nif exist \"{count}\" set /p count=<\"{count}\"\nset /a count+=1\n> \"{count}\" <nul set /p =!count!\n> \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo(!arg!\nfor %%I in (\"!arg!\") do (\n  if /I \"%%~xI\"==\".wav\" (\n    if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n    > \"%%~fI\" type nul\n  )\n)\nshift\ngoto loop\n:done\nexit /b 0\n"
    );
    write_platform_script(path, &unix_script, &windows_script);
}

pub(super) fn write_failing_ffmpeg(path: &Path) {
    let unix_script = "#!/bin/sh\nlast=''\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    *.wav)\n      last=\"$arg\"\n      ;;\n  esac\ndone\nif [ -n \"$last\" ]; then\n  mkdir -p \"$(dirname \"$last\")\"\n  : > \"$last\"\nfi\nprintf 'synthetic ffmpeg failure' >&2\nexit 1\n";
    let windows_script = "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nset \"last=\"\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\nfor %%I in (\"!arg!\") do if /I \"%%~xI\"==\".wav\" set \"last=%%~fI\"\nshift\ngoto loop\n:done\nif defined last (\n  for %%I in (\"!last!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n  > \"!last!\" type nul\n)\necho synthetic ffmpeg failure 1>&2\nexit /b 1\n";
    write_platform_script(path, unix_script, windows_script);
}

pub(super) fn write_fake_whisper_cli(path: &Path, log_path: &Path, fail: bool) {
    let log = log_path.display();
    let unix_script = if fail {
        format!(
            "#!/bin/sh\ntouch '{log}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\ndone\nprintf 'synthetic whisper failure' >&2\nexit 1\n"
        )
    } else {
        format!(
            "#!/bin/sh\ntouch '{log}'\nout=''\nnext=''\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\n  if [ \"$next\" = 'of' ]; then\n    out=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -of)\n      next='of'\n      ;;\n  esac\ndone\nmkdir -p \"$(dirname \"$out\")\"\nprintf 'synthetic transcript' > \"$out.txt\"\n"
        )
    };
    let windows_script = if fail {
        format!(
            "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nif not exist \"{log}\" > \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo(!arg!\nshift\ngoto loop\n:done\necho synthetic whisper failure 1>&2\nexit /b 1\n"
        )
    } else {
        format!(
            "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nif not exist \"{log}\" > \"{log}\" type nul\nset \"out=\"\nset \"next=\"\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo(!arg!\nif /I \"!next!\"==\"of\" (\n  set \"out=!arg!\"\n  set \"next=\"\n) else if /I \"!arg!\"==\"-of\" (\n  set \"next=of\"\n)\nshift\ngoto loop\n:done\nif defined out (\n  for %%I in (\"!out!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n  > \"!out!.txt\" <nul set /p =synthetic transcript\n)\nexit /b 0\n"
        )
    };
    write_platform_script(path, &unix_script, &windows_script);
}

pub(super) fn write_fake_llama_cli(
    path: &Path,
    log_path: &Path,
    prompt_capture_path: &Path,
    fail: bool,
) {
    let log = log_path.display();
    let prompt_capture = prompt_capture_path.display();
    let unix_script = if fail {
        format!(
            "#!/bin/sh\ntouch '{log}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\ndone\nprintf 'HF_TOKEN=%s\\n' \"${{HF_TOKEN:-}}\" >> '{log}'\nprintf 'LLAMA_CACHE=%s\\n' \"${{LLAMA_CACHE:-}}\" >> '{log}'\nprintf 'synthetic llama failure' >&2\nexit 1\n"
        )
    } else {
        format!(
            "#!/bin/sh\ntouch '{log}'\nprompt=''\nmodel=''\nhf_repo=''\nnext=''\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> '{log}'\n  if [ \"$next\" = 'f' ]; then\n    prompt=\"$arg\"\n    next=''\n    continue\n  fi\n  if [ \"$next\" = 'm' ]; then\n    model=\"$arg\"\n    next=''\n    continue\n  fi\n  if [ \"$next\" = 'hf' ]; then\n    hf_repo=\"$arg\"\n    next=''\n    continue\n  fi\n  case \"$arg\" in\n    -f)\n      next='f'\n      ;;\n    -m)\n      next='m'\n      ;;\n    -hf)\n      next='hf'\n      ;;\n  esac\ndone\nprintf 'HF_TOKEN=%s\\n' \"${{HF_TOKEN:-}}\" >> '{log}'\nprintf 'LLAMA_CACHE=%s\\n' \"${{LLAMA_CACHE:-}}\" >> '{log}'\nif [ -n \"$prompt\" ]; then\n  cat \"$prompt\" > '{prompt_capture}'\nfi\nif [ -n \"$model\" ]; then\n  mkdir -p \"$(dirname \"$model\")\"\n  printf 'synthetic model' > \"$model\"\nelif [ -n \"$hf_repo\" ] && [ -n \"${{LLAMA_CACHE:-}}\" ]; then\n  mkdir -p \"$LLAMA_CACHE\"\n  printf 'synthetic downloaded model' > \"$LLAMA_CACHE/downloaded-model.gguf\"\nfi\nif [ -n \"$prompt\" ]; then\n  printf 'synthetic summary'\nfi\n"
        )
    };
    let windows_script = if fail {
        format!(
            "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nif not exist \"{log}\" > \"{log}\" type nul\n:loop\nif \"%~1\"==\"\" goto done\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo(!arg!\nshift\ngoto loop\n:done\nset \"llama_cache=!LLAMA_CACHE!\"\nif \"!llama_cache:~0,4!\"==\"\\\\?\\\" set \"llama_cache=!llama_cache:~4!\"\n>> \"{log}\" echo HF_TOKEN=!HF_TOKEN!\n>> \"{log}\" echo LLAMA_CACHE=!llama_cache!\necho synthetic llama failure 1>&2\nexit /b 1\n"
        )
    } else {
        format!(
            "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nif not exist \"{log}\" > \"{log}\" type nul\nset \"prompt=\"\nset \"model=\"\nset \"hf_repo=\"\nset \"next=\"\n:loop\nif \"%~1\"==\"\" goto after\nset \"arg=%~1\"\nif \"!arg:~0,4!\"==\"\\\\?\\\" set \"arg=!arg:~4!\"\n>> \"{log}\" echo(!arg!\nif /I \"!next!\"==\"f\" (\n  set \"prompt=!arg!\"\n  set \"next=\"\n) else if /I \"!next!\"==\"m\" (\n  set \"model=!arg!\"\n  set \"next=\"\n) else if /I \"!next!\"==\"hf\" (\n  set \"hf_repo=!arg!\"\n  set \"next=\"\n) else if /I \"!arg!\"==\"-f\" (\n  set \"next=f\"\n) else if /I \"!arg!\"==\"-m\" (\n  set \"next=m\"\n) else if /I \"!arg!\"==\"-hf\" (\n  set \"next=hf\"\n)\nshift\ngoto loop\n:after\nset \"llama_cache=!LLAMA_CACHE!\"\nif \"!llama_cache:~0,4!\"==\"\\\\?\\\" set \"llama_cache=!llama_cache:~4!\"\n>> \"{log}\" echo HF_TOKEN=!HF_TOKEN!\n>> \"{log}\" echo LLAMA_CACHE=!llama_cache!\nif defined prompt copy /y \"!prompt!\" \"{prompt_capture}\" >nul\nif defined model (\n  for %%I in (\"!model!\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\n  > \"!model!\" <nul set /p =synthetic model\n) else if defined hf_repo if defined llama_cache (\n  if not exist \"!llama_cache!\" mkdir \"!llama_cache!\"\n  > \"!llama_cache!\\downloaded-model.gguf\" <nul set /p =synthetic downloaded model\n)\nif defined prompt <nul set /p =synthetic summary\nexit /b 0\n"
        )
    };
    write_platform_script(path, &unix_script, &windows_script);
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

pub(super) fn write_blocking_download_script(path: &Path, count_path: &Path, gate_path: &Path) {
    let count = count_path.display();
    let gate = gate_path.display();
    let unix_script = format!(
        "#!/bin/sh\ncount=0\nif [ -f '{count}' ]; then\n  count=$(cat '{count}')\nfi\ncount=$((count + 1))\nprintf '%s' \"$count\" > '{count}'\nmkdir -p \"$2\"\nwhile [ -f '{gate}' ]; do\n  sleep 0.05\ndone\n: > \"$2/ggml-$1.bin\"\n"
    );
    let windows_script = format!(
        "@echo off\nsetlocal EnableExtensions EnableDelayedExpansion\nset /a count=0\nif exist \"{count}\" set /p count=<\"{count}\"\nset /a count+=1\n> \"{count}\" <nul set /p =!count!\nset \"out_dir=%~2\"\nif \"!out_dir:~0,4!\"==\"\\\\?\\\" set \"out_dir=!out_dir:~4!\"\n:wait\nif exist \"{gate}\" (\n  powershell -NoProfile -Command \"Start-Sleep -Milliseconds 50\" >nul 2>&1\n  goto wait\n)\nif not exist \"!out_dir!\" mkdir \"!out_dir!\"\n> \"!out_dir!\\ggml-%~1.bin\" type nul\nexit /b 0\n"
    );
    write_platform_script(path, &unix_script, &windows_script);
}

pub(super) fn wait_for_path(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(25));
    }
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

pub(super) fn make_executable(_path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(_path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(_path, permissions).expect("permissions");
    }
}

pub(super) fn read_run_count(path: &Path) -> u32 {
    fs::read_to_string(path)
        .expect("count file")
        .trim()
        .parse()
        .expect("count should parse")
}
