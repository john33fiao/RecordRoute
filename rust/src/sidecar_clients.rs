use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use reqwest::multipart;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::config::AppConfig;
use crate::models::StructuredSummary;

#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
    pub language: Option<String>,
    pub raw_response: Value,
}

#[async_trait]
pub trait TranscriptionClient: Send + Sync {
    async fn transcribe(&self, wav_bytes: Vec<u8>) -> Result<TranscriptionResult>;
}

#[async_trait]
pub trait SummaryClient: Send + Sync {
    async fn summarize(&self, transcript: &str) -> Result<StructuredSummary>;
}

#[async_trait]
pub trait EmbeddingClient: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

pub struct HttpWhisperClient {
    http: Client,
    base_url: String,
    model_path: String,
    loaded: Mutex<bool>,
}

impl HttpWhisperClient {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            http: build_client(config.sidecar_timeout),
            base_url: trim_base_url(&config.whisper_base_url),
            model_path: config.whisper_model.clone(),
            loaded: Mutex::new(false),
        }
    }

    async fn ensure_loaded(&self) -> Result<()> {
        let mut loaded = self.loaded.lock().await;
        if *loaded {
            return Ok(());
        }

        let form = multipart::Form::new().text("model", self.model_path.clone());
        let response = self
            .http
            .post(format!("{}/load", self.base_url))
            .multipart(form)
            .send()
            .await
            .with_context(|| "failed to call whisper /load")?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("whisper /load failed: {body}");
        }

        *loaded = true;
        Ok(())
    }
}

#[async_trait]
impl TranscriptionClient for HttpWhisperClient {
    async fn transcribe(&self, wav_bytes: Vec<u8>) -> Result<TranscriptionResult> {
        self.ensure_loaded().await?;

        let form = multipart::Form::new()
            .text("response_format", "vjson")
            .part(
                "file",
                multipart::Part::bytes(wav_bytes)
                    .file_name("audio.wav")
                    .mime_str("audio/wav")?,
            );

        let response = self
            .http
            .post(format!("{}/inference", self.base_url))
            .multipart(form)
            .send()
            .await
            .with_context(|| "failed to call whisper /inference")?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("whisper /inference failed: {body}");
        }

        let raw_response = response.json::<Value>().await?;
        let text = raw_response
            .get("text")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("whisper response did not include non-empty text"))?
            .to_string();
        let language = raw_response
            .get("language")
            .and_then(Value::as_str)
            .map(ToString::to_string);

        Ok(TranscriptionResult {
            text,
            language,
            raw_response,
        })
    }
}

pub struct HttpLlamaSummaryClient {
    http: Client,
    base_url: String,
    model: String,
}

impl HttpLlamaSummaryClient {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            http: build_client(config.sidecar_timeout),
            base_url: trim_base_url(&config.llama_summary_base_url),
            model: config.summary_model.clone(),
        }
    }
}

#[async_trait]
impl SummaryClient for HttpLlamaSummaryClient {
    async fn summarize(&self, transcript: &str) -> Result<StructuredSummary> {
        let payload = json!({
            "model": self.model,
            "temperature": 0.1,
            "max_tokens": 512,
            "response_format": {
                "type": "json_object",
                "schema": {
                    "type": "object",
                    "required": ["title", "abstract", "bullet_points"],
                    "additionalProperties": false,
                    "properties": {
                        "title": { "type": "string", "minLength": 1, "maxLength": 160 },
                        "abstract": { "type": "string", "minLength": 1, "maxLength": 1200 },
                        "bullet_points": {
                            "type": "array",
                            "minItems": 3,
                            "maxItems": 8,
                            "items": { "type": "string", "minLength": 1, "maxLength": 280 }
                        }
                    }
                }
            },
            "messages": [
                {
                    "role": "system",
                    "content": "You summarize speech transcripts into a compact factual JSON object. Output JSON only. Do not invent facts. Use concise Korean or transcript language when obvious."
                },
                {
                    "role": "user",
                    "content": format!(
                        "Create a structured summary for the following transcript. Return only JSON with keys title, abstract, bullet_points.\n\nTranscript:\n{}",
                        transcript
                    )
                }
            ]
        });

        let response = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&payload)
            .send()
            .await
            .with_context(|| "failed to call llama summary server")?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("llama summary request failed: {body}");
        }

        let completion = response.json::<ChatCompletionResponse>().await?;
        let content = completion
            .choices
            .first()
            .ok_or_else(|| anyhow!("llama summary response had no choices"))?
            .message
            .content_as_string()?;

        let summary = serde_json::from_str::<StructuredSummary>(&content)
            .with_context(|| format!("failed to parse summary JSON: {content}"))?;

        if summary.title.trim().is_empty() || summary.abstract_text.trim().is_empty() {
            bail!("summary response contained empty title or abstract");
        }
        if summary.bullet_points.iter().all(|item| item.trim().is_empty()) {
            bail!("summary response contained no bullet points");
        }

        Ok(StructuredSummary {
            title: summary.title.trim().to_string(),
            abstract_text: summary.abstract_text.trim().to_string(),
            bullet_points: summary
                .bullet_points
                .into_iter()
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect(),
        })
    }
}

pub struct HttpLlamaEmbeddingClient {
    http: Client,
    base_url: String,
    model: String,
}

impl HttpLlamaEmbeddingClient {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            http: build_client(config.sidecar_timeout),
            base_url: trim_base_url(&config.llama_embed_base_url),
            model: config.embed_model.clone(),
        }
    }
}

#[async_trait]
impl EmbeddingClient for HttpLlamaEmbeddingClient {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let payload = json!({
            "model": self.model,
            "input": text,
            "encoding_format": "float"
        });

        let response = self
            .http
            .post(format!("{}/v1/embeddings", self.base_url))
            .json(&payload)
            .send()
            .await
            .with_context(|| "failed to call llama embedding server")?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("llama embedding request failed: {body}");
        }

        let embeddings = response.json::<EmbeddingResponse>().await?;
        let vector = embeddings
            .data
            .first()
            .ok_or_else(|| anyhow!("embedding response had no data"))?
            .embedding
            .clone();

        if vector.is_empty() {
            bail!("embedding response returned an empty vector");
        }

        Ok(vector)
    }
}

fn build_client(timeout: Duration) -> Client {
    Client::builder()
        .timeout(timeout)
        .build()
        .expect("reqwest client build should succeed")
}

fn trim_base_url(value: &str) -> String {
    value.trim_end_matches('/').to_string()
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Value,
}

impl ChatMessage {
    fn content_as_string(&self) -> Result<String> {
        match &self.content {
            Value::String(value) => Ok(value.clone()),
            Value::Array(values) => {
                let text = values
                    .iter()
                    .filter_map(|item| item.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n");
                if text.is_empty() {
                    bail!("chat completion content array did not contain text blocks");
                }
                Ok(text)
            }
            Value::Object(_) => Ok(self.content.to_string()),
            _ => bail!("unsupported chat completion content format"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingDatum>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingDatum {
    embedding: Vec<f32>,
}
