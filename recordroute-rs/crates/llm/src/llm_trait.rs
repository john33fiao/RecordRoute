use recordroute_common::Result;
use crate::types::GenerateRequest;
use async_trait::async_trait;

/// Common trait for LLM clients
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Generate text from a prompt
    async fn generate(&self, request: GenerateRequest) -> Result<String>;

    /// Generate embedding for text
    async fn embed(&self, model: &str, text: &str) -> Result<Vec<f32>>;

    /// Test connection/availability
    fn test_connection(&self) -> Result<bool>;
}
