use actix_web::{get, post, web, HttpResponse};
use serde::Serialize;
use std::fs;
use tracing::error;

use crate::state::AppState;
use crate::types::{SearchQuery, SimilarDocsRequest, SimilarDocItem};

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
    pub query: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct SearchResultItem {
    pub doc_id: String,
    pub score: f32,
    pub filename: String,
    pub one_line_summary: Option<String>,
    pub transcript_path: Option<String>,
    pub summary_path: Option<String>,
}

#[get("/search")]
pub async fn search(
    query: web::Query<SearchQuery>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> actix_web::Result<HttpResponse> {
    if query.q.trim().is_empty() {
        return Ok(HttpResponse::BadRequest().body("Query cannot be empty"));
    }

    // Perform vector search
    let search_results = state
        .vector_search
        .search(&query.q, query.top_k)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    // Convert to response format
    let results: Vec<SearchResultItem> = search_results
        .into_iter()
        .map(|r| SearchResultItem {
            doc_id: r.doc_id,
            score: r.score,
            filename: r.metadata.filename,
            one_line_summary: r.metadata.one_line_summary,
            transcript_path: r.metadata.transcript_path,
            summary_path: r.metadata.summary_path,
        })
        .collect();

    let count = results.len();

    Ok(HttpResponse::Ok().json(SearchResponse {
        results,
        query: query.q.clone(),
        count,
    }))
}

#[get("/search/stats")]
pub async fn search_stats(
    state: web::Data<std::sync::Arc<AppState>>,
) -> actix_web::Result<HttpResponse> {
    let (count, model) = state.vector_search.stats().await;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "total_documents": count,
        "embedding_model": model,
    })))
}

/// Find similar documents
#[post("/similar")]
pub async fn similar_documents(
    req: web::Json<SimilarDocsRequest>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> actix_web::Result<HttpResponse> {
    let file_path = &req.file_path;

    // Extract record ID from file path (remove /download/ prefix)
    let record_id = file_path.trim_start_matches("/download/");

    // Find the record
    let history = state.history.read().await;
    let record = match history.get_by_id(record_id) {
        Some(r) => r,
        None => {
            return Ok(HttpResponse::NotFound().json(serde_json::json!({
                "error": "Record not found"
            })));
        }
    };

    // Check if the record has STT done
    if !record.stt_done {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "STT not completed for this record"
        })));
    }

    // Read the transcript
    let transcript_path = state
        .config
        .db_base_path
        .join(record_id)
        .join("transcript.txt");

    let transcript = match fs::read_to_string(&transcript_path) {
        Ok(content) => content,
        Err(e) => {
            error!("Failed to read transcript: {}", e);
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to read transcript"
            })));
        }
    };

    drop(history); // Release read lock

    // Perform vector search using the transcript
    let search_results = state
        .vector_search
        .search(&transcript, 6) // Get top 6 (including self)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    // Filter out the current document and format results
    let similar_docs: Vec<SimilarDocItem> = search_results
        .into_iter()
        .filter(|r| r.doc_id != record_id) // Exclude self
        .take(5) // Top 5 similar documents
        .map(|r| {
            let link = format!("/download/{}", r.doc_id);
            SimilarDocItem {
                display_name: r.metadata.filename.clone(),
                file: r.doc_id.clone(),
                link,
                score: r.score,
                title_summary: r.metadata.one_line_summary,
            }
        })
        .collect();

    Ok(HttpResponse::Ok().json(similar_docs))
}
