use super::{
    SttRunSummary, SummaryRunSummary, backfill_summary_embeddings, load_repo_env, read_line,
    repo_root, run_stt_with_repo_root, run_summary_with_repo_root, run_with_repo_root,
    search_summaries,
};
use crate::server;
use std::ffi::{OsStr, OsString};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

const CLI_COMMAND_SPECS: &[CliCommandSpec] = &[
    CliCommandSpec::new(CliCommandKind::Ffmpeg, "ffmpeg", "ffmpeg 작업"),
    CliCommandSpec::new(CliCommandKind::Stt, "stt", "stt 작업"),
    CliCommandSpec::new(CliCommandKind::Summary, "summary", "summary 작업"),
    CliCommandSpec::new(
        CliCommandKind::PrepareModels,
        "prepare-models",
        "prepare-models 작업",
    ),
    CliCommandSpec::new(
        CliCommandKind::PrepareLlamaModel,
        "prepare-llama-model",
        "prepare-llama-model 작업",
    ),
    CliCommandSpec::new(
        CliCommandKind::EmbedSummaries,
        "embed-summaries",
        "embed-summaries 작업",
    ),
    CliCommandSpec::new(
        CliCommandKind::SearchSummaries,
        "search-summaries",
        "search-summaries 작업",
    ),
    CliCommandSpec::new(CliCommandKind::Server, "server", "server 작업"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CliCommand {
    Ffmpeg { input: Option<PathBuf> },
    Stt,
    Summary,
    PrepareModels,
    PrepareLlamaModel,
    EmbedSummaries,
    SearchSummaries { query: String },
    Server,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliCommandKind {
    Ffmpeg,
    Stt,
    Summary,
    PrepareModels,
    PrepareLlamaModel,
    EmbedSummaries,
    SearchSummaries,
    Server,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CliCommandSpec {
    kind: CliCommandKind,
    name: &'static str,
    menu_label: &'static str,
}

impl CliCommandSpec {
    const fn new(kind: CliCommandKind, name: &'static str, menu_label: &'static str) -> Self {
        Self {
            kind,
            name,
            menu_label,
        }
    }
}

impl CliCommandKind {
    fn parse_args(self, args: &[OsString]) -> Result<CliCommand, String> {
        match self {
            Self::Ffmpeg => match args {
                [_, input] => Ok(CliCommand::Ffmpeg {
                    input: Some(PathBuf::from(input)),
                }),
                [_] => Ok(CliCommand::Ffmpeg { input: None }),
                _ => Err("expected zero, one, or two arguments".to_string()),
            },
            Self::Stt => parse_argless_command(args, CliCommand::Stt, "stt"),
            Self::Summary => parse_argless_command(args, CliCommand::Summary, "summary"),
            Self::PrepareModels => {
                parse_argless_command(args, CliCommand::PrepareModels, "prepare-models")
            }
            Self::PrepareLlamaModel => {
                parse_argless_command(args, CliCommand::PrepareLlamaModel, "prepare-llama-model")
            }
            Self::EmbedSummaries => {
                parse_argless_command(args, CliCommand::EmbedSummaries, "embed-summaries")
            }
            Self::SearchSummaries => parse_search_summaries_args(args),
            Self::Server => parse_argless_command(args, CliCommand::Server, "server"),
        }
    }

    fn resolve_interactive(
        self,
        reader: &mut dyn BufRead,
        writer: &mut dyn Write,
    ) -> Result<CliCommand, String> {
        match self {
            Self::Ffmpeg => Ok(CliCommand::Ffmpeg { input: None }),
            Self::Stt => Ok(CliCommand::Stt),
            Self::Summary => Ok(CliCommand::Summary),
            Self::PrepareModels => Ok(CliCommand::PrepareModels),
            Self::PrepareLlamaModel => Ok(CliCommand::PrepareLlamaModel),
            Self::EmbedSummaries => Ok(CliCommand::EmbedSummaries),
            Self::SearchSummaries => {
                write!(writer, "Search query: ").map_err(|error| error.to_string())?;
                writer.flush().map_err(|error| error.to_string())?;

                let line = read_line(reader, "failed to read search query")?;
                Ok(CliCommand::SearchSummaries {
                    query: line.trim().to_string(),
                })
            }
            Self::Server => Ok(CliCommand::Server),
        }
    }
}

pub fn main_cli() -> Result<(), String> {
    let repo_root = repo_root()?;
    load_repo_env(&repo_root)?;
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    match resolve_cli_command(&args, &mut reader, &mut writer)? {
        CliCommand::Ffmpeg { input } => {
            let input = match input {
                Some(input) => input,
                None => resolve_input_path(&[], &mut reader, &mut writer)?,
            };
            let summary = run_with_repo_root(&repo_root, &input)?;
            print_run_summary(&summary, &mut writer)?;
        }
        CliCommand::Stt => {
            let summary = run_stt_with_repo_root(&repo_root, &mut reader, &mut writer)?;
            print_stt_summary(&summary, &mut writer)?;
        }
        CliCommand::Summary => {
            let summary = run_summary_with_repo_root(&repo_root, &mut reader, &mut writer)?;
            print_summary_run_summary(&summary, &mut writer)?;
        }
        CliCommand::PrepareModels => {
            super::prepare_models_with_repo_root(&repo_root)?;
        }
        CliCommand::PrepareLlamaModel => {
            super::prepare_llama_model_with_repo_root(&repo_root)?;
        }
        CliCommand::EmbedSummaries => {
            let rows = backfill_summary_embeddings(&repo_root)?;
            for (job_id, rebuilt) in rows {
                writeln!(
                    writer,
                    "{job_id}: {}",
                    if rebuilt { "rebuilt" } else { "skipped" }
                )
                .map_err(|error| error.to_string())?;
            }
        }
        CliCommand::SearchSummaries { query } => {
            let rows = search_summaries(&repo_root, &query, 10, None)
                .map_err(|error| error.to_string())?;
            for row in rows {
                writeln!(
                    writer,
                    "{} {:.4} {}",
                    row.job_id, row.score, row.summary_excerpt
                )
                .map_err(|error| error.to_string())?;
            }
        }
        CliCommand::Server => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|error| format!("failed to create tokio runtime: {error}"))?;
            runtime.block_on(server::serve())?;
        }
    }

    Ok(())
}

pub(crate) fn resolve_cli_command(
    args: &[OsString],
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
) -> Result<CliCommand, String> {
    match args {
        [] => prompt_for_mode(reader, writer),
        [mode, ..] => {
            if let Some(spec) = find_command_spec(mode) {
                spec.kind.parse_args(args)
            } else if let [input] = args {
                Ok(CliCommand::Ffmpeg {
                    input: Some(PathBuf::from(input)),
                })
            } else {
                Err("expected zero, one, or two arguments".to_string())
            }
        }
    }
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

            let line = read_line(reader, "failed to read input path")?;
            parse_input_path_value(line.trim())
        }
        [path] => parse_input_path_os(path),
        _ => Err("expected exactly one input path".to_string()),
    }
}

fn prompt_for_mode(reader: &mut dyn BufRead, writer: &mut dyn Write) -> Result<CliCommand, String> {
    loop {
        writeln!(writer, "Select mode:").map_err(|error| error.to_string())?;
        for (index, spec) in CLI_COMMAND_SPECS.iter().enumerate() {
            writeln!(writer, "{}. {}", index + 1, spec.menu_label)
                .map_err(|error| error.to_string())?;
        }
        write!(writer, "Enter number: ").map_err(|error| error.to_string())?;
        writer.flush().map_err(|error| error.to_string())?;

        let line = read_line(reader, "failed to read mode selection")?;
        if let Some(spec) = parse_mode_selection(line.trim()) {
            return spec.kind.resolve_interactive(reader, writer);
        }

        writeln!(
            writer,
            "Invalid selection. Enter a number from 1 to {}.",
            CLI_COMMAND_SPECS.len()
        )
        .map_err(|error| error.to_string())?;
    }
}

fn find_command_spec(mode: &OsStr) -> Option<&'static CliCommandSpec> {
    CLI_COMMAND_SPECS
        .iter()
        .find(|spec| mode == OsStr::new(spec.name))
}

fn parse_mode_selection(selection: &str) -> Option<&'static CliCommandSpec> {
    let index = selection.parse::<usize>().ok()?.checked_sub(1)?;
    CLI_COMMAND_SPECS.get(index)
}

fn parse_argless_command(
    args: &[OsString],
    command: CliCommand,
    name: &str,
) -> Result<CliCommand, String> {
    match args {
        [_] => Ok(command),
        [_, ..] => Err(format!("{name} mode does not accept additional arguments")),
        _ => Err("expected zero, one, or two arguments".to_string()),
    }
}

fn parse_search_summaries_args(args: &[OsString]) -> Result<CliCommand, String> {
    match args {
        [_, query] => Ok(CliCommand::SearchSummaries {
            query: query.to_string_lossy().into_owned(),
        }),
        [_] => Err("search-summaries mode requires a query argument".to_string()),
        [_, ..] => Err("search-summaries mode accepts exactly one query argument".to_string()),
        _ => Err("expected zero, one, or two arguments".to_string()),
    }
}

fn print_run_summary(summary: &super::RunSummary, writer: &mut dyn Write) -> Result<(), String> {
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

fn print_stt_summary(summary: &SttRunSummary, writer: &mut dyn Write) -> Result<(), String> {
    writeln!(writer, "job_id: {}", summary.job_id).map_err(|error| error.to_string())?;
    writeln!(writer, "job_dir: {}", summary.job_dir.display())
        .map_err(|error| error.to_string())?;
    writeln!(writer, "stt_dir: {}", summary.stt_dir.display())
        .map_err(|error| error.to_string())?;
    for transcript in &summary.transcripts {
        writeln!(
            writer,
            "{} -> {}",
            transcript.source_path.display(),
            transcript.text_path.display()
        )
        .map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn print_summary_run_summary(
    summary: &SummaryRunSummary,
    writer: &mut dyn Write,
) -> Result<(), String> {
    writeln!(writer, "job_id: {}", summary.job_id).map_err(|error| error.to_string())?;
    writeln!(writer, "job_dir: {}", summary.job_dir.display())
        .map_err(|error| error.to_string())?;
    writeln!(writer, "summary_dir: {}", summary.summary_dir.display())
        .map_err(|error| error.to_string())?;
    writeln!(writer, "summary_file: {}", summary.summary_file.display())
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn parse_input_path_os(input: &OsStr) -> Result<PathBuf, String> {
    match input.to_str() {
        Some(value) => parse_input_path_value(value),
        None => Ok(PathBuf::from(input)),
    }
}

fn parse_input_path_value(input: &str) -> Result<PathBuf, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input path is required".to_string());
    }

    Ok(PathBuf::from(strip_wrapping_quotes(trimmed)))
}

fn strip_wrapping_quotes(input: &str) -> &str {
    if input.len() < 2 {
        return input;
    }

    let bytes = input.as_bytes();
    let first = bytes[0];
    let last = bytes[input.len() - 1];
    if (first == b'\'' && last == b'\'') || (first == b'"' && last == b'"') {
        &input[1..input.len() - 1]
    } else {
        input
    }
}
