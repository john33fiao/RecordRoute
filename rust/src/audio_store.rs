use crate::index::SourceKind;
use crate::storage::StorageConfig;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

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
            fs::create_dir_all(path).map_err(|error| {
                format!("failed to create directory {}: {error}", path.display())
            })?;
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
        copy_if_missing_in_temp_root(&source, &cache_path, &self.cache_temp_root())?;
        Ok(cache_path)
    }

    pub fn apply_cache_policy(
        &self,
        warm_keys: &[String],
        keep_keys: &HashSet<String>,
    ) -> Result<(), String> {
        self.ensure_dirs()?;
        for storage_key in warm_keys {
            if let Err(error) = self.materialize_to_cache(storage_key) {
                eprintln!("{error}");
            }
        }
        self.prune_cache_except(keep_keys)
    }

    fn prune_cache_except(&self, keep_keys: &HashSet<String>) -> Result<(), String> {
        if !self.cache_root.is_dir() {
            return Ok(());
        }

        for storage_key in self.cache_storage_keys()? {
            if keep_keys.contains(&storage_key) {
                continue;
            }
            let path = self.cache_root.join(&storage_key);
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(_) => continue,
            }
        }

        self.prune_empty_cache_dirs();
        Ok(())
    }

    fn cache_storage_keys(&self) -> Result<Vec<String>, String> {
        let mut pending_dirs = vec![self.cache_root.clone()];
        let mut storage_keys = Vec::new();

        while let Some(dir) = pending_dirs.pop() {
            let entries = match fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(error) if error.kind() == ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(format!(
                        "failed to read cache directory {}: {error}",
                        dir.display()
                    ));
                }
            };

            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => continue,
                };
                let file_type = match entry.file_type() {
                    Ok(file_type) => file_type,
                    Err(_) => continue,
                };
                let path = entry.path();
                if file_type.is_symlink() {
                    continue;
                }
                if file_type.is_dir() {
                    pending_dirs.push(path);
                    continue;
                }
                if !file_type.is_file() {
                    continue;
                }

                let relative = match path.strip_prefix(&self.cache_root) {
                    Ok(relative) => relative,
                    Err(_) => continue,
                };
                storage_keys.push(storage_key_from_relative_path(relative));
            }
        }

        Ok(storage_keys)
    }

    fn prune_empty_cache_dirs(&self) {
        let mut directories = Vec::new();
        collect_directories(&self.cache_root, &mut directories);
        directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        for directory in directories {
            if directory == self.cache_root {
                continue;
            }
            let _ = fs::remove_dir(&directory);
        }
    }

    fn cache_temp_root(&self) -> PathBuf {
        self.cache_root
            .parent()
            .unwrap_or(&self.cache_root)
            .join(".audio-cache-tmp")
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
    copy_replace(source, destination, None)
}

fn copy_if_missing_in_temp_root(
    source: &Path,
    destination: &Path,
    temp_root: &Path,
) -> Result<(), String> {
    if destination.is_file() {
        return Ok(());
    }
    copy_replace(source, destination, Some(temp_root))
}

fn copy_replace(source: &Path, destination: &Path, temp_root: Option<&Path>) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create parent directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let temp_path = build_temp_path(destination, temp_root)?;
    let mut reader = fs::File::open(source)
        .map_err(|error| format!("failed to open {}: {error}", source.display()))?;
    let mut writer = fs::File::create(&temp_path)
        .map_err(|error| format!("failed to create {}: {error}", temp_path.display()))?;
    std::io::copy(&mut reader, &mut writer).map_err(|error| {
        format!(
            "failed to copy {} to {}: {error}",
            source.display(),
            temp_path.display()
        )
    })?;
    writer
        .flush()
        .map_err(|error| format!("failed to flush {}: {error}", temp_path.display()))?;
    drop(writer);
    match fs::rename(&temp_path, destination) {
        Ok(()) => Ok(()),
        Err(error) if destination.is_file() => {
            let _ = fs::remove_file(&temp_path);
            Ok(())
        }
        Err(error) => Err(format!(
            "failed to finalize {} from {}: {error}",
            destination.display(),
            temp_path.display()
        )),
    }
}

fn build_temp_path(destination: &Path, temp_root: Option<&Path>) -> Result<PathBuf, String> {
    match temp_root {
        Some(temp_root) => {
            fs::create_dir_all(temp_root).map_err(|error| {
                format!(
                    "failed to create temp directory {}: {error}",
                    temp_root.display()
                )
            })?;
            Ok(temp_root.join(format!("{}.tmp", Uuid::now_v7())))
        }
        None => Ok(destination.with_extension(format!(
            "{}.{}.tmp",
            destination
                .extension()
                .and_then(OsStr::to_str)
                .unwrap_or("artifact"),
            Uuid::now_v7()
        ))),
    }
}

fn storage_key_from_relative_path(relative: &Path) -> String {
    relative.to_string_lossy().replace('\\', "/")
}

fn collect_directories(root: &Path, directories: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        directories.push(path.clone());
        collect_directories(&path, directories);
    }
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
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

#[cfg(test)]
mod tests {
    use super::AudioStore;
    use std::collections::HashSet;
    use std::fs;
    use std::sync::Arc;
    use std::thread;
    use uuid::Uuid;

    #[test]
    fn materialize_to_cache_is_safe_under_concurrent_calls() {
        let repo_root =
            std::env::temp_dir().join(format!("recordroute-audio-store-{}", Uuid::now_v7()));
        let source = repo_root.join("db/audio/sources/hash/source.wav");
        fs::create_dir_all(
            source
                .parent()
                .expect("source path should have a parent directory"),
        )
        .expect("create source dir");
        fs::write(&source, "synthetic audio").expect("write source");

        let store = Arc::new(AudioStore::new(&repo_root).expect("audio store"));
        store.ensure_dirs().expect("ensure dirs");

        let handles = (0..4)
            .map(|_| {
                let store = Arc::clone(&store);
                thread::spawn(move || {
                    store
                        .materialize_to_cache("sources/hash/source.wav")
                        .expect("materialize to cache")
                })
            })
            .collect::<Vec<_>>();

        let cached_paths = handles
            .into_iter()
            .map(|handle| handle.join().expect("join worker"))
            .collect::<Vec<_>>();

        for cached_path in cached_paths {
            assert_eq!(
                fs::read_to_string(&cached_path).expect("read cached"),
                "synthetic audio"
            );
        }
    }

    #[test]
    fn apply_cache_policy_prunes_stale_cache_files_and_empty_dirs() {
        let repo_root =
            std::env::temp_dir().join(format!("recordroute-audio-store-{}", Uuid::now_v7()));
        let store = AudioStore::new(&repo_root).expect("audio store");
        store.ensure_dirs().expect("ensure dirs");

        let keep_path = store.cache_root.join("sources/hash/keep.wav");
        let stale_path = store.cache_root.join("jobs/job-stale/mono_mix.wav");
        let temp_residue = store.cache_root.join("jobs/job-stale/mono_mix.wav.tmp");

        fs::create_dir_all(keep_path.parent().expect("keep parent")).expect("keep parent dir");
        fs::create_dir_all(stale_path.parent().expect("stale parent")).expect("stale parent dir");
        fs::write(&keep_path, "keep").expect("write keep");
        fs::write(&stale_path, "stale").expect("write stale");
        fs::write(&temp_residue, "tmp").expect("write temp residue");

        store
            .apply_cache_policy(&[], &HashSet::from(["sources/hash/keep.wav".to_string()]))
            .expect("apply cache policy");

        assert!(keep_path.is_file());
        assert!(!stale_path.exists());
        assert!(!temp_residue.exists());
        assert!(!store.cache_root.join("jobs/job-stale").exists());
    }
}
