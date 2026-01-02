use async_trait::async_trait;
use recordroute_common::Result;
use std::path::Path;
use tracing::{info, warn};

use crate::llm_trait::LlmClient;
use crate::types::GenerateRequest;

/// GPU backend used for llama.cpp
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlamaBackend {
    /// NVIDIA GPU via CUDA
    Cuda,
    /// Apple GPU via Metal
    Metal,
    /// CPU execution
    Cpu,
}

fn detect_backend() -> LlamaBackend {
    if cfg!(feature = "cuda") {
        LlamaBackend::Cuda
    } else if cfg!(feature = "metal") {
        LlamaBackend::Metal
    } else {
        LlamaBackend::Cpu
    }
}

/// llama.cpp based LLM client
///
/// TODO: Full implementation requires proper llama-cpp-2 API integration
/// Currently this is a placeholder. Use OllamaClient for production.
pub struct LlamaCppClient {
    model_path: String,
    #[allow(dead_code)] // Reserved for future llama.cpp implementation
    embedding_model_path: Option<String>,
    #[allow(dead_code)] // Reserved for future llama.cpp implementation
    n_ctx: u32,
    #[allow(dead_code)] // Reserved for future llama.cpp implementation
    n_threads: u32,
    backend: LlamaBackend,
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
        let backend = detect_backend();

        info!(
            "Initializing llama.cpp client (STUB) - Model: {}, Context: {}, Threads: {}, Backend: {:?}",
            model_path, n_ctx, n_threads, backend
        );

        warn!("LlamaCppClient is currently a stub. Use OllamaClient for production.");

        Ok(Self {
            model_path,
            embedding_model_path,
            n_ctx,
            n_threads,
            backend,
        })
    }

    /// Generate text using llama.cpp (STUB)
    pub async fn generate(&self, _request: GenerateRequest) -> Result<String> {
        Err(anyhow::anyhow!(
            "LlamaCppClient is not fully implemented yet. Use OllamaClient instead."
        )
        .into())
    }

    /// Generate embedding using llama.cpp (STUB)
    pub async fn embed(&self, _model: &str, _text: &str) -> Result<Vec<f32>> {
        Err(anyhow::anyhow!(
            "LlamaCppClient is not fully implemented yet. Use OllamaClient instead."
        )
        .into())
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
