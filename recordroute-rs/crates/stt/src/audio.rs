//! Audio preprocessing and conversion
//!
//! Handles audio format conversion, resampling, and normalization

use recordroute_common::{RecordRouteError, Result};
use std::path::Path;
use tracing::info;

/// Supported audio file extensions
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "flac", "m4a", "mp3", "mp4", "mpeg", "mpga", "oga", "ogg", "qta", "wav", "webm",
];

/// Check if file extension is supported
pub fn is_supported_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Audio buffer (mono, 16kHz, f32 samples)
pub struct AudioBuffer {
    /// Audio samples normalized to [-1.0, 1.0]
    pub samples: Vec<f32>,
    
    /// Sample rate in Hz
    pub sample_rate: u32,
    
    /// Number of channels
    pub channels: u16,
}

impl AudioBuffer {
    /// Create a new audio buffer
    pub fn new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        Self {
            samples,
            sample_rate,
            channels,
        }
    }
    
    /// Get duration in seconds
    pub fn duration(&self) -> f32 {
        self.samples.len() as f32 / self.sample_rate as f32
    }
    
    /// Convert to mono by averaging channels
    pub fn to_mono(mut self) -> Self {
        if self.channels == 1 {
            return self;
        }
        
        info!("Converting {} channel audio to mono", self.channels);
        
        let channels = self.channels as usize;
        let num_frames = self.samples.len() / channels;
        let mut mono_samples = Vec::with_capacity(num_frames);
        
        for frame_idx in 0..num_frames {
            let mut sum = 0.0;
            for ch in 0..channels {
                sum += self.samples[frame_idx * channels + ch];
            }
            mono_samples.push(sum / channels as f32);
        }
        
        self.samples = mono_samples;
        self.channels = 1;
        self
    }
    
    /// Resample to target sample rate
    ///
    /// Note: This is a simple linear interpolation resampler.
    /// For production use, consider using a proper resampling library.
    pub fn resample(mut self, target_rate: u32) -> Self {
        if self.sample_rate == target_rate {
            return self;
        }
        
        info!("Resampling from {}Hz to {}Hz", self.sample_rate, target_rate);
        
        let ratio = self.sample_rate as f64 / target_rate as f64;
        let new_length = (self.samples.len() as f64 / ratio) as usize;
        let mut resampled = Vec::with_capacity(new_length);
        
        for i in 0..new_length {
            let src_index = i as f64 * ratio;
            let src_index_floor = src_index.floor() as usize;
            let src_index_ceil = (src_index_floor + 1).min(self.samples.len() - 1);
            let fraction = src_index - src_index_floor as f64;
            
            // Linear interpolation
            let sample = self.samples[src_index_floor] * (1.0 - fraction) as f32
                + self.samples[src_index_ceil] * fraction as f32;
            
            resampled.push(sample);
        }
        
        self.samples = resampled;
        self.sample_rate = target_rate;
        self
    }
}

/// Convert audio file to WAV using FFmpeg
///
/// # Arguments
/// * `input_path` - Input audio/video file
/// * `output_path` - Output WAV file path
///
/// # Returns
/// Path to the converted WAV file
pub fn convert_to_wav_ffmpeg(
    input_path: &Path,
    output_path: &Path,
) -> Result<()> {
    use std::process::Command;
    
    info!("Converting {} to WAV using FFmpeg", input_path.display());
    
    // Get FFmpeg path from environment or use default
    let ffmpeg_cmd = std::env::var("FFMPEG_PATH").unwrap_or_else(|_| "ffmpeg".to_string());
    
    let output = Command::new(&ffmpeg_cmd)
        .args(&[
            "-i", &input_path.to_string_lossy(),
            "-ar", "16000",      // 16kHz sample rate
            "-ac", "1",          // Mono
            "-c:a", "pcm_s16le", // 16-bit PCM
            "-y",                // Overwrite output
            &output_path.to_string_lossy(),
        ])
        .output()
        .map_err(|e| RecordRouteError::stt(format!("Failed to run FFmpeg: {}", e)))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RecordRouteError::stt(format!(
            "FFmpeg conversion failed: {}",
            stderr
        )));
    }
    
    info!("FFmpeg conversion successful: {}", output_path.display());
    
    Ok(())
}

/// Extract audio from video file
pub fn extract_audio_from_video(
    video_path: &Path,
    output_audio_path: &Path,
) -> Result<()> {
    convert_to_wav_ffmpeg(video_path, output_audio_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_is_supported_audio() {
        assert!(is_supported_audio(Path::new("test.wav")));
        assert!(is_supported_audio(Path::new("test.mp3")));
        assert!(is_supported_audio(Path::new("test.m4a")));
        assert!(!is_supported_audio(Path::new("test.txt")));
        assert!(!is_supported_audio(Path::new("test.rs")));
    }
    
    #[test]
    fn test_audio_buffer_duration() {
        let buffer = AudioBuffer::new(vec![0.0; 16000], 16000, 1);
        assert_eq!(buffer.duration(), 1.0);
        
        let buffer = AudioBuffer::new(vec![0.0; 8000], 16000, 1);
        assert_eq!(buffer.duration(), 0.5);
    }
    
    #[test]
    fn test_to_mono() {
        // Stereo -> Mono
        let samples = vec![0.5, -0.5, 0.5, -0.5]; // 2 frames, 2 channels
        let buffer = AudioBuffer::new(samples, 16000, 2);
        
        let mono = buffer.to_mono();
        assert_eq!(mono.channels, 1);
        assert_eq!(mono.samples.len(), 2);
        assert_eq!(mono.samples[0], 0.0); // average of 0.5 and -0.5
    }
    
    #[test]
    fn test_resample() {
        let samples = vec![0.0; 44100]; // 1 second at 44.1kHz
        let buffer = AudioBuffer::new(samples, 44100, 1);
        
        let resampled = buffer.resample(16000);
        assert_eq!(resampled.sample_rate, 16000);
        // Should be approximately 16000 samples
        assert!((resampled.samples.len() as i32 - 16000).abs() < 100);
    }
}
