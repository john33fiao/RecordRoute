//! RecordRoute STT (Speech-to-Text) Engine
//!
//! Whisper.cpp based speech recognition module

pub mod audio;
pub mod postprocess;
pub mod types;
pub mod whisper;

// Re-export main types
pub use types::{Segment, Transcription, TranscriptionOptions};
pub use whisper::{WhisperEngine, GpuDevice};
