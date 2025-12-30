//! Model Manager for automatic model downloading
//!
//! Handles automatic downloading and verification of Whisper models

use crate::Result;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

/// Whisper model information
#[derive(Debug, Clone)]
pub struct WhisperModel {
    /// Model name (e.g., "base", "small", "medium")
    pub name: String,

    /// File size in bytes
    pub size: u64,

    /// SHA256 hash for verification
    pub sha256: Option<String>,

    /// Download URL
    pub url: String,
}

impl WhisperModel {
    /// Get model filename
    pub fn filename(&self) -> String {
        format!("ggml-{}.bin", self.name)
    }

    /// Get size in MB
    pub fn size_mb(&self) -> f64 {
        self.size as f64 / 1024.0 / 1024.0
    }
}

/// Available Whisper models
pub fn available_whisper_models() -> Vec<WhisperModel> {
    let base_url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

    vec![
        WhisperModel {
            name: "tiny".to_string(),
            size: 75 * 1024 * 1024, // 75 MB
            sha256: None,
            url: format!("{}/ggml-tiny.bin", base_url),
        },
        WhisperModel {
            name: "base".to_string(),
            size: 142 * 1024 * 1024, // 142 MB
            sha256: None,
            url: format!("{}/ggml-base.bin", base_url),
        },
        WhisperModel {
            name: "small".to_string(),
            size: 466 * 1024 * 1024, // 466 MB
            sha256: None,
            url: format!("{}/ggml-small.bin", base_url),
        },
        WhisperModel {
            name: "medium".to_string(),
            size: 1500 * 1024 * 1024, // 1.5 GB
            sha256: None,
            url: format!("{}/ggml-medium.bin", base_url),
        },
        WhisperModel {
            name: "large-v3".to_string(),
            size: 3100 * 1024 * 1024, // 3.1 GB
            sha256: None,
            url: format!("{}/ggml-large-v3.bin", base_url),
        },
    ]
}

/// Model Manager
pub struct ModelManager {
    models_dir: PathBuf,
    client: Client,
}

impl ModelManager {
    /// Create new model manager
    pub fn new(models_dir: PathBuf) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(3600)) // 1 hour for large downloads
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;

        Ok(Self { models_dir, client })
    }

    /// Get default models directory
    pub fn default_models_dir() -> PathBuf {
        // Check environment variable first
        if let Ok(dir) = std::env::var("RECORDROUTE_MODELS_DIR") {
            return PathBuf::from(dir);
        }

        // Use platform-specific cache directory
        #[cfg(target_os = "linux")]
        {
            if let Some(home) = std::env::var_os("HOME") {
                return PathBuf::from(home)
                    .join(".cache/recordroute/models");
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(home) = std::env::var_os("HOME") {
                return PathBuf::from(home)
                    .join("Library/Caches/recordroute/models");
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
                return PathBuf::from(local_app_data)
                    .join("recordroute\\models");
            }
        }

        // Fallback
        PathBuf::from("models")
    }

    /// Ensure Whisper model exists, download if missing
    pub async fn ensure_whisper_model(&self, model_name: &str) -> Result<PathBuf> {
        let model_path = self.models_dir.join(format!("ggml-{}.bin", model_name));

        if model_path.exists() {
            info!("Model already exists: {}", model_path.display());
            return Ok(model_path);
        }

        info!("Model not found, downloading: {}", model_name);

        // Find model info
        let models = available_whisper_models();
        let model_info = models
            .iter()
            .find(|m| m.name == model_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown model: {}", model_name))?;

        self.download_model(model_info, &model_path).await?;

        Ok(model_path)
    }

    /// Download model from URL
    pub async fn download_model(&self, model: &WhisperModel, dest: &Path) -> Result<()> {
        info!(
            "Downloading {} ({:.1} MB) from {}",
            model.filename(),
            model.size_mb(),
            model.url
        );

        // Create directory
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Check disk space (simplified check)
        // TODO: Implement proper disk space check

        // Create progress bar
        let pb = ProgressBar::new(model.size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("#>-"),
        );

        // Download with progress
        let response = self.client.get(&model.url).send().await
            .map_err(|e| anyhow::anyhow!("Failed to download: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Download failed with status: {}",
                response.status()
            ).into());
        }

        // Write to temporary file first
        let temp_path = dest.with_extension("tmp");
        let mut file = fs::File::create(&temp_path).await?;
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        use futures::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| anyhow::anyhow!("Download error: {}", e))?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
        }

        pb.finish_with_message("Download complete");
        file.sync_all().await?;
        drop(file);

        // Verify file size
        let metadata = fs::metadata(&temp_path).await?;
        if metadata.len() < model.size / 2 {
            fs::remove_file(&temp_path).await?;
            return Err(anyhow::anyhow!(
                "Downloaded file is too small ({} bytes, expected ~{} bytes)",
                metadata.len(),
                model.size
            ).into());
        }

        // Rename to final destination
        fs::rename(&temp_path, dest).await?;

        info!("Download successful: {}", dest.display());

        Ok(())
    }

    /// Verify model integrity
    pub async fn verify_model(&self, path: &Path, expected_hash: Option<&str>) -> Result<bool> {
        if !path.exists() {
            return Ok(false);
        }

        // If no hash provided, just check file exists
        let Some(expected) = expected_hash else {
            return Ok(true);
        };

        info!("Verifying model: {}", path.display());

        // Calculate SHA256
        let data = fs::read(path).await?;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let hash = format!("{:x}", hasher.finalize());

        Ok(hash == expected)
    }

    /// List installed models
    pub async fn list_installed_models(&self) -> Result<Vec<String>> {
        if !self.models_dir.exists() {
            return Ok(Vec::new());
        }

        let mut models = Vec::new();
        let mut entries = fs::read_dir(&self.models_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("bin") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    if name.starts_with("ggml-") {
                        models.push(name.trim_start_matches("ggml-").to_string());
                    }
                }
            }
        }

        Ok(models)
    }

    /// Clean unused models (interactive)
    pub async fn clean_unused_models(&self, keep_models: &[String]) -> Result<usize> {
        let installed = self.list_installed_models().await?;
        let mut removed = 0;

        for model in installed {
            if !keep_models.contains(&model) {
                let path = self.models_dir.join(format!("ggml-{}.bin", model));
                warn!("Removing unused model: {}", model);
                fs::remove_file(&path).await?;
                removed += 1;
            }
        }

        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_available_models() {
        let models = available_whisper_models();
        assert!(models.len() >= 5);
        assert!(models.iter().any(|m| m.name == "base"));
    }

    #[test]
    fn test_model_filename() {
        let model = WhisperModel {
            name: "base".to_string(),
            size: 142 * 1024 * 1024,
            sha256: None,
            url: "".to_string(),
        };
        assert_eq!(model.filename(), "ggml-base.bin");
    }

    #[test]
    fn test_default_models_dir() {
        let dir = ModelManager::default_models_dir();
        assert!(!dir.to_string_lossy().is_empty());
    }
}
