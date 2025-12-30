use recordroute_common::Result;
use tracing::{debug, info};

use crate::chunking::chunk_text;
use crate::client::OllamaClient;
use crate::types::{GenerateOptions, GenerateRequest, Summary};

/// Summarizer for long text using map-reduce strategy
pub struct Summarizer {
    client: OllamaClient,
    model: String,
}

impl Summarizer {
    /// Create new summarizer
    pub fn new(client: OllamaClient, model: impl Into<String>) -> Self {
        Self {
            client,
            model: model.into(),
        }
    }

    /// Summarize long text using map-reduce
    pub async fn summarize(&self, text: &str) -> Result<Summary> {
        info!("Starting summarization - Text length: {} chars", text.len());

        // Step 1: Check if text is short enough for direct summarization
        if text.len() < 8000 {
            debug!("Text is short, using direct summarization");
            return self.summarize_direct(text).await;
        }

        // Step 2: Split into chunks (map phase)
        let chunks = chunk_text(text, 2000, 200);
        info!("Split text into {} chunks", chunks.len());

        // Step 3: Summarize each chunk
        let mut chunk_summaries = Vec::new();
        for (i, chunk) in chunks.iter().enumerate() {
            debug!("Summarizing chunk {}/{}", i + 1, chunks.len());
            let summary = self.summarize_chunk(&chunk.text).await?;
            chunk_summaries.push(summary);
        }

        // Step 4: Combine chunk summaries (reduce phase)
        let combined = chunk_summaries.join("\n\n");
        info!("Combined chunk summaries - Length: {} chars", combined.len());

        // Step 5: Final summarization
        let final_summary = self.summarize_direct(&combined).await?;

        Ok(final_summary)
    }

    /// Summarize a single chunk
    async fn summarize_chunk(&self, text: &str) -> Result<String> {
        let prompt = format!(
            "다음 텍스트를 핵심 내용을 포함하여 간결하게 요약해주세요. 중요한 정보는 빠뜨리지 마세요.\n\n텍스트:\n{}\n\n요약:",
            text
        );

        let request = GenerateRequest {
            model: self.model.clone(),
            prompt,
            stream: Some(false),
            options: Some(GenerateOptions {
                temperature: Some(0.3),
                top_p: Some(0.9),
                num_predict: Some(500),
            }),
        };

        self.client.generate(request).await
    }

    /// Direct summarization (for shorter texts)
    async fn summarize_direct(&self, text: &str) -> Result<Summary> {
        // Generate full summary
        let full_summary_prompt = format!(
            "다음 텍스트를 상세하게 요약해주세요. 주요 내용과 중요한 세부사항을 모두 포함하세요.\n\n텍스트:\n{}\n\n상세 요약:",
            text
        );

        let full_summary_request = GenerateRequest {
            model: self.model.clone(),
            prompt: full_summary_prompt,
            stream: Some(false),
            options: Some(GenerateOptions {
                temperature: Some(0.3),
                top_p: Some(0.9),
                num_predict: Some(1000),
            }),
        };

        let full_summary = self.client.generate(full_summary_request).await?;

        // Generate one-line summary
        let one_line_summary = self.generate_one_line(&full_summary).await?;

        Ok(Summary::new(
            full_summary,
            one_line_summary,
            self.model.clone(),
        ))
    }

    /// Generate one-line summary
    pub async fn generate_one_line(&self, text: &str) -> Result<String> {
        let prompt = format!(
            "다음 요약을 한 문장으로 압축해주세요. 가장 핵심적인 내용만 포함하세요.\n\n요약:\n{}\n\n한 줄 요약:",
            text
        );

        let request = GenerateRequest {
            model: self.model.clone(),
            prompt,
            stream: Some(false),
            options: Some(GenerateOptions {
                temperature: Some(0.2),
                top_p: Some(0.9),
                num_predict: Some(100),
            }),
        };

        let response = self.client.generate(request).await?;

        // Clean up the response (remove extra whitespace, newlines)
        Ok(response.trim().replace('\n', " "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summarizer_creation() {
        let client = OllamaClient::new("http://localhost:11434").unwrap();
        let summarizer = Summarizer::new(client, "llama3.2");
        assert_eq!(summarizer.model, "llama3.2");
    }
}
