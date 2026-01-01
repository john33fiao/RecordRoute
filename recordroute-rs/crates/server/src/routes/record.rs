use actix_web::{post, web, HttpResponse};
use std::fs;
use tracing::{error, info};

use crate::state::AppState;
use crate::types::{
    CheckExistingSttRequest, CheckExistingSttResponse, ResetAllTasksRequest,
    ResetRecordRequest, ResetSummaryEmbeddingRequest, SuccessResponse,
    UpdateFilenameRequest, UpdateSttTextRequest, UpdateSttTextResponse,
};

/// Update STT text manually
#[post("/update_stt_text")]
pub async fn update_stt_text(
    req: web::Json<UpdateSttTextRequest>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> actix_web::Result<HttpResponse> {
    let file_identifier = &req.file_identifier;
    let content = &req.content;

    info!("Updating STT text for file: {}", file_identifier);

    // Find the record by file identifier
    let history = state.history.read().await;
    let record = history
        .get_active_records()
        .into_iter()
        .find(|r| r.id == *file_identifier);

    let record_id = match record {
        Some(r) => r.id.clone(),
        None => {
            error!("Record not found: {}", file_identifier);
            return Ok(HttpResponse::NotFound().json(UpdateSttTextResponse {
                success: false,
                record_id: None,
            }));
        }
    };

    drop(history); // Release read lock

    // Construct STT file path
    let stt_path = state
        .config
        .db_base_path
        .join(&record_id)
        .join("transcript.txt");

    // Ensure directory exists
    if let Some(parent) = stt_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            error!("Failed to create directory: {}", e);
            return Ok(
                HttpResponse::InternalServerError().json(UpdateSttTextResponse {
                    success: false,
                    record_id: Some(record_id),
                }),
            );
        }
    }

    // Write STT content to file
    if let Err(e) = fs::write(&stt_path, content) {
        error!("Failed to write STT file: {}", e);
        return Ok(
            HttpResponse::InternalServerError().json(UpdateSttTextResponse {
                success: false,
                record_id: Some(record_id.clone()),
            }),
        );
    }

    // Update history record
    let mut history = state.history.write().await;
    let stt_path_str = stt_path.to_string_lossy().to_string();

    if let Err(e) = history.update_record(&record_id, |record| {
        record.stt_done = true;
        record.stt_path = Some(stt_path_str);
    }) {
        error!("Failed to update history record: {}", e);
        return Ok(
            HttpResponse::InternalServerError().json(UpdateSttTextResponse {
                success: false,
                record_id: Some(record_id),
            }),
        );
    }

    info!("Successfully updated STT text for record: {}", record_id);

    Ok(HttpResponse::Ok().json(UpdateSttTextResponse {
        success: true,
        record_id: Some(record_id),
    }))
}

/// Check if STT result exists
#[post("/check_existing_stt")]
pub async fn check_existing_stt(
    req: web::Json<CheckExistingSttRequest>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> actix_web::Result<HttpResponse> {
    let file_path = &req.file_path;

    // Extract record ID from file path
    let record_id = file_path.trim_start_matches("/download/");

    // Find the record
    let history = state.history.read().await;
    let record = history.get_by_id(record_id);

    let has_stt = record.map(|r| r.stt_done).unwrap_or(false);

    Ok(HttpResponse::Ok().json(CheckExistingSttResponse { has_stt }))
}

/// Reset record completely (remove STT, summary, embedding)
#[post("/reset")]
pub async fn reset_record(
    req: web::Json<ResetRecordRequest>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> actix_web::Result<HttpResponse> {
    let record_id = &req.record_id;

    info!("Resetting record: {}", record_id);

    // Update history record to clear all flags
    let mut history = state.history.write().await;

    if let Err(e) = history.update_record(record_id, |record| {
        record.stt_done = false;
        record.summarize_done = false;
        record.embed_done = false;
        record.stt_path = None;
        record.summary_path = None;
        record.one_line_summary = None;
    }) {
        error!("Failed to reset record: {}", e);
        return Ok(HttpResponse::InternalServerError().json(SuccessResponse {
            success: false,
            message: Some(format!("Failed to reset record: {}", e)),
        }));
    }

    // Delete output directory
    let output_dir = state.config.db_base_path.join(record_id);
    if output_dir.exists() {
        if let Err(e) = fs::remove_dir_all(&output_dir) {
            error!("Failed to remove output directory: {}", e);
        }
    }

    // Remove from vector index
    if let Err(e) = state.vector_search.delete_document(record_id).await {
        error!("Failed to remove from vector index: {}", e);
    }

    info!("Successfully reset record: {}", record_id);

    Ok(HttpResponse::Ok().json(SuccessResponse {
        success: true,
        message: Some("Record reset successfully".to_string()),
    }))
}

/// Reset only summary and embedding (keep STT)
#[post("/reset_summary_embedding")]
pub async fn reset_summary_embedding(
    req: web::Json<ResetSummaryEmbeddingRequest>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> actix_web::Result<HttpResponse> {
    let record_id = &req.record_id;

    info!("Resetting summary and embedding for record: {}", record_id);

    // Update history record
    let mut history = state.history.write().await;

    if let Err(e) = history.update_record(record_id, |record| {
        record.summarize_done = false;
        record.embed_done = false;
        record.summary_path = None;
        record.one_line_summary = None;
    }) {
        error!("Failed to reset summary/embedding: {}", e);
        return Ok(HttpResponse::InternalServerError().json(SuccessResponse {
            success: false,
            message: Some(format!("Failed to reset: {}", e)),
        }));
    }

    // Delete summary file
    let summary_path = state
        .config
        .db_base_path
        .join(record_id)
        .join("summary.txt");
    if summary_path.exists() {
        if let Err(e) = fs::remove_file(&summary_path) {
            error!("Failed to remove summary file: {}", e);
        }
    }

    // Remove from vector index
    if let Err(e) = state.vector_search.delete_document(record_id).await {
        error!("Failed to remove from vector index: {}", e);
    }

    info!(
        "Successfully reset summary and embedding for record: {}",
        record_id
    );

    Ok(HttpResponse::Ok().json(SuccessResponse {
        success: true,
        message: Some("Summary and embedding reset successfully".to_string()),
    }))
}

/// Update filename
#[post("/update_filename")]
pub async fn update_filename(
    req: web::Json<UpdateFilenameRequest>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> actix_web::Result<HttpResponse> {
    let record_id = &req.record_id;
    let new_filename = &req.filename;

    info!("Updating filename for record: {} to {}", record_id, new_filename);

    // Update history record
    let mut history = state.history.write().await;

    if let Err(e) = history.update_record(record_id, |record| {
        record.filename = new_filename.clone();
    }) {
        error!("Failed to update filename: {}", e);
        return Ok(HttpResponse::InternalServerError().json(SuccessResponse {
            success: false,
            message: Some(format!("Failed to update filename: {}", e)),
        }));
    }

    info!("Successfully updated filename for record: {}", record_id);

    Ok(HttpResponse::Ok().json(SuccessResponse {
        success: true,
        message: Some("Filename updated successfully".to_string()),
    }))
}

/// Reset all tasks (bulk operation)
#[post("/reset_all_tasks")]
pub async fn reset_all_tasks(
    req: web::Json<ResetAllTasksRequest>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> actix_web::Result<HttpResponse> {
    let tasks = &req.tasks;

    info!("Resetting all tasks: {:?}", tasks);

    let reset_stt = tasks.contains(&"stt".to_string());
    let reset_summary = tasks.contains(&"summary".to_string());
    let reset_embedding = tasks.contains(&"embedding".to_string());

    if !reset_stt && !reset_summary && !reset_embedding {
        return Ok(HttpResponse::BadRequest().json(SuccessResponse {
            success: false,
            message: Some("No tasks specified".to_string()),
        }));
    }

    // Get all active records
    let mut history = state.history.write().await;
    let records = history.get_active_records();
    let record_ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();

    let mut updated_count = 0;

    for record_id in &record_ids {
        // Update record flags
        if let Err(e) = history.update_record(record_id, |record| {
            if reset_stt {
                record.stt_done = false;
                record.stt_path = None;
            }
            if reset_summary {
                record.summarize_done = false;
                record.summary_path = None;
                record.one_line_summary = None;
            }
            if reset_embedding {
                record.embed_done = false;
            }
        }) {
            error!("Failed to update record {}: {}", record_id, e);
            continue;
        }

        // Delete files if needed
        let output_dir = state.config.db_base_path.join(record_id);

        if reset_stt {
            let stt_path = output_dir.join("transcript.txt");
            if stt_path.exists() {
                let _ = fs::remove_file(&stt_path);
            }
        }

        if reset_summary {
            let summary_path = output_dir.join("summary.txt");
            if summary_path.exists() {
                let _ = fs::remove_file(&summary_path);
            }
        }

        if reset_embedding {
            // Remove from vector index
            let _ = state.vector_search.delete_document(record_id).await;
        }

        updated_count += 1;
    }

    info!("Successfully reset {} records", updated_count);

    Ok(HttpResponse::Ok().json(SuccessResponse {
        success: true,
        message: Some(format!("Successfully reset {} records", updated_count)),
    }))
}
