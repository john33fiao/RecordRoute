use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Vector index entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEntry {
    /// Document ID (file UUID)
    pub doc_id: String,

    /// Path to embedding file
    pub embedding_path: PathBuf,

    /// Document metadata
    pub metadata: VectorMetadata,

    /// Timestamp when indexed
    pub indexed_at: DateTime<Utc>,

    /// Deleted flag
    #[serde(default)]
    pub deleted: bool,
}

/// Vector metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMetadata {
    /// Original filename
    pub filename: String,

    /// File path
    pub file_path: String,

    /// Transcript path
    pub transcript_path: Option<String>,

    /// Summary path
    pub summary_path: Option<String>,

    /// One-line summary
    pub one_line_summary: Option<String>,

    /// Document timestamp (for filtering)
    #[serde(default)]
    pub timestamp: Option<DateTime<Utc>>,

    /// Tags
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Vector index structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIndex {
    /// Entries map (doc_id -> entry)
    pub entries: HashMap<String, VectorEntry>,

    /// Embedding model used
    pub embedding_model: String,

    /// Embedding dimension
    pub embedding_dim: usize,
}

impl VectorIndex {
    /// Create new empty index
    pub fn new(embedding_model: impl Into<String>, embedding_dim: usize) -> Self {
        Self {
            entries: HashMap::new(),
            embedding_model: embedding_model.into(),
            embedding_dim,
        }
    }

    /// Add entry to index
    pub fn add_entry(&mut self, entry: VectorEntry) {
        self.entries.insert(entry.doc_id.clone(), entry);
    }

    /// Get entry by doc_id
    pub fn get_entry(&self, doc_id: &str) -> Option<&VectorEntry> {
        self.entries.get(doc_id)
    }

    /// Delete entry (soft delete)
    pub fn delete_entry(&mut self, doc_id: &str) {
        if let Some(entry) = self.entries.get_mut(doc_id) {
            entry.deleted = true;
        }
    }

    /// Get active entries (not deleted)
    pub fn active_entries(&self) -> Vec<&VectorEntry> {
        self.entries.values().filter(|e| !e.deleted).collect()
    }

    /// Count active entries
    pub fn count(&self) -> usize {
        self.active_entries().len()
    }
}

/// Search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Document ID
    pub doc_id: String,

    /// Similarity score (0.0 to 1.0)
    pub score: f32,

    /// Metadata
    pub metadata: VectorMetadata,
}

impl SearchResult {
    pub fn new(doc_id: String, score: f32, metadata: VectorMetadata) -> Self {
        Self {
            doc_id,
            score,
            metadata,
        }
    }
}
