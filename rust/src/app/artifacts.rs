use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

const SUMMARY_FILE_NAME: &str = "result.md";
const LEGACY_SUMMARY_TEXT_FILE_NAME: &str = "result.txt";

pub(crate) fn supported_audio_files(job_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut audio_files = Vec::new();

    for entry in fs::read_dir(job_dir).map_err(|error| {
        format!(
            "failed to read job directory {}: {error}",
            job_dir.display()
        )
    })? {
        let entry = entry.map_err(|error| {
            format!(
                "failed to inspect job directory entry {}: {error}",
                job_dir.display()
            )
        })?;
        let path = entry.path();
        if !path.is_file() || !is_supported_audio_file(&path) {
            continue;
        }
        audio_files.push(path);
    }

    audio_files.sort();
    Ok(audio_files)
}

pub(crate) fn count_supported_audio_files(job_dir: &Path) -> Result<usize, String> {
    supported_audio_files(job_dir).map(|files| files.len())
}

pub(crate) fn transcript_text_files(stt_dir: &Path) -> Result<Vec<PathBuf>, String> {
    if !stt_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut transcript_files = Vec::new();
    for entry in fs::read_dir(stt_dir).map_err(|error| {
        format!(
            "failed to read stt directory {}: {error}",
            stt_dir.display()
        )
    })? {
        let entry = entry.map_err(|error| {
            format!(
                "failed to inspect stt directory entry {}: {error}",
                stt_dir.display()
            )
        })?;
        let path = entry.path();
        if path.is_file() && is_transcript_text_file(&path) {
            transcript_files.push(path);
        }
    }

    transcript_files.sort();
    Ok(transcript_files)
}

pub(crate) fn transcript_output_path(stt_dir: &Path, audio_file: &Path) -> Result<PathBuf, String> {
    let stem = audio_file.file_stem().ok_or_else(|| {
        format!(
            "audio file does not have a valid file stem: {}",
            audio_file.display()
        )
    })?;
    Ok(stt_dir.join(stem).with_extension("txt"))
}

pub(crate) fn summary_output_path(
    summary_dir: &Path,
    _source_file_name: &str,
) -> Result<PathBuf, String> {
    Ok(summary_dir.join(SUMMARY_FILE_NAME))
}

pub(crate) fn summary_prompt_file_path(
    summary_dir: &Path,
    source_file_name: &str,
) -> Result<PathBuf, String> {
    let mut file_name = OsString::from(".");
    file_name.push(source_file_stem(source_file_name)?);
    file_name.push(".prompt.txt");
    Ok(summary_dir.join(file_name))
}

pub(crate) fn ensure_summary_output_path(
    summary_dir: &Path,
    source_file_name: &str,
) -> Result<PathBuf, String> {
    let canonical = summary_output_path(summary_dir, source_file_name)?;
    if canonical.is_file() {
        return Ok(canonical);
    }

    for legacy in legacy_summary_output_paths(summary_dir, source_file_name)? {
        if !legacy.is_file() {
            continue;
        }

        fs::rename(&legacy, &canonical).map_err(|error| {
            format!(
                "failed to migrate summary artifact {} to {}: {error}",
                legacy.display(),
                canonical.display()
            )
        })?;
        return Ok(canonical);
    }

    Ok(canonical)
}

pub(crate) fn read_summary_text(
    summary_dir: &Path,
    source_file_name: &str,
) -> Result<String, String> {
    let path = ensure_summary_output_path(summary_dir, source_file_name)?;
    if !path.is_file() {
        return Err(format!("summary not found: {}", path.display()));
    }
    fs::read_to_string(&path)
        .map_err(|error| format!("failed to read summary {}: {error}", path.display()))
}

pub(crate) fn collect_job_files(
    job_dir: &Path,
    source_file_name: &str,
) -> Result<Vec<String>, String> {
    let mut files = supported_audio_files(job_dir)?
        .into_iter()
        .filter_map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .collect::<Vec<_>>();

    let stt_dir = job_dir.join("stt");
    if stt_dir.is_dir() {
        for entry in fs::read_dir(&stt_dir)
            .map_err(|error| format!("failed to read {}: {error}", stt_dir.display()))?
        {
            let entry = entry.map_err(|error| format!("failed to read stt entry: {error}"))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                files.push(format!("stt/{name}"));
            }
        }
    }

    let summary_dir = job_dir.join("summary");
    if summary_dir.is_dir() {
        let canonical = ensure_summary_output_path(&summary_dir, source_file_name)?;
        let legacy_paths = legacy_summary_output_paths(&summary_dir, source_file_name)?;
        for entry in fs::read_dir(&summary_dir)
            .map_err(|error| format!("failed to read {}: {error}", summary_dir.display()))?
        {
            let entry = entry.map_err(|error| format!("failed to read summary entry: {error}"))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path != canonical && legacy_paths.iter().any(|legacy| legacy == &path) {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                files.push(format!("summary/{name}"));
            }
        }
    }

    files.sort();
    Ok(files)
}

pub(crate) fn summary_file_name() -> &'static str {
    SUMMARY_FILE_NAME
}

pub(crate) fn legacy_summary_output_paths(
    summary_dir: &Path,
    source_file_name: &str,
) -> Result<Vec<PathBuf>, String> {
    Ok(vec![
        legacy_summary_text_output_path(summary_dir),
        legacy_summary_markdown_output_path(summary_dir, source_file_name)?,
    ])
}

fn legacy_summary_text_output_path(summary_dir: &Path) -> PathBuf {
    summary_dir.join(LEGACY_SUMMARY_TEXT_FILE_NAME)
}

fn legacy_summary_markdown_output_path(
    summary_dir: &Path,
    source_file_name: &str,
) -> Result<PathBuf, String> {
    Ok(summary_dir
        .join(source_file_stem(source_file_name)?)
        .with_extension("md"))
}

fn source_file_stem(source_file_name: &str) -> Result<OsString, String> {
    Path::new(source_file_name)
        .file_stem()
        .map(OsStr::to_os_string)
        .ok_or_else(|| format!("source file does not have a valid file stem: {source_file_name}"))
}

fn is_supported_audio_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(extension)
            if extension.eq_ignore_ascii_case("wav")
                || extension.eq_ignore_ascii_case("mp3")
                || extension.eq_ignore_ascii_case("flac")
                || extension.eq_ignore_ascii_case("ogg")
    )
}

fn is_transcript_text_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"))
}
