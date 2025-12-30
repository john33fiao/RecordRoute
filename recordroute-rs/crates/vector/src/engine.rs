use recordroute_common::{AppConfig, Result};
use recordroute_llm::OllamaClient;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::similarity::cosine_similarity;
use crate::types::{SearchResult, VectorEntry, VectorIndex, VectorMetadata};

/// Vector search engine
pub struct VectorSearchEngine {
    index: Arc<RwLock<VectorIndex>>,
    index_path: PathBuf,
    embedding_dir: PathBuf,
    ollama: Arc<OllamaClient>,
    embedding_model: String,
}

impl VectorSearchEngine {
    /// Create new vector search engine
    pub fn new(
        config: &AppConfig,
        ollama: Arc<OllamaClient>,
    ) -> Result<Self> {
        let index_path = config.vector_index_path.clone();
        let embedding_dir = config.db_base_path.join("embeddings");

        // Load or create index
        let index = if index_path.exists() {
            let data = std::fs::read_to_string(&index_path)?;
            serde_json::from_str(&data)?
        } else {
            VectorIndex::new(&config.embedding_model, 768) // default dimension
        };

        info!("Vector search engine initialized - {} entries", index.count());

        Ok(Self {
            index: Arc::new(RwLock::new(index)),
            index_path,
            embedding_dir,
            ollama,
            embedding_model: config.embedding_model.clone(),
        })
    }

    /// Add document to index
    pub async fn add_document(
        &self,
        doc_id: impl Into<String>,
        text: &str,
        metadata: VectorMetadata,
    ) -> Result<()> {
        let doc_id = doc_id.into();
        info!("Adding document to vector index: {}", doc_id);

        // Generate embedding
        let embedding = self.ollama.embed(&self.embedding_model, text).await?;

        // Save embedding to file
        tokio::fs::create_dir_all(&self.embedding_dir).await?;
        let embedding_file = self.embedding_dir.join(format!("{}.json", doc_id));
        let embedding_json = serde_json::to_string(&embedding)?;
        tokio::fs::write(&embedding_file, embedding_json).await?;

        // Add to index
        let entry = VectorEntry {
            doc_id: doc_id.clone(),
            embedding_path: embedding_file,
            metadata,
            indexed_at: chrono::Utc::now(),
            deleted: false,
        };

        let mut index = self.index.write().await;
        index.add_entry(entry);
        self.save_index(&index).await?;

        info!("Document added to index: {}", doc_id);
        Ok(())
    }

    /// Search for similar documents
    pub async fn search(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<SearchResult>> {
        debug!("Searching for: {} (top_k={})", query, top_k);

        // Generate query embedding
        let query_embedding = self.ollama.embed(&self.embedding_model, query).await?;

        // Get all active entries
        let index = self.index.read().await;
        let entries = index.active_entries();

        // Compute similarities
        let mut results = Vec::new();
        for entry in entries {
            // Load embedding
            let embedding_data = tokio::fs::read_to_string(&entry.embedding_path).await?;
            let embedding: Vec<f32> = serde_json::from_str(&embedding_data)?;

            // Compute similarity
            let score = cosine_similarity(&query_embedding, &embedding);

            results.push(SearchResult::new(
                entry.doc_id.clone(),
                score,
                entry.metadata.clone(),
            ));
        }

        // Sort by score (descending)
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Return top k
        results.truncate(top_k);

        info!("Search completed - {} results", results.len());
        Ok(results)
    }

    /// Delete document from index
    pub async fn delete_document(&self, doc_id: &str) -> Result<()> {
        let mut index = self.index.write().await;
        index.delete_entry(doc_id);
        self.save_index(&index).await?;

        info!("Document deleted from index: {}", doc_id);
        Ok(())
    }

    /// Save index to file
    async fn save_index(&self, index: &VectorIndex) -> Result<()> {
        let data = serde_json::to_string_pretty(index)?;
        tokio::fs::write(&self.index_path, data).await?;
        Ok(())
    }

    /// Get index statistics
    pub async fn stats(&self) -> (usize, String) {
        let index = self.index.read().await;
        (index.count(), index.embedding_model.clone())
    }
}
