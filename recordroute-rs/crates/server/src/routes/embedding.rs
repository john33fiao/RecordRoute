use actix_web::{post, web, HttpResponse};
use std::fs;
use tracing::{error, info};

use crate::state::AppState;
use crate::types::IncrementalEmbeddingResponse;

/// Run incremental embedding on all STT files that don't have embeddings yet
#[post("/incremental_embedding")]
pub async fn incremental_embedding(
    state: web::Data<std::sync::Arc<AppState>>,
) -> actix_web::Result<HttpResponse> {
    info!("Starting incremental embedding");

    let mut processed_count = 0;

    // Get all active records
    let history = state.history.read().await;
    let records = history.get_active_records();

    // Find records with STT completed but embedding not done
    let records_to_process: Vec<_> = records
        .into_iter()
        .filter(|r| r.stt_done && !r.embed_done)
        .collect();

    drop(history); // Release read lock

    info!(
        "Found {} records requiring embedding",
        records_to_process.len()
    );

    // Process each record
    for record in records_to_process {
        let record_id = &record.id;

        // Read the transcript
        let transcript_path = state
            .config
            .db_base_path
            .join(record_id)
            .join("transcript.txt");

        if !transcript_path.exists() {
            error!(
                "Transcript file not found for record {}: {:?}",
                record_id, transcript_path
            );
            continue;
        }

        let transcript = match fs::read_to_string(&transcript_path) {
            Ok(content) => content,
            Err(e) => {
                error!("Failed to read transcript for {}: {}", record_id, e);
                continue;
            }
        };

        // Check if already has embedding in vector index
        // (This is a simplified check - in production you might want to check timestamps)
        let _has_embedding = state
            .vector_search
            .stats()
            .await
            .0
            > 0; // Simplified check - currently not used for conditional logic

        // Generate embedding
        info!("Generating embedding for record: {}", record_id);

        let metadata = recordroute_vector::VectorMetadata {
            filename: record.filename.clone(),
            file_path: format!("/download/{}", record_id),
            transcript_path: record.stt_path.clone(),
            summary_path: record.summary_path.clone(),
            one_line_summary: record.one_line_summary.clone(),
            timestamp: Some(record.timestamp),
            tags: record.tags.clone(),
        };

        match state
            .vector_search
            .add_document(record_id, &transcript, metadata)
            .await
        {
            Ok(_) => {
                // Update history record
                let mut history = state.history.write().await;
                if let Err(e) = history.update_record(record_id, |r| {
                    r.embed_done = true;
                }) {
                    error!("Failed to update record {}: {}", record_id, e);
                } else {
                    processed_count += 1;
                    info!("Successfully created embedding for record: {}", record_id);
                }
            }
            Err(e) => {
                error!("Failed to create embedding for {}: {}", record_id, e);
                continue;
            }
        }
    }

    info!(
        "Incremental embedding completed: {} files processed",
        processed_count
    );

    Ok(HttpResponse::Ok().json(IncrementalEmbeddingResponse {
        success: true,
        processed_count,
        message: format!("증분 임베딩 완료: {}개 파일 처리됨", processed_count),
    }))
}
