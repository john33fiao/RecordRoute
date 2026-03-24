use std::fmt::{Display, Formatter};
use std::str::FromStr;

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingStatus {
    Queued,
    Processing,
    Completed,
    Failed,
}

impl ProcessingStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl Display for ProcessingStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ProcessingStatus {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "processing" => Ok(Self::Processing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(anyhow!("unsupported processing status `{value}`")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingStep {
    UploadSaved,
    WavReady,
    Transcribed,
    Summarized,
    Embedded,
}

impl ProcessingStep {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UploadSaved => "upload_saved",
            Self::WavReady => "wav_ready",
            Self::Transcribed => "transcribed",
            Self::Summarized => "summarized",
            Self::Embedded => "embedded",
        }
    }
}

impl Display for ProcessingStep {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ProcessingStep {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "upload_saved" => Ok(Self::UploadSaved),
            "wav_ready" => Ok(Self::WavReady),
            "transcribed" => Ok(Self::Transcribed),
            "summarized" => Ok(Self::Summarized),
            "embedded" => Ok(Self::Embedded),
            _ => Err(anyhow!("unsupported processing step `{value}`")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredSummary {
    pub title: String,
    #[serde(rename = "abstract")]
    pub abstract_text: String,
    pub bullet_points: Vec<String>,
}

impl StructuredSummary {
    pub fn canonical_text(&self) -> String {
        let bullets = self
            .bullet_points
            .iter()
            .map(|bullet| format!("- {}", bullet.trim()))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "Title: {}\n\nAbstract:\n{}\n\nBullet Points:\n{}",
            self.title.trim(),
            self.abstract_text.trim(),
            bullets
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub id: Uuid,
    pub original_filename: String,
    pub original_content_type: Option<String>,
    pub file_size_bytes: i64,
    pub original_rel_path: String,
    pub wav_rel_path: Option<String>,
    pub language: Option<String>,
    pub transcript: Option<String>,
    pub summary: Option<StructuredSummary>,
    pub summary_canonical_text: Option<String>,
    pub status: ProcessingStatus,
    pub current_step: ProcessingStep,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub recording_id: Uuid,
    pub status: ProcessingStatus,
    pub step: ProcessingStep,
    pub attempt_count: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub next_attempt_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingBundle {
    pub recording: Recording,
    pub job: Option<Job>,
}

#[derive(Debug, Clone)]
pub struct NewRecording {
    pub id: Uuid,
    pub original_filename: String,
    pub original_content_type: Option<String>,
    pub file_size_bytes: i64,
    pub original_rel_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingListItem {
    pub id: Uuid,
    pub original_filename: String,
    pub status: ProcessingStatus,
    pub current_step: ProcessingStep,
    pub language: Option<String>,
    pub has_transcript: bool,
    pub has_summary: bool,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: Uuid,
    pub original_filename: String,
    pub status: ProcessingStatus,
    pub current_step: ProcessingStep,
    pub summary_excerpt: Option<String>,
    pub transcript_excerpt: Option<String>,
    pub keyword_hit: bool,
    pub keyword_score: f32,
    pub similarity_score: Option<f32>,
    pub created_at: DateTime<Utc>,
}
