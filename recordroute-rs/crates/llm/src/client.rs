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

    /// Generate text with Ollama (with retry logic)
    pub async fn generate(&self, request: GenerateRequest) -> Result<String> {
        self.generate_with_retry(request, 3).await
    }

    /// Generate text with custom retry count
    async fn generate_with_retry(&self, request: GenerateRequest, max_retries: u32) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url);

        debug!(
            "Sending generate request to Ollama - Model: {}, Prompt length: {}",
            request.model,
            request.prompt.len()
        );

        let mut last_error = None;

        for attempt in 1..=max_retries {
            match self.try_generate(&url, &request).await {
                Ok(response) => {
                    debug!(
                        "Received response from Ollama - Length: {}, Done: {}",
                        response.len(),
                        response.len() > 0
                    );
                    return Ok(response);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries {
                        let delay = std::time::Duration::from_secs(2u64.pow(attempt - 1));
                        tracing::warn!(
                            "Ollama request failed (attempt {}/{}): {}. Retrying in {:?}...",
                            attempt,
                            max_retries,
                            last_error.as_ref().unwrap(),
                            delay
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retries failed").into()))
    }

    /// Single attempt to generate text
    async fn try_generate(&self, url: &str, request: &GenerateRequest) -> Result<String> {
        let response = self
            .client
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send request: {}", e))?
            .error_for_status()
            .map_err(|e| anyhow::anyhow!("Ollama API error: {}", e))?;

        let result: GenerateResponse = response.json().await
            .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

        if result.response.is_empty() {
            return Err(anyhow::anyhow!("Empty response from Ollama").into());
        }

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

    /// Generate embedding for text (with retry logic)
    pub async fn embed(&self, model: impl Into<String>, text: impl Into<String>) -> Result<Vec<f32>> {
        self.embed_with_retry(model, text, 3).await
    }

    /// Generate embedding with custom retry count
    async fn embed_with_retry(&self, model: impl Into<String>, text: impl Into<String>, max_retries: u32) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.base_url);
        let text = text.into();
        let model = model.into();

        debug!("Generating embedding - Model: {}, Text length: {}", model, text.len());

        let request = EmbedRequest {
            model: model.clone(),
            prompt: text,
        };

        let mut last_error = None;

        for attempt in 1..=max_retries {
            match self.try_embed(&url, &request).await {
                Ok(embedding) => {
                    debug!("Received embedding - Dimension: {}", embedding.len());
                    return Ok(embedding);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries {
                        let delay = std::time::Duration::from_secs(2u64.pow(attempt - 1));
                        tracing::warn!(
                            "Embedding request failed (attempt {}/{}). Retrying in {:?}...",
                            attempt,
                            max_retries,
                            delay
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retries failed").into()))
    }

    /// Single attempt to generate embedding
    async fn try_embed(&self, url: &str, request: &EmbedRequest) -> Result<Vec<f32>> {
        let response = self
            .client
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send embedding request: {}", e))?
            .error_for_status()
            .map_err(|e| anyhow::anyhow!("Ollama embedding API error: {}", e))?;

        let result: EmbedResponse = response.json().await
            .map_err(|e| anyhow::anyhow!("Failed to parse embedding response: {}", e))?;

        if result.embedding.is_empty() {
            return Err(anyhow::anyhow!("Empty embedding from Ollama").into());
        }

        Ok(result.embedding)
    }
}
