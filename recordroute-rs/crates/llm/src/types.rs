use serde::{Deserialize, Serialize};

/// Ollama generate request
#[derive(Debug, Clone, Serialize)]
pub struct GenerateRequest {
    /// Model name (e.g., "llama3.2", "gemma2")
    pub model: String,

    /// Prompt text
    pub prompt: String,

    /// Disable streaming
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    /// Generation options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<GenerateOptions>,
}

/// Generation options
#[derive(Debug, Clone, Serialize, Default)]
pub struct GenerateOptions {
    /// Temperature (0.0 - 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_predict: Option<i32>,
}

/// Ollama generate response
#[derive(Debug, Clone, Deserialize)]
pub struct GenerateResponse {
    /// Model name
    pub model: String,

    /// Generated text
    pub response: String,

    /// Whether generation is complete
    pub done: bool,

    /// Context (for multi-turn conversations)
    #[serde(default)]
    pub context: Vec<i32>,
}

/// Summarization result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    /// Full summary text
    pub text: String,

    /// One-line summary
    pub one_line: String,

    /// Model used
    pub model: String,
}

impl Summary {
    /// Create new summary
    pub fn new(text: String, one_line: String, model: String) -> Self {
        Self {
            text,
            one_line,
            model,
        }
    }
}
