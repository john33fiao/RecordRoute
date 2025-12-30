//! RecordRoute LLM Integration
//!
//! Ollama API 클라이언트 및 텍스트 요약 기능
//! Phase 3에서 구현 예정

use recordroute_common::Result;

/// Ollama API 클라이언트 (추후 구현)
pub struct OllamaClient {
    base_url: String,
}

impl OllamaClient {
    /// 새 Ollama 클라이언트 생성 (스텁)
    pub fn new(base_url: String) -> Result<Self> {
        Ok(Self { base_url })
    }
}
