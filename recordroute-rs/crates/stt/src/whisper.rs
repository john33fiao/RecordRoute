use recordroute_common::{RecordRouteError, Result};
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn, debug};
use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};

use crate::types::{Segment, Transcription, TranscriptionOptions};
use crate::postprocess;

/// Whisper STT Engine
pub struct WhisperEngine {
    ctx: Arc<WhisperContext>,
    model_path: String,
}

impl WhisperEngine {
    /// Create a new Whisper engine from model path
    ///
    /// # Arguments
    /// * `model_path` - Path to the Whisper model file (.bin or .gguf)
    ///
    /// # Example
    /// ```no_run
    /// use recordroute_stt::WhisperEngine;
    ///
    /// let engine = WhisperEngine::new("models/ggml-base.bin").unwrap();
    /// ```
    pub fn new(model_path: impl AsRef<Path>) -> Result<Self> {
        let path = model_path.as_ref();
        
        if !path.exists() {
            return Err(RecordRouteError::stt(format!(
                "Model file not found: {}",
                path.display()
            )));
        }
        
        info!("Loading Whisper model from: {}", path.display());
        
        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(path.to_str().unwrap(), params)
            .map_err(|e| RecordRouteError::stt(format!("Failed to load Whisper model: {}", e)))?;
        
        info!("Whisper model loaded successfully");
        
        Ok(Self {
            ctx: Arc::new(ctx),
            model_path: path.to_string_lossy().to_string(),
        })
    }
    
    /// Transcribe an audio file
    ///
    /// # Arguments
    /// * `audio_path` - Path to the audio file
    /// * `options` - Transcription options
    ///
    /// # Returns
    /// Transcription result with text and segments
    pub fn transcribe(
        &self,
        audio_path: impl AsRef<Path>,
        options: &TranscriptionOptions,
    ) -> Result<Transcription> {
        let path = audio_path.as_ref();
        
        if !path.exists() {
            return Err(RecordRouteError::stt(format!(
                "Audio file not found: {}",
                path.display()
            )));
        }
        
        info!("Transcribing audio file: {}", path.display());
        
        // Load audio data
        let audio_data = self.load_audio(path)?;
        
        // Create transcription parameters
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        
        // Set language
        if let Some(lang) = &options.language {
            params.set_language(Some(lang.as_str()));
        }
        
        // Set initial prompt
        // Note: whisper-rs v0.10 doesn't expose set_initial_prompt
        // This can be added in future versions
        if let Some(_prompt) = &options.initial_prompt {
            // TODO: Add initial prompt support when available
        }

        // Set thresholds
        params.set_temperature(options.temperature);
        params.set_no_speech_thold(options.no_speech_threshold);
        
        // Enable/disable options
        params.set_print_special(false);
        params.set_print_progress(true);
        params.set_print_realtime(false);
        params.set_print_timestamps(true);
        
        // Create a new state for this transcription
        let mut state = self.ctx.create_state()
            .map_err(|e| RecordRouteError::stt(format!("Failed to create Whisper state: {}", e)))?;
        
        // Run transcription
        debug!("Starting Whisper inference...");
        state.full(params, &audio_data)
            .map_err(|e| RecordRouteError::stt(format!("Transcription failed: {}", e)))?;
        
        // Extract results
        let num_segments = state.full_n_segments()
            .map_err(|e| RecordRouteError::stt(format!("Failed to get segment count: {}", e)))?;
        
        debug!("Transcription complete, {} segments found", num_segments);
        
        let mut segments = Vec::new();
        let mut full_text = String::new();
        
        for i in 0..num_segments {
            let segment_text = state.full_get_segment_text(i)
                .map_err(|e| RecordRouteError::stt(format!("Failed to get segment text: {}", e)))?;
            
            let start = state.full_get_segment_t0(i)
                .map_err(|e| RecordRouteError::stt(format!("Failed to get segment start time: {}", e)))?;
            
            let end = state.full_get_segment_t1(i)
                .map_err(|e| RecordRouteError::stt(format!("Failed to get segment end time: {}", e)))?;
            
            // Convert from centiseconds to seconds
            let start_sec = start as f32 / 100.0;
            let end_sec = end as f32 / 100.0;
            
            // Post-process text
            let processed_text = postprocess::process_segment_text(
                &segment_text,
                options.filter_fillers,
                options.min_segment_length,
                options.normalize_punctuation,
            );
            
            // Skip empty or filtered segments
            if processed_text.is_empty() {
                continue;
            }
            
            full_text.push_str(&processed_text);
            full_text.push(' ');
            
            segments.push(Segment::new(start_sec, end_sec, processed_text));
        }
        
        // Detect language
        let language = options.language.clone().unwrap_or_else(|| "unknown".to_string());
        
        // Merge consecutive segments with same text
        let segments = postprocess::merge_segments(segments, 0.2);
        
        info!("Transcription successful: {} segments, {} characters", 
              segments.len(), full_text.len());
        
        Ok(Transcription::new(full_text.trim().to_string(), segments, language))
    }
    
    /// Load and preprocess audio file
    ///
    /// Converts audio to 16kHz mono PCM format required by Whisper
    fn load_audio(&self, path: &Path) -> Result<Vec<f32>> {
        info!("Loading audio file: {}", path.display());
        
        // For now, use a simple WAV loader
        // TODO: Phase 2.2 - Implement full audio preprocessing with symphonia
        
        // Check file extension
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        
        match ext.as_str() {
            "wav" => self.load_wav(path),
            _ => Err(RecordRouteError::stt(format!(
                "Unsupported audio format: {}. Currently only WAV is supported. \
                 Use FFmpeg to convert to 16kHz mono WAV first.",
                ext
            ))),
        }
    }
    
    /// Load WAV file
    fn load_wav(&self, path: &Path) -> Result<Vec<f32>> {
        use std::fs::File;
        use std::io::BufReader;
        
        // Simple WAV header parsing (44 bytes)
        let file = File::open(path)
            .map_err(|e| RecordRouteError::stt(format!("Failed to open WAV file: {}", e)))?;
        
        let mut reader = BufReader::new(file);
        
        // Read WAV header
        use std::io::Read;
        let mut header = [0u8; 44];
        reader.read_exact(&mut header)
            .map_err(|e| RecordRouteError::stt(format!("Failed to read WAV header: {}", e)))?;
        
        // Verify RIFF header
        if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
            return Err(RecordRouteError::stt("Invalid WAV file format".to_string()));
        }
        
        // Read sample rate (bytes 24-27)
        let sample_rate = u32::from_le_bytes([header[24], header[25], header[26], header[27]]);
        let num_channels = u16::from_le_bytes([header[22], header[23]]);
        let bits_per_sample = u16::from_le_bytes([header[34], header[35]]);
        
        debug!("WAV file info: {}Hz, {} channels, {} bits", 
               sample_rate, num_channels, bits_per_sample);
        
        // Read remaining audio data as i16 PCM
        let mut pcm_data = Vec::new();
        reader.read_to_end(&mut pcm_data)
            .map_err(|e| RecordRouteError::stt(format!("Failed to read audio data: {}", e)))?;
        
        // Convert to f32 samples
        let num_samples = pcm_data.len() / 2;
        let mut samples = Vec::with_capacity(num_samples);
        
        for i in 0..num_samples {
            let sample_bytes = [pcm_data[i * 2], pcm_data[i * 2 + 1]];
            let sample = i16::from_le_bytes(sample_bytes);
            // Normalize to [-1.0, 1.0]
            samples.push(sample as f32 / 32768.0);
        }
        
        // If stereo, convert to mono by averaging channels
        if num_channels == 2 {
            let mut mono_samples = Vec::with_capacity(samples.len() / 2);
            for i in (0..samples.len()).step_by(2) {
                let left = samples[i];
                let right = samples.get(i + 1).copied().unwrap_or(left);
                mono_samples.push((left + right) / 2.0);
            }
            samples = mono_samples;
        }
        
        // Resample if needed (Whisper requires 16kHz)
        if sample_rate != 16000 {
            warn!("Audio sample rate is {}Hz, resampling to 16kHz is recommended", sample_rate);
            // TODO: Implement proper resampling in Phase 2.2
        }
        
        info!("Loaded {} audio samples", samples.len());
        
        Ok(samples)
    }
    
    /// Get model path
    pub fn model_path(&self) -> &str {
        &self.model_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_engine_creation_with_missing_model() {
        let result = WhisperEngine::new("nonexistent_model.bin");
        assert!(result.is_err());
    }
}
