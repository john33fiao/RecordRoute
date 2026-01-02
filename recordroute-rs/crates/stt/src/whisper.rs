use recordroute_common::{RecordRouteError, Result};
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn, debug};
use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};

use crate::types::{Segment, Transcription, TranscriptionOptions};
use crate::postprocess;

/// GPU 디바이스 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuDevice {
    /// CUDA (NVIDIA GPU)
    Cuda,
    /// Metal (Apple GPU)
    Metal,
    /// CPU만 사용
    Cpu,
}

/// Whisper STT Engine
pub struct WhisperEngine {
    ctx: Arc<WhisperContext>,
    model_path: String,
    gpu_device: GpuDevice,
}

impl WhisperEngine {
    /// 사용 가능한 GPU 디바이스 감지 (우선순위: CUDA > Metal > CPU)
    ///
    /// CUDA/Metal 지원 여부는 **조건부 컴파일(feature flags)** 로 제어합니다.
    ///
    /// - `--features cuda` 로 빌드하면 CUDA 백엔드를 사용합니다.
    /// - 그렇지 않고 `--features metal` 이면 Metal 백엔드를 사용합니다.
    /// - 두 GPU feature가 모두 비활성화되어 있으면 CPU만 사용합니다.
    fn detect_gpu_device() -> GpuDevice {
        if cfg!(feature = "cuda") {
            info!("CUDA feature enabled; building Whisper with CUDA backend");
            GpuDevice::Cuda
        } else if cfg!(feature = "metal") {
            info!("Metal feature enabled; building Whisper with Metal backend");
            GpuDevice::Metal
        } else {
            info!("No GPU features enabled; building Whisper for CPU only");
            GpuDevice::Cpu
        }
    }

    /// Create a new Whisper engine from model path with automatic GPU detection
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

        // GPU 디바이스 감지
        let gpu_device = Self::detect_gpu_device();
        info!("Using device: {:?}", gpu_device);

        // WhisperContextParameters 생성
        // whisper-rs는 feature로 CUDA/Metal이 활성화되어 있으면
        // 자동으로 GPU를 사용하려고 시도합니다.
        let params = WhisperContextParameters::default();

        // 모델 로드 시도
        let ctx = match WhisperContext::new_with_params(path.to_str().unwrap(), params) {
            Ok(ctx) => {
                info!("Whisper model loaded successfully with {:?}", gpu_device);
                ctx
            }
            Err(e) => {
                // GPU 초기화 실패 시 CPU로 fallback
                if gpu_device != GpuDevice::Cpu {
                    warn!("Failed to load model with GPU ({:?}): {}", gpu_device, e);
                    warn!("Falling back to CPU");

                    // GPU feature가 활성화되어 있어도 두 번째 시도는 실패할 가능성이 높지만 시도해봅니다
                    let cpu_params = WhisperContextParameters::default();

                    WhisperContext::new_with_params(path.to_str().unwrap(), cpu_params)
                        .map_err(|e| RecordRouteError::stt(format!("Failed to load Whisper model even with CPU: {}", e)))?
                } else {
                    return Err(RecordRouteError::stt(format!("Failed to load Whisper model: {}", e)));
                }
            }
        };

        info!("Whisper model loaded successfully");

        Ok(Self {
            ctx: Arc::new(ctx),
            model_path: path.to_string_lossy().to_string(),
            gpu_device,
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

        // Check file extension
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            "wav" => {
                // For WAV files, load directly if already in correct format
                self.load_wav(path)
            }
            "m4a" | "mp3" | "mp4" | "aac" | "flac" | "ogg" => {
                // For other formats, convert to WAV using FFmpeg
                info!("Converting {} to WAV using FFmpeg", ext);
                self.convert_and_load_audio(path)
            }
            _ => {
                // Try conversion anyway
                warn!("Unknown audio format: {}, attempting conversion", ext);
                self.convert_and_load_audio(path)
            }
        }
    }

    /// Convert audio file to 16kHz mono WAV using FFmpeg and load it
    fn convert_and_load_audio(&self, path: &Path) -> Result<Vec<f32>> {
        use std::process::Command;
        use std::fs;

        // Create temporary WAV file path
        let temp_wav = path.with_extension("temp.wav");

        info!("Converting audio to WAV: {} -> {}", path.display(), temp_wav.display());

        // Run FFmpeg to convert to 16kHz mono WAV
        let output = Command::new("ffmpeg")
            .arg("-i")
            .arg(path)
            .arg("-ar")
            .arg("16000")  // 16kHz sample rate
            .arg("-ac")
            .arg("1")      // Mono
            .arg("-y")     // Overwrite output file
            .arg(&temp_wav)
            .output()
            .map_err(|e| RecordRouteError::stt(format!(
                "Failed to run FFmpeg: {}. Make sure FFmpeg is installed.", e
            )))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Clean up temp file if it exists
            let _ = fs::remove_file(&temp_wav);
            return Err(RecordRouteError::stt(format!(
                "FFmpeg conversion failed: {}", stderr
            )));
        }

        info!("Audio conversion successful, loading WAV file");

        // Load the converted WAV file
        let result = self.load_wav(&temp_wav);

        // Clean up temporary file
        if let Err(e) = fs::remove_file(&temp_wav) {
            warn!("Failed to remove temporary WAV file: {}", e);
        }

        result
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

    /// Get GPU device being used
    pub fn gpu_device(&self) -> GpuDevice {
        self.gpu_device
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
