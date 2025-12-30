//! RecordRoute LLM Integration
//!
//! Ollama API client and text summarization

mod chunking;
mod client;
mod summarize;
mod types;

pub use client::OllamaClient;
pub use chunking::{chunk_text, split_paragraphs, TextChunk};
pub use summarize::Summarizer;
pub use types::{GenerateOptions, GenerateRequest, GenerateResponse, Summary};
