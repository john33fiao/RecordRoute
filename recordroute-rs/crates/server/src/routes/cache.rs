use actix_web::{get, HttpResponse};
use tracing::info;

use crate::types::{CacheCleanupResponse, CacheStatsResponse};

/// Get cache statistics
#[get("/cache/stats")]
pub async fn cache_stats() -> actix_web::Result<HttpResponse> {
    // Note: Rust implementation uses vector index instead of separate cache
    // This endpoint provides stats on the vector index as a cache substitute

    info!("Getting cache statistics");

    // For now, return basic stats
    // In a full implementation, you would track actual cache metrics
    let response = CacheStatsResponse {
        total_entries: 0,
        cache_size_bytes: 0,
        expired_entries: 0,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Clean up expired cache entries
#[get("/cache/cleanup")]
pub async fn cache_cleanup() -> actix_web::Result<HttpResponse> {
    info!("Cleaning up expired cache entries");

    // Note: Rust implementation uses persistent vector index
    // No temporary cache to clean up in the current architecture
    // This is a no-op for compatibility with the Python API

    let response = CacheCleanupResponse {
        success: true,
        cleaned_entries: 0,
        message: "캐시 정리 완료 (Rust 구현은 영구 벡터 인덱스 사용)".to_string(),
    };

    Ok(HttpResponse::Ok().json(response))
}
