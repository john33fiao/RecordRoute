use recordroute_common::Result;
use std::path::Path;
use tracing::{info, warn};
use async_trait::async_trait;

use crate::types::GenerateRequest;
use crate::llm_trait::LlmClient;

/// llama.cpp based LLM client
///
/// TODO: Full implementation requires proper llama-cpp-2 API integration
/// Currently this is a placeholder. Use OllamaClient for production.
pub struct LlamaCppClient {
    model_path: String,
    #[allow(dead_code)]
    embedding_model_path: Option<String>,
    #[allow(dead_code)]
    n_ctx: u32,
    #[allow(dead_code)]
    n_threads: u32,
}

impl LlamaCppClient {
    /// Create new llama.cpp client
    pub fn new(
        model_path: impl Into<String>,
        embedding_model_path: Option<String>,
        n_ctx: u32,
        n_threads: u32,
    ) -> Result<Self> {
        let model_path = model_path.into();

        info!(
            "Initializing llama.cpp client (STUB) - Model: {}, Context: {}, Threads: {}",
            model_path, n_ctx, n_threads
        );

        warn!("LlamaCppClient is currently a stub. Use OllamaClient for production.");

        Ok(Self {
            model_path,
            embedding_model_path,
            n_ctx,
            n_threads,
        })
    }

    /// Generate text using llama.cpp (STUB)
    pub async fn generate(&self, _request: GenerateRequest) -> Result<String> {
        Err(anyhow::anyhow!(
            "LlamaCppClient is not fully implemented yet. Use OllamaClient instead."
        ).into())
    }

    /// Generate embedding using llama.cpp (STUB)
    pub async fn embed(&self, _model: &str, _text: &str) -> Result<Vec<f32>> {
        Err(anyhow::anyhow!(
            "LlamaCppClient is not fully implemented yet. Use OllamaClient instead."
        ).into())
    }

    /// Test if model file exists
    fn test_connection_internal(&self) -> Result<bool> {
        let path = Path::new(&self.model_path);
        Ok(path.exists() && path.is_file())
    }
}

#[async_trait]
impl LlmClient for LlamaCppClient {
    async fn generate(&self, request: GenerateRequest) -> Result<String> {
        self.generate(request).await
    }

    async fn embed(&self, model: &str, text: &str) -> Result<Vec<f32>> {
        self.embed(model, text).await
    }

    fn test_connection(&self) -> Result<bool> {
        self.test_connection_internal()
    }
}
