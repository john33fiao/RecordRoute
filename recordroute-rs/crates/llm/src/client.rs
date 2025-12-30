use recordroute_common::Result;
use reqwest::Client;
use tracing::{debug, info};

use crate::types::{EmbedRequest, EmbedResponse, GenerateRequest, GenerateResponse};

/// Ollama API client
#[derive(Debug, Clone)]
pub struct OllamaClient {
    base_url: String,
    client: Client,
}

impl OllamaClient {
    /// Create new Ollama client
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let base_url = base_url.into();
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300)) // 5 minutes for LLM calls
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;

        info!("Ollama client initialized: {}", base_url);
        Ok(Self { base_url, client })
    }

    /// Generate text with Ollama
    pub async fn generate(&self, request: GenerateRequest) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url);

        debug!(
            "Sending generate request to Ollama - Model: {}, Prompt length: {}",
            request.model,
            request.prompt.len()
        );

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send request: {}", e))?
            .error_for_status()
            .map_err(|e| anyhow::anyhow!("Ollama API error: {}", e))?;

        let result: GenerateResponse = response.json().await
            .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

        debug!(
            "Received response from Ollama - Length: {}, Done: {}",
            result.response.len(),
            result.done
        );

        Ok(result.response)
    }

    /// Generate with streaming support (returns final response)
    pub async fn generate_stream(&self, request: GenerateRequest) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url);

        let mut request = request;
        request.stream = Some(true);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send streaming request: {}", e))?
            .error_for_status()
            .map_err(|e| anyhow::anyhow!("Ollama API error: {}", e))?;

        let body = response.text().await
            .map_err(|e| anyhow::anyhow!("Failed to read response body: {}", e))?;

        // Parse NDJSON (newline-delimited JSON)
        let mut final_response = String::new();
        for line in body.lines() {
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(chunk) = serde_json::from_str::<GenerateResponse>(line) {
                final_response.push_str(&chunk.response);
                if chunk.done {
                    break;
                }
            }
        }

        Ok(final_response)
    }

    /// Test connection to Ollama
    pub async fn test_connection(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.base_url);

        let response = self.client.get(&url).send().await
            .map_err(|e| anyhow::anyhow!("Failed to connect to Ollama: {}", e))?;
        Ok(response.status().is_success())
    }

    /// Generate embedding for text
    pub async fn embed(&self, model: impl Into<String>, text: impl Into<String>) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.base_url);
        let text = text.into();
        let model = model.into();

        debug!("Generating embedding - Model: {}, Text length: {}", model, text.len());

        let request = EmbedRequest {
            model,
            prompt: text,
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send embedding request: {}", e))?
            .error_for_status()
            .map_err(|e| anyhow::anyhow!("Ollama embedding API error: {}", e))?;

        let result: EmbedResponse = response.json().await
            .map_err(|e| anyhow::anyhow!("Failed to parse embedding response: {}", e))?;

        debug!("Received embedding - Dimension: {}", result.embedding.len());

        Ok(result.embedding)
    }
}
