//! RecordRoute LLM Integration
//!
//! Ollama API client and text summarization

mod chunking;
mod client;
mod prompts;
mod summarize;
mod types;

pub use client::OllamaClient;
pub use chunking::{chunk_text, split_paragraphs, TextChunk};
pub use prompts::{chunk_prompt, one_line_prompt, reduce_prompt, BASE_PROMPT};
pub use summarize::Summarizer;
pub use types::{EmbedRequest, EmbedResponse, GenerateOptions, GenerateRequest, GenerateResponse, Summary};
