use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use recordroute_core::config::AppConfig;
use recordroute_core::error::CoreError;
use recordroute_core::paths::{normalize_record_path, resolve_record_path};
use serde::Serialize;
use serde_json::{Map, Value};

const HISTORY_FILE_NAME: &str = "upload_history.json";
const FILE_REGISTRY_FILE_NAME: &str = "file_registry.json";
const TASK_TYPES: [&str; 3] = ["stt", "embedding", "summary"];

#[derive(Debug, Clone)]
pub struct StorageContext {
    pub config: AppConfig,
}

impl StorageContext {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    pub fn history_file(&self) -> PathBuf {
        self.config.db_root.join(HISTORY_FILE_NAME)
    }

    pub fn file_registry_file(&self) -> PathBuf {
        self.config.db_root.join(FILE_REGISTRY_FILE_NAME)
    }

    pub fn load_upload_history(&self) -> Result<Vec<HistoryRecord>, CoreError> {
        let path = self.history_file();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let payload = match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str::<Value>(&contents) {
                Ok(value) => value,
                Err(_) => return Ok(Vec::new()),
            },
            Err(error) => return Err(CoreError::from(error)),
        };

        let Some(records) = payload.as_array() else {
            return Ok(Vec::new());
        };

        Ok(records
            .iter()
            .filter_map(|value| HistoryRecord::from_value(value.clone()))
            .filter(|record| !record.deleted)
            .collect())
    }

    pub fn load_file_registry(&self) -> Result<BTreeMap<String, FileRegistryEntry>, CoreError> {
        let path = self.file_registry_file();
        if !path.exists() {
            return Ok(BTreeMap::new());
        }

        let payload = match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str::<Value>(&contents) {
                Ok(value) => value,
                Err(_) => return Ok(BTreeMap::new()),
            },
            Err(error) => return Err(CoreError::from(error)),
        };

        let Some(entries) = payload.as_object() else {
            return Ok(BTreeMap::new());
        };

        Ok(entries
            .iter()
            .filter_map(|(key, value)| FileRegistryEntry::from_value(key, value.clone()))
            .map(|entry| (entry.registry_key.clone(), entry))
            .collect())
    }

    pub fn resolve_file_identifier(
        &self,
        file_identifier: &str,
    ) -> Result<Option<ResolvedFile>, CoreError> {
        let cleaned_identifier = sanitize_identifier(file_identifier)?;
        let registry = self.load_file_registry()?;

        if let Some(entry) = registry.get(&cleaned_identifier) {
            return self.resolve_registry_entry(&cleaned_identifier, entry);
        }

        let normalized_path = normalize_record_path(&self.config.db_root, &cleaned_identifier)?;
        let full_path = resolve_record_path(&self.config.db_root, &normalized_path)?;
        if !full_path.exists() {
            return Ok(None);
        }

        let mut resolved_identifier = cleaned_identifier.clone();
        let mut record_id = None;
        let mut task_type = None;
        let mut original_filename = None;

        for (registry_key, entry) in &registry {
            if entry.file_path == normalized_path {
                resolved_identifier = registry_key.clone();
                record_id = entry.record_id.clone();
                task_type = entry.task_type.clone();
                original_filename = entry.original_filename.clone();
                break;
            }
        }

        if original_filename.is_none() {
            original_filename = full_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned());
        }

        Ok(Some(ResolvedFile {
            file_identifier: resolved_identifier,
            file_path: normalized_path,
            full_path,
            record_id,
            task_type,
            original_filename,
        }))
    }

    pub fn load_segments(
        &self,
        file_identifier: &str,
    ) -> Result<Option<Vec<SegmentItem>>, CoreError> {
        let Some(resolved_file) = self.resolve_file_identifier(file_identifier)? else {
            return Ok(None);
        };

        let segments_path = resolved_file.full_path.with_extension("segments.json");
        if !segments_path.exists() {
            return Ok(Some(Vec::new()));
        }

        let contents = fs::read_to_string(&segments_path)?;
        let payload = match serde_json::from_str::<Value>(&contents) {
            Ok(value) => value,
            Err(_) => return Ok(Some(Vec::new())),
        };

        let segments = payload
            .as_object()
            .and_then(|object| object.get("segments"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|value| SegmentItem::from_value(value.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(Some(segments))
    }

    fn resolve_registry_entry(
        &self,
        registry_key: &str,
        entry: &FileRegistryEntry,
    ) -> Result<Option<ResolvedFile>, CoreError> {
        let full_path = match resolve_record_path(&self.config.db_root, &entry.file_path) {
            Ok(path) => path,
            Err(CoreError::InvalidRecordPath | CoreError::RecordPathEscapesDbRoot { .. }) => {
                return Ok(None);
            }
            Err(error) => return Err(error),
        };

        if !full_path.exists() {
            return Ok(None);
        }

        let original_filename = entry.original_filename.clone().or_else(|| {
            full_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        });

        Ok(Some(ResolvedFile {
            file_identifier: registry_key.to_string(),
            file_path: entry.file_path.clone(),
            full_path,
            record_id: entry.record_id.clone(),
            task_type: entry.task_type.clone(),
            original_filename,
        }))
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HistoryRecord {
    pub id: String,
    pub timestamp: String,
    pub filename: String,
    pub file_type: String,
    pub duration: Option<String>,
    pub file_path: String,
    pub completed_tasks: BTreeMap<String, bool>,
    pub download_links: BTreeMap<String, String>,
    pub title_summary: String,
    pub file_hash: Option<String>,
    pub deleted: bool,
    pub deleted_at: Option<String>,
    pub deleted_assets: BTreeMap<String, Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl HistoryRecord {
    fn from_value(value: Value) -> Option<Self> {
        let mut object = value.as_object()?.clone();

        let deleted = take_bool(&mut object, "deleted").unwrap_or(false);

        let mut completed_tasks = take_bool_map(&mut object, "completed_tasks");
        for task_type in TASK_TYPES {
            completed_tasks
                .entry(task_type.to_string())
                .or_insert(false);
        }

        Some(Self {
            id: take_string(&mut object, "id").unwrap_or_default(),
            timestamp: take_string(&mut object, "timestamp").unwrap_or_default(),
            filename: take_string(&mut object, "filename").unwrap_or_default(),
            file_type: take_string(&mut object, "file_type").unwrap_or_default(),
            duration: take_string(&mut object, "duration"),
            file_path: take_string(&mut object, "file_path").unwrap_or_default(),
            completed_tasks,
            download_links: take_string_map(&mut object, "download_links"),
            title_summary: take_string(&mut object, "title_summary").unwrap_or_default(),
            file_hash: take_string(&mut object, "file_hash"),
            deleted,
            deleted_at: take_string(&mut object, "deleted_at"),
            deleted_assets: take_value_map(&mut object, "deleted_assets"),
            extra: object.into_iter().collect(),
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FileRegistryEntry {
    pub registry_key: String,
    pub file_uuid: String,
    pub file_path: String,
    pub record_id: Option<String>,
    pub task_type: Option<String>,
    pub original_filename: Option<String>,
    pub created_at: Option<String>,
    pub deleted: bool,
    pub deleted_at: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl FileRegistryEntry {
    fn from_value(registry_key: &str, value: Value) -> Option<Self> {
        let mut object = value.as_object()?.clone();

        Some(Self {
            registry_key: registry_key.to_string(),
            file_uuid: take_string(&mut object, "file_uuid")
                .unwrap_or_else(|| registry_key.to_string()),
            file_path: take_string(&mut object, "file_path").unwrap_or_default(),
            record_id: take_string(&mut object, "record_id"),
            task_type: take_string(&mut object, "task_type"),
            original_filename: take_string(&mut object, "original_filename"),
            created_at: take_string(&mut object, "created_at"),
            deleted: take_bool(&mut object, "deleted").unwrap_or(false),
            deleted_at: take_string(&mut object, "deleted_at"),
            extra: object.into_iter().collect(),
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SegmentItem {
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub speaker: Option<String>,
}

impl SegmentItem {
    fn from_value(value: Value) -> Option<Self> {
        let object = value.as_object()?;
        Some(Self {
            start: object
                .get("start")
                .and_then(json_to_f64)
                .unwrap_or_default(),
            end: object.get("end").and_then(json_to_f64).unwrap_or_default(),
            text: object
                .get("text")
                .and_then(value_to_string)
                .unwrap_or_default(),
            speaker: object.get("speaker").and_then(value_to_string),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFile {
    pub file_identifier: String,
    pub file_path: String,
    pub full_path: PathBuf,
    pub record_id: Option<String>,
    pub task_type: Option<String>,
    pub original_filename: Option<String>,
}

fn sanitize_identifier(file_identifier: &str) -> Result<String, CoreError> {
    let mut cleaned = file_identifier.trim().replace('\\', "/");
    if let Some(stripped) = cleaned.strip_prefix("/download/") {
        cleaned = stripped.to_string();
    }
    cleaned = cleaned.trim_start_matches('/').to_string();

    if cleaned.is_empty() {
        return Err(CoreError::InvalidFileIdentifier);
    }

    Ok(cleaned)
}

fn take_string(object: &mut Map<String, Value>, key: &str) -> Option<String> {
    object.remove(key).and_then(|value| value_to_string(&value))
}

fn take_bool(object: &mut Map<String, Value>, key: &str) -> Option<bool> {
    object.remove(key).and_then(|value| value.as_bool())
}

fn take_bool_map(object: &mut Map<String, Value>, key: &str) -> BTreeMap<String, bool> {
    object
        .remove(key)
        .and_then(|value| value.as_object().cloned())
        .map(|items| {
            items
                .into_iter()
                .map(|(key, value)| (key, value.as_bool().unwrap_or(false)))
                .collect()
        })
        .unwrap_or_default()
}

fn take_string_map(object: &mut Map<String, Value>, key: &str) -> BTreeMap<String, String> {
    object
        .remove(key)
        .and_then(|value| value.as_object().cloned())
        .map(|items| {
            items
                .into_iter()
                .filter_map(|(key, value)| value_to_string(&value).map(|value| (key, value)))
                .collect()
        })
        .unwrap_or_default()
}

fn take_value_map(object: &mut Map<String, Value>, key: &str) -> BTreeMap<String, Value> {
    object
        .remove(key)
        .and_then(|value| value.as_object().cloned())
        .map(|items| items.into_iter().collect())
        .unwrap_or_default()
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(string) => Some(string.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        _ => None,
    }
}

fn json_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(string) => string.parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{SegmentItem, StorageContext};
    use recordroute_core::config::AppConfig;
    use serde_json::json;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn history_loader_returns_empty_when_missing_or_invalid() {
        let temp_dir = TempDir::new().unwrap();
        let context = test_context(temp_dir.path());

        assert_eq!(context.load_upload_history().unwrap(), Vec::new());

        fs::write(context.history_file(), "{invalid").unwrap();
        assert_eq!(context.load_upload_history().unwrap(), Vec::new());
    }

    #[test]
    fn history_loader_filters_deleted_records_and_fills_defaults() {
        let temp_dir = TempDir::new().unwrap();
        let context = test_context(temp_dir.path());

        fs::write(
            context.history_file(),
            serde_json::to_vec_pretty(&json!([
                {
                    "id": "active",
                    "timestamp": "2026-03-23T10:00:00",
                    "filename": "meeting.m4a",
                    "file_type": "audio",
                    "file_path": "DB/uploads/id/meeting.m4a",
                    "download_links": { "stt": "/download/file" }
                },
                {
                    "id": "deleted",
                    "timestamp": "2026-03-23T10:10:00",
                    "filename": "old.m4a",
                    "file_type": "audio",
                    "file_path": "DB/uploads/id/old.m4a",
                    "deleted": true
                }
            ]))
            .unwrap(),
        )
        .unwrap();

        let history = context.load_upload_history().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, "active");
        assert_eq!(history[0].completed_tasks["stt"], false);
        assert_eq!(history[0].completed_tasks["embedding"], false);
        assert_eq!(history[0].completed_tasks["summary"], false);
    }

    #[test]
    fn registry_loader_returns_empty_when_missing_or_invalid() {
        let temp_dir = TempDir::new().unwrap();
        let context = test_context(temp_dir.path());

        assert!(context.load_file_registry().unwrap().is_empty());

        fs::write(context.file_registry_file(), "{invalid").unwrap();
        assert!(context.load_file_registry().unwrap().is_empty());
    }

    #[test]
    fn resolves_file_identifier_by_registry_key_and_alias() {
        let temp_dir = TempDir::new().unwrap();
        let context = test_context(temp_dir.path());
        let file_path = context
            .config
            .db_root
            .join("uploads")
            .join("upload-id")
            .join("meeting.mp3");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, b"audio").unwrap();
        fs::write(
            context.file_registry_file(),
            serde_json::to_vec_pretty(&json!({
                "download-key": {
                    "file_uuid": "download-key",
                    "file_path": "DB/uploads/upload-id/meeting.mp3",
                    "record_id": "record-id",
                    "task_type": "stt",
                    "original_filename": "meeting.mp3"
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let by_key = context
            .resolve_file_identifier("download-key")
            .unwrap()
            .unwrap();
        assert_eq!(by_key.file_path, "DB/uploads/upload-id/meeting.mp3");
        assert_eq!(by_key.record_id.as_deref(), Some("record-id"));

        let by_alias = context
            .resolve_file_identifier("DB/uploads/upload-id/meeting.mp3")
            .unwrap()
            .unwrap();
        assert_eq!(by_alias.file_identifier, "download-key");
    }

    #[test]
    fn segments_loader_returns_empty_for_missing_or_invalid_sidecar() {
        let temp_dir = TempDir::new().unwrap();
        let context = test_context(temp_dir.path());
        let audio_path = context
            .config
            .db_root
            .join("uploads")
            .join("upload-id")
            .join("meeting.m4a");
        fs::create_dir_all(audio_path.parent().unwrap()).unwrap();
        fs::write(&audio_path, b"audio").unwrap();
        fs::write(
            context.file_registry_file(),
            serde_json::to_vec_pretty(&json!({
                "segment-key": {
                    "file_uuid": "segment-key",
                    "file_path": "DB/uploads/upload-id/meeting.m4a"
                }
            }))
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            context.load_segments("segment-key").unwrap().unwrap(),
            Vec::<SegmentItem>::new()
        );

        fs::write(audio_path.with_extension("segments.json"), "{invalid").unwrap();
        assert_eq!(
            context.load_segments("segment-key").unwrap().unwrap(),
            Vec::<SegmentItem>::new()
        );
    }

    #[test]
    fn segments_loader_normalizes_valid_sidecar() {
        let temp_dir = TempDir::new().unwrap();
        let context = test_context(temp_dir.path());
        let audio_path = context
            .config
            .db_root
            .join("uploads")
            .join("upload-id")
            .join("meeting.m4a");
        fs::create_dir_all(audio_path.parent().unwrap()).unwrap();
        fs::write(&audio_path, b"audio").unwrap();
        fs::write(
            context.file_registry_file(),
            serde_json::to_vec_pretty(&json!({
                "segment-key": {
                    "file_uuid": "segment-key",
                    "file_path": "DB/uploads/upload-id/meeting.m4a"
                }
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            audio_path.with_extension("segments.json"),
            serde_json::to_vec_pretty(&json!({
                "segments": [
                    { "start": 0.0, "end": 2.4, "text": "안녕하세요", "speaker": "SPEAKER_00" },
                    { "start": "2.4", "end": 5.0, "text": "회의를 시작하겠습니다.", "speaker": "SPEAKER_01" }
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        let segments = context.load_segments("segment-key").unwrap().unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].speaker.as_deref(), Some("SPEAKER_00"));
        assert_eq!(segments[1].start, 2.4);
    }

    fn test_context(root: &Path) -> StorageContext {
        let db_root = root.join("DB");
        let model_root = root.join("models");
        fs::create_dir_all(&db_root).unwrap();
        fs::create_dir_all(&model_root).unwrap();

        StorageContext::new(AppConfig::from_env_iter([
            ("DB_FOLDER_PATH", db_root.to_string_lossy().to_string()),
            ("MODEL_ROOT_PATH", model_root.to_string_lossy().to_string()),
        ]))
    }
}
