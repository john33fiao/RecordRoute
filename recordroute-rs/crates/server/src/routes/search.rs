use actix_web::{get, web, HttpResponse};
use serde::Serialize;

use crate::state::AppState;
use crate::types::SearchQuery;

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
