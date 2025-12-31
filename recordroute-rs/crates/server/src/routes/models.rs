use actix_web::{get, HttpResponse};
use recordroute_common::model_manager::ModelManager;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::error;

#[derive(Debug, Serialize, Deserialize)]
struct ModelsResponse {
    models: Vec<String>,
    default: DefaultModels,
}

#[derive(Debug, Serialize, Deserialize)]
struct DefaultModels {
    whisper: String,
    summarize: String,
    embedding: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ErrorResponse {
    error: String,
    details: Option<String>,
}

/// GET /models - Get available GGUF models
#[get("/models")]
pub async fn get_models() -> HttpResponse {
    match get_available_models().await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => {
            error!("Failed to get models: {}", e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: "모델 목록을 조회할 수 없습니다".to_string(),
                details: Some(e.to_string()),
            })
        }
    }
}

async fn get_available_models() -> anyhow::Result<ModelsResponse> {
    let models_dir = ModelManager::default_models_dir();

    // Also check project root models directory
    let project_root = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    let project_models_dir = project_root.join("models");

    let mut models = Vec::new();

    // Scan default models directory
    if models_dir.exists() {
        scan_models_dir(&models_dir, &mut models).await?;
    }

    // Scan project models directory (if different from default)
    if project_models_dir != models_dir && project_models_dir.exists() {
        scan_models_dir(&project_models_dir, &mut models).await?;
    }

    // Remove duplicates and sort
    models.sort();
    models.dedup();

    Ok(ModelsResponse {
        models,
        default: DefaultModels {
            whisper: "large-v3-turbo".to_string(),
            summarize: std::env::var("DEFAULT_MODEL")
                .unwrap_or_else(|_| "gemma-3-4b-it".to_string()),
            embedding: std::env::var("EMBEDDING_MODEL")
                .unwrap_or_else(|_| "nomic-embed-text".to_string()),
        },
    })
}

async fn scan_models_dir(dir: &PathBuf, models: &mut Vec<String>) -> anyhow::Result<()> {
    // Use a stack-based approach instead of recursion to avoid boxing
    let mut dirs_to_scan = vec![dir.clone()];

    while let Some(current_dir) = dirs_to_scan.pop() {
        let mut entries = tokio::fs::read_dir(&current_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "gguf" {
                        if let Some(name) = path.file_name() {
                            let name_str = name.to_string_lossy().to_string();
                            // Skip mmproj files
                            if !name_str.to_lowercase().contains("mmproj") {
                                models.push(name_str);
                            }
                        }
                    }
                }
            } else if path.is_dir() {
                // Add subdirectory to scan queue
                dirs_to_scan.push(path);
            }
        }
    }

    Ok(())
}
