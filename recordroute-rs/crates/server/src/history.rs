use crate::types::HistoryRecord;
use recordroute_common::Result;
use serde_json;
use std::fs;
use std::path::Path;

pub struct HistoryManager {
    records: Vec<HistoryRecord>,
    file_path: std::path::PathBuf,
}

impl HistoryManager {
    pub fn load(path: &Path) -> Result<Self> {
        let mut records: Vec<HistoryRecord> = if path.exists() {
            let data = fs::read_to_string(path)?;
            serde_json::from_str(&data).unwrap_or_else(|_| Vec::new())
        } else {
            Vec::new()
        };

        // Migrate old records without file_path
        for record in &mut records {
            if record.file_path.is_empty() {
                record.file_path = format!("/download/{}", record.id);
            }
        }

        Ok(Self {
            records,
            file_path: path.to_path_buf(),
        })
    }

    pub fn add_record(&mut self, record: HistoryRecord) -> Result<()> {
        self.records.push(record);
        self.save()
    }

    pub fn update_record(&mut self, id: &str, update_fn: impl FnOnce(&mut HistoryRecord)) -> Result<()> {
        if let Some(record) = self.records.iter_mut().find(|r| r.id == id) {
            update_fn(record);
            self.save()?;
        }
        Ok(())
    }

    pub fn delete_records(&mut self, ids: &[String]) -> Result<()> {
        for id in ids {
            if let Some(record) = self.records.iter_mut().find(|r| &r.id == id) {
                record.deleted = true;
            }
        }
        self.save()
    }

    pub fn get_active_records(&self) -> Vec<HistoryRecord> {
        self.records.iter().filter(|r| !r.deleted).cloned().collect()
    }

    pub fn get_by_id(&self, id: &str) -> Option<&HistoryRecord> {
        self.records.iter().find(|r| r.id == id && !r.deleted)
    }

    fn save(&self) -> Result<()> {
        let data = serde_json::to_string_pretty(&self.records)?;
        fs::write(&self.file_path, data)?;
        Ok(())
    }
}
