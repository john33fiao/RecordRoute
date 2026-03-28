use crate::index::SourceKind;
use crate::storage::StorageConfig;
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSource {
    pub source_ref: String,
    pub source_kind: SourceKind,
    pub source_content_sha256: String,
    pub source_file_name: String,
}

#[derive(Debug, Clone)]
pub struct AudioStore {
    root: PathBuf,
    cache_root: PathBuf,
    spool_root: PathBuf,
}

impl AudioStore {
    pub fn new(repo_root: &Path) -> Result<Self, String> {
        let config = StorageConfig::load(repo_root)?;
        Ok(Self {
            root: config.audio.root,
            cache_root: config.audio.cache_root,
            spool_root: config.audio.spool_root,
        })
    }

    pub fn ensure_dirs(&self) -> Result<(), String> {
        for path in [&self.root, &self.cache_root, &self.spool_root] {
            fs::create_dir_all(path)
                .map_err(|error| format!("failed to create directory {}: {error}", path.display()))?;
        }
        Ok(())
    }

    pub fn spool_root(&self) -> &Path {
        &self.spool_root
    }

    pub fn job_dir(&self, job_id: &str) -> PathBuf {
        self.root.join("jobs").join(job_id)
    }

    pub fn resolve_storage_path(&self, storage_key: &str) -> PathBuf {
        self.root.join(storage_key)
    }

    pub fn publish_source(
        &self,
        input_path: &Path,
        preferred_name: Option<&str>,
        source_kind: SourceKind,
    ) -> Result<ImportedSource, String> {
        self.ensure_dirs()?;
        if !input_path.is_file() {
            return Err(format!("input file not found: {}", input_path.display()));
        }
        let file_hash = hash_file(input_path)?;
        let source_file_name = preferred_name
            .map(str::to_string)
            .or_else(|| {
                input_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| storage_file_name_from_path(input_path));
        let extension = input_path.extension().and_then(OsStr::to_str);
        let storage_key = build_source_storage_key(&file_hash, extension);
        let destination = self.resolve_storage_path(&storage_key);
        copy_if_missing(input_path, &destination)?;
        Ok(ImportedSource {
            source_ref: storage_key,
            source_kind,
            source_content_sha256: file_hash,
            source_file_name,
        })
    }

    pub fn read_bytes(&self, storage_key: &str) -> Result<Vec<u8>, String> {
        let path = self.resolve_storage_path(storage_key);
        fs::read(&path).map_err(|error| format!("failed to read audio {}: {error}", path.display()))
    }

    pub fn materialize_to_cache(&self, storage_key: &str) -> Result<PathBuf, String> {
        self.ensure_dirs()?;
        let source = self.resolve_storage_path(storage_key);
        if !source.is_file() {
            return Err(format!("audio artifact not found: {}", source.display()));
        }
        let cache_path = self.cache_root.join(storage_key);
        if cache_path.is_file() {
            return Ok(cache_path);
        }
        copy_if_missing(&source, &cache_path)?;
        Ok(cache_path)
    }
}

fn build_source_storage_key(file_hash: &str, extension: Option<&str>) -> String {
    match extension.filter(|value| !value.is_empty()) {
        Some(extension) => format!("sources/{file_hash}/source.{extension}"),
        None => format!("sources/{file_hash}/source.bin"),
    }
}

fn storage_file_name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| "source.bin".to_string())
}

fn copy_if_missing(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.is_file() {
        return Ok(());
    }
    copy_replace(source, destination)
}

fn copy_replace(source: &Path, destination: &Path) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create parent directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let temp_path = destination.with_extension(format!(
        "{}.tmp",
        destination
            .extension()
            .and_then(OsStr::to_str)
            .unwrap_or("artifact")
    ));
    let mut reader = fs::File::open(source)
        .map_err(|error| format!("failed to open {}: {error}", source.display()))?;
    let mut writer = fs::File::create(&temp_path)
        .map_err(|error| format!("failed to create {}: {error}", temp_path.display()))?;
    std::io::copy(&mut reader, &mut writer)
        .map_err(|error| format!("failed to copy {} to {}: {error}", source.display(), temp_path.display()))?;
    writer
        .flush()
        .map_err(|error| format!("failed to flush {}: {error}", temp_path.display()))?;
    drop(writer);
    fs::rename(&temp_path, destination).map_err(|error| {
        format!(
            "failed to finalize {} from {}: {error}",
            destination.display(),
            temp_path.display()
        )
    })
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file =
        fs::File::open(path).map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
