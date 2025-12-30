use recordroute_common::Result;
use tracing::{debug, info};

use crate::chunking::chunk_text;
use crate::client::OllamaClient;
use crate::prompts::{chunk_prompt, one_line_prompt, reduce_prompt};
use crate::types::{GenerateOptions, GenerateRequest, Summary};

/// Batch size for hierarchical reduce (Python uses 10)
const BATCH_REDUCE_SIZE: usize = 10;

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
            info!("Summarizing chunk {}/{}", i + 1, chunks.len());
            let summary = self.summarize_chunk(&chunk.text).await?;
            chunk_summaries.push(summary);
        }

        info!("All chunks summarized - {} chunk summaries", chunk_summaries.len());

        // Step 4: Reduce phase with batch processing
        let final_text = if chunk_summaries.len() > BATCH_REDUCE_SIZE {
            info!(
                "Using batch reduce - {} summaries, batch size: {}",
                chunk_summaries.len(),
                BATCH_REDUCE_SIZE
            );
            self.batch_reduce(chunk_summaries).await?
        } else {
            // Direct reduce for smaller number of chunks
            let combined = chunk_summaries.join("\n\n---청크 요약 구분선---\n\n");
            info!("Combined chunk summaries - Length: {} chars", combined.len());
            let prompt = reduce_prompt(&combined);
            self.generate_with_options(&prompt, 1000).await?
        };

        // Step 5: Generate one-line summary
        let one_line_summary = self.generate_one_line(&final_text).await?;

        Ok(Summary::new(
            final_text,
            one_line_summary,
            self.model.clone(),
        ))
    }

    /// Batch reduce: hierarchical summarization for many chunks
    async fn batch_reduce(&self, chunk_summaries: Vec<String>) -> Result<String> {
        let num_chunks = chunk_summaries.len();
        let num_batches = (num_chunks + BATCH_REDUCE_SIZE - 1) / BATCH_REDUCE_SIZE;

        info!(
            "1st level reduce: {} chunks -> {} batches (size: {})",
            num_chunks, num_batches, BATCH_REDUCE_SIZE
        );

        // 1st level reduce: group chunks into batches
        let mut batch_summaries = Vec::new();
        for (batch_idx, batch_chunk) in chunk_summaries.chunks(BATCH_REDUCE_SIZE).enumerate() {
            info!(
                "Processing 1st level batch {}/{} ({} chunks)",
                batch_idx + 1,
                num_batches,
                batch_chunk.len()
            );

            let batch_combined = batch_chunk.join("\n\n---청크 요약 구분선---\n\n");
            let prompt = reduce_prompt(&batch_combined);
            let batch_summary = self.generate_with_options(&prompt, 1000).await?;
            batch_summaries.push(batch_summary);
        }

        info!("1st level reduce complete - {} batch summaries", batch_summaries.len());

        // 2nd level reduce: combine batch summaries
        info!("2nd level reduce: combining {} batch summaries", batch_summaries.len());
        let final_combined = batch_summaries.join("\n\n---배치 요약 구분선---\n\n");
        let final_prompt = reduce_prompt(&final_combined);
        let final_summary = self.generate_with_options(&final_prompt, 1000).await?;

        info!("Batch reduce complete");
        Ok(final_summary)
    }

    /// Summarize a single chunk
    async fn summarize_chunk(&self, text: &str) -> Result<String> {
        let prompt = chunk_prompt(text);
        self.generate_with_options(&prompt, 500).await
    }

    /// Direct summarization (for shorter texts)
    async fn summarize_direct(&self, text: &str) -> Result<Summary> {
        // Generate full summary using structured prompt
        let prompt = chunk_prompt(text);
        let full_summary = self.generate_with_options(&prompt, 1000).await?;

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
        let prompt = one_line_prompt(text);
        let response = self.generate_with_options(&prompt, 100).await?;

        // Clean up the response (remove extra whitespace, newlines)
        Ok(response.trim().replace('\n', " "))
    }

    /// Helper: Generate text with common options
    async fn generate_with_options(&self, prompt: &str, max_tokens: i32) -> Result<String> {
        let request = GenerateRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: Some(false),
            options: Some(GenerateOptions {
                temperature: Some(0.3),
                top_p: Some(0.9),
                num_predict: Some(max_tokens),
            }),
        };

        self.client.generate(request).await
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
