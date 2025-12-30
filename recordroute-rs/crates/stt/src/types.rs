use serde::{Deserialize, Serialize};

/// Single transcription segment with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    /// Start time in seconds
    pub start: f32,
    
    /// End time in seconds
    pub end: f32,
    
    /// Transcribed text
    pub text: String,
}

impl Segment {
    /// Create a new segment
    pub fn new(start: f32, end: f32, text: String) -> Self {
        Self { start, end, text }
    }
    
    /// Get duration in seconds
    pub fn duration(&self) -> f32 {
        self.end - self.start
    }
    
    /// Format timestamp as HH:MM:SS
    pub fn format_timestamp(seconds: f32) -> String {
        let seconds = seconds as u32;
        let h = seconds / 3600;
        let m = (seconds % 3600) / 60;
        let s = seconds % 60;
        format!("{:02}:{:02}:{:02}", h, m, s)
    }
    
    /// Get formatted time range string
    pub fn time_range(&self) -> String {
        format!(
            "{} - {}",
            Self::format_timestamp(self.start),
            Self::format_timestamp(self.end)
        )
    }
}

/// Complete transcription result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcription {
    /// Full transcribed text
    pub text: String,
    
    /// Individual segments with timestamps
    pub segments: Vec<Segment>,
    
    /// Detected language (ISO code)
    pub language: String,
}

impl Transcription {
    /// Create a new transcription
    pub fn new(text: String, segments: Vec<Segment>, language: String) -> Self {
        Self {
            text,
            segments,
            language,
        }
    }
    
    /// Create from full text only (no segments)
    pub fn from_text(text: String, language: String) -> Self {
        Self {
            text,
            segments: Vec::new(),
            language,
        }
    }
    
    /// Get total duration
    pub fn duration(&self) -> f32 {
        self.segments
            .last()
            .map(|seg| seg.end)
            .unwrap_or(0.0)
    }
    
    /// Export to markdown format
    pub fn to_markdown(&self, title: &str) -> String {
        let mut md = format!("# {}\n\n", title);
        
        if self.segments.is_empty() {
            // No segments, just full text
            md.push_str(&self.text);
        } else {
            // Format with timestamps
            for segment in &self.segments {
                md.push_str(&format!("[{}] {}\n", segment.time_range(), segment.text));
            }
        }
        
        md
    }
}

/// Transcription options
#[derive(Debug, Clone)]
pub struct TranscriptionOptions {
    /// Language hint (e.g., "ko", "en")
    pub language: Option<String>,
    
    /// Initial prompt for domain-specific terms
    pub initial_prompt: Option<String>,
    
    /// Temperature for sampling (0.0 = greedy)
    pub temperature: f32,
    
    /// Filter out filler words
    pub filter_fillers: bool,
    
    /// Minimum segment length (characters)
    pub min_segment_length: usize,
    
    /// Normalize punctuation
    pub normalize_punctuation: bool,
    
    /// No speech threshold
    pub no_speech_threshold: f32,
    
    /// Log probability threshold
    pub logprob_threshold: f32,
    
    /// Compression ratio threshold
    pub compression_ratio_threshold: f32,
}

impl Default for TranscriptionOptions {
    fn default() -> Self {
        Self {
            language: Some("ko".to_string()),
            initial_prompt: None,
            temperature: 0.0,
            filter_fillers: false,
            min_segment_length: 2,
            normalize_punctuation: true,
            no_speech_threshold: 0.6,
            logprob_threshold: -1.0,
            compression_ratio_threshold: 2.4,
        }
    }
}

impl TranscriptionOptions {
    /// Create new options with defaults
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set language
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }
    
    /// Set initial prompt
    pub fn with_initial_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.initial_prompt = Some(prompt.into());
        self
    }
    
    /// Enable filler filtering
    pub fn filter_fillers(mut self, enable: bool) -> Self {
        self.filter_fillers = enable;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_segment_creation() {
        let seg = Segment::new(0.0, 5.5, "Hello world".to_string());
        assert_eq!(seg.duration(), 5.5);
        assert_eq!(seg.time_range(), "00:00:00 - 00:00:05");
    }
    
    #[test]
    fn test_timestamp_format() {
        assert_eq!(Segment::format_timestamp(0.0), "00:00:00");
        assert_eq!(Segment::format_timestamp(65.0), "00:01:05");
        assert_eq!(Segment::format_timestamp(3661.0), "01:01:01");
    }
    
    #[test]
    fn test_transcription_markdown() {
        let segments = vec![
            Segment::new(0.0, 2.0, "First segment".to_string()),
            Segment::new(2.0, 5.0, "Second segment".to_string()),
        ];
        
        let transcription = Transcription::new(
            "First segment Second segment".to_string(),
            segments,
            "ko".to_string(),
        );
        
        let md = transcription.to_markdown("Test");
        assert!(md.contains("# Test"));
        assert!(md.contains("[00:00:00 - 00:00:02]"));
    }
}
