use super::super::cli::CliCommand;
use super::super::*;
use super::support::*;
use crate::test_support::env_lock;
use std::ffi::OsString;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use uuid::Uuid;
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

    let path = resolve_input_path(&args, &mut reader, &mut output).expect("prompt should parse");

    assert_eq!(path, PathBuf::from("/tmp/from-prompt.wav"));
    assert_eq!(
        String::from_utf8(output).expect("utf8"),
        "Input audio path: "
    );
}

#[test]
fn strips_wrapping_quotes_from_prompt_input() {
    let args = Vec::<OsString>::new();
    let mut reader = Cursor::new(b"'/tmp/from-prompt.wav'\n".to_vec());
    let mut output = Vec::new();

    let path = resolve_input_path(&args, &mut reader, &mut output).expect("prompt should parse");

    assert_eq!(path, PathBuf::from("/tmp/from-prompt.wav"));
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
fn strips_wrapping_quotes_from_single_path_argument() {
    let args = vec![OsString::from("\"/tmp/input.mp3\"")];
    let mut reader = Cursor::new(Vec::<u8>::new());
    let mut output = Vec::new();

    let path = resolve_input_path(&args, &mut reader, &mut output).expect("path should parse");

    assert_eq!(path, PathBuf::from("/tmp/input.mp3"));
    assert!(output.is_empty());
}

#[test]
fn resolves_legacy_input_as_ffmpeg_command() {
    let args = vec![OsString::from("/tmp/input.wav")];
    let mut reader = Cursor::new(Vec::<u8>::new());
    let mut output = Vec::new();

    let command = resolve_cli_command(&args, &mut reader, &mut output).expect("command");

    assert_eq!(
        command,
        CliCommand::Ffmpeg {
            input: Some(PathBuf::from("/tmp/input.wav"))
        }
    );
}

#[test]
fn prompts_for_mode_when_no_args() {
    let mut reader = Cursor::new(b"2\n".to_vec());
    let mut output = Vec::new();

    let command = resolve_cli_command(&[], &mut reader, &mut output).expect("command");

    assert_eq!(command, CliCommand::Stt);
    assert!(
        String::from_utf8(output)
            .expect("utf8")
            .contains("Select mode:")
    );
}

#[test]
fn retries_invalid_mode_selection() {
    let mut reader = Cursor::new(b"9\n1\n".to_vec());
    let mut output = Vec::new();

    let command = resolve_cli_command(&[], &mut reader, &mut output).expect("command");

    assert_eq!(command, CliCommand::Ffmpeg { input: None });
    assert!(
        String::from_utf8(output)
            .expect("utf8")
            .contains("Invalid selection. Enter 1, 2, 3, or 4.")
    );
}

#[test]
fn stt_command_rejects_extra_arguments() {
    let args = vec![OsString::from("stt"), OsString::from("extra")];
    let mut reader = Cursor::new(Vec::<u8>::new());
    let mut output = Vec::new();

    let error = resolve_cli_command(&args, &mut reader, &mut output).expect_err("should fail");

    assert_eq!(error, "stt mode does not accept additional arguments");
}

#[test]
fn resolves_summary_command() {
    let args = vec![OsString::from("summary")];
    let mut reader = Cursor::new(Vec::<u8>::new());
    let mut output = Vec::new();

    let command = resolve_cli_command(&args, &mut reader, &mut output).expect("command");

    assert_eq!(command, CliCommand::Summary);
}

#[test]
fn resolves_prepare_llama_model_command() {
    let args = vec![OsString::from("prepare-llama-model")];
    let mut reader = Cursor::new(Vec::<u8>::new());
    let mut output = Vec::new();

    let command = resolve_cli_command(&args, &mut reader, &mut output).expect("command");

    assert_eq!(command, CliCommand::PrepareLlamaModel);
}
#[test]
fn resolves_prepare_models_command() {
    let args = vec![OsString::from("prepare-models")];
    let mut reader = Cursor::new(Vec::<u8>::new());
    let mut output = Vec::new();

    let command = resolve_cli_command(&args, &mut reader, &mut output).expect("command");

    assert_eq!(command, CliCommand::PrepareModels);
}

#[test]
fn resolves_server_command() {
    let args = vec![OsString::from("server")];
    let mut reader = Cursor::new(Vec::<u8>::new());
    let mut output = Vec::new();

    let command = resolve_cli_command(&args, &mut reader, &mut output).expect("command");

    assert_eq!(command, CliCommand::Server);
}

#[test]
fn server_command_rejects_extra_arguments() {
    let args = vec![OsString::from("server"), OsString::from("extra")];
    let mut reader = Cursor::new(Vec::<u8>::new());
    let mut output = Vec::new();

    let error = resolve_cli_command(&args, &mut reader, &mut output).expect_err("should fail");

    assert_eq!(error, "server mode does not accept additional arguments");
}

#[test]
fn prompts_for_server_mode_when_selected() {
    let mut reader = Cursor::new(b"4\n".to_vec());
    let mut output = Vec::new();

    let command = resolve_cli_command(&[], &mut reader, &mut output).expect("command");

    assert_eq!(command, CliCommand::Server);
    assert!(
        String::from_utf8(output)
            .expect("utf8")
            .contains("4. server 작업")
    );
}

#[test]
fn summary_command_rejects_extra_arguments() {
    let args = vec![OsString::from("summary"), OsString::from("extra")];
    let mut reader = Cursor::new(Vec::<u8>::new());
    let mut output = Vec::new();

    let error = resolve_cli_command(&args, &mut reader, &mut output).expect_err("should fail");

    assert_eq!(error, "summary mode does not accept additional arguments");
}

#[test]
fn prepare_llama_model_command_rejects_extra_arguments() {
    let args = vec![
        OsString::from("prepare-llama-model"),
        OsString::from("extra"),
    ];
    let mut reader = Cursor::new(Vec::<u8>::new());
    let mut output = Vec::new();

    let error = resolve_cli_command(&args, &mut reader, &mut output).expect_err("should fail");

    assert_eq!(
        error,
        "prepare-llama-model mode does not accept additional arguments"
    );
}
#[test]
fn prepare_models_command_rejects_extra_arguments() {
    let args = vec![OsString::from("prepare-models"), OsString::from("extra")];
    let mut reader = Cursor::new(Vec::<u8>::new());
    let mut output = Vec::new();

    let error = resolve_cli_command(&args, &mut reader, &mut output).expect_err("should fail");

    assert_eq!(
        error,
        "prepare-models mode does not accept additional arguments"
    );
}

#[test]
fn prompts_for_summary_mode_when_selected() {
    let mut reader = Cursor::new(b"3\n".to_vec());
    let mut output = Vec::new();

    let command = resolve_cli_command(&[], &mut reader, &mut output).expect("command");

    assert_eq!(command, CliCommand::Summary);
    assert!(
        String::from_utf8(output)
            .expect("utf8")
            .contains("3. summary 작업")
    );
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
fn load_repo_env_reads_only_dot_env() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let repo_root = temp_workspace();

    unsafe {
        std::env::remove_var("RECORDROUTE_TEST_DOTENV_LOADED");
        std::env::remove_var("RECORDROUTE_TEST_DOTENV_IGNORED");
    }

    fs::write(
        repo_root.join(".env"),
        "RECORDROUTE_TEST_DOTENV_LOADED=from-dotenv\n",
    )
    .expect("env");
    fs::write(
        repo_root.join(".env.example"),
        "RECORDROUTE_TEST_DOTENV_IGNORED=from-example\n",
    )
    .expect("env example");

    load_repo_env(&repo_root).expect("dotenv load");

    assert_eq!(
        std::env::var("RECORDROUTE_TEST_DOTENV_LOADED").expect("loaded"),
        "from-dotenv"
    );
    assert!(std::env::var("RECORDROUTE_TEST_DOTENV_IGNORED").is_err());

    unsafe {
        std::env::remove_var("RECORDROUTE_TEST_DOTENV_LOADED");
        std::env::remove_var("RECORDROUTE_TEST_DOTENV_IGNORED");
    }
}
