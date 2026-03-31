use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use crate::index::{AudioArtifactRecord, IndexStore};

const SUMMARY_FILE_NAME: &str = "result.md";

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

pub(crate) fn listed_or_discovered_audio_files(
    index_store: &IndexStore,
    job_id: &str,
) -> Result<Vec<PathBuf>, String> {
    Ok(listed_or_discovered_audio_artifacts(index_store, job_id)?
        .into_iter()
        .map(|artifact| index_store.job_dir(job_id).join(artifact.logical_name))
        .collect())
}

pub(crate) fn listed_or_discovered_audio_artifacts(
    index_store: &IndexStore,
    job_id: &str,
) -> Result<Vec<AudioArtifactRecord>, String> {
    let artifacts = index_store.list_audio_artifacts(job_id)?;
    if !artifacts.is_empty() {
        return Ok(artifacts);
    }

    let job_dir = index_store.job_dir(job_id);
    if !job_dir.is_dir() {
        return Ok(Vec::new());
    }

    supported_audio_files(&job_dir)?
        .into_iter()
        .map(|path| {
            let logical_name = audio_logical_name(&path)?;
            Ok(AudioArtifactRecord {
                job_id: job_id.to_string(),
                storage_key: format!("jobs/{job_id}/{logical_name}"),
                logical_name,
            })
        })
        .collect()
}

pub(crate) fn resolve_audio_artifacts(
    index_store: &IndexStore,
    job_id: &str,
    audio_files: &[PathBuf],
) -> Result<Vec<AudioArtifactRecord>, String> {
    let artifacts = listed_or_discovered_audio_artifacts(index_store, job_id)?;
    if audio_files.is_empty() {
        return Ok(artifacts);
    }

    let mut resolved = Vec::with_capacity(audio_files.len());
    for audio_file in audio_files {
        let logical_name = audio_logical_name(audio_file)?;
        let artifact = artifacts
            .iter()
            .find(|artifact| artifact.logical_name == logical_name)
            .cloned()
            .ok_or_else(|| format!("audio artifact not found for job {job_id}: {logical_name}"))?;
        resolved.push(artifact);
    }
    Ok(resolved)
}

pub(crate) fn transcript_output_path(stt_dir: &Path, audio_file: &Path) -> Result<PathBuf, String> {
    Ok(stt_dir.join(transcript_file_name(audio_file)?))
}

pub(crate) fn audio_logical_name(audio_file: &Path) -> Result<String, String> {
    audio_file
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| format!("audio file does not have a valid file name: {}", audio_file.display()))
}

pub(crate) fn transcript_file_name(audio_file: &Path) -> Result<String, String> {
    let stem = audio_file.file_stem().ok_or_else(|| {
        format!(
            "audio file does not have a valid file stem: {}",
            audio_file.display()
        )
    })?;
    Ok(PathBuf::from(stem)
        .with_extension("txt")
        .to_string_lossy()
        .into_owned())
}

pub(crate) fn transcript_id_from_file_name(file_name: &str) -> Result<String, String> {
    Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_string)
        .ok_or_else(|| format!("transcript file does not have a valid file stem: {file_name}"))
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

pub(crate) fn one_line_summary_prompt_file_path(
    summary_dir: &Path,
    source_file_name: &str,
) -> Result<PathBuf, String> {
    let mut file_name = OsString::from(".");
    file_name.push(source_file_stem(source_file_name)?);
    file_name.push(".one-line.prompt.txt");
    Ok(summary_dir.join(file_name))
}

pub(crate) fn one_line_summary_output_path(
    summary_dir: &Path,
    source_file_name: &str,
) -> Result<PathBuf, String> {
    let mut file_name = OsString::from(".");
    file_name.push(source_file_stem(source_file_name)?);
    file_name.push(".one-line.txt");
    Ok(summary_dir.join(file_name))
}

pub(crate) fn summary_keywords_prompt_file_path(
    summary_dir: &Path,
    source_file_name: &str,
) -> Result<PathBuf, String> {
    let mut file_name = OsString::from(".");
    file_name.push(source_file_stem(source_file_name)?);
    file_name.push(".keywords.prompt.txt");
    Ok(summary_dir.join(file_name))
}

pub(crate) fn summary_keywords_output_path(
    summary_dir: &Path,
    source_file_name: &str,
) -> Result<PathBuf, String> {
    let mut file_name = OsString::from(".");
    file_name.push(source_file_stem(source_file_name)?);
    file_name.push(".keywords.txt");
    Ok(summary_dir.join(file_name))
}

pub(crate) fn summary_file_name() -> &'static str {
    SUMMARY_FILE_NAME
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
