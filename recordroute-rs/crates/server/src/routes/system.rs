use actix_web::{post, HttpResponse};
use tracing::info;

use crate::types::SuccessResponse;

/// Shutdown server
#[post("/shutdown")]
pub async fn shutdown() -> actix_web::Result<HttpResponse> {
    info!("Server shutdown requested");

    // In Actix-web, we can't directly shutdown the server from a route handler.
    // The shutdown should be handled by the Electron app killing the process.
    // We'll just return success and let the Electron app handle the actual shutdown.

    Ok(HttpResponse::Ok().json(SuccessResponse {
        success: true,
        message: Some("Shutdown signal received. Server will terminate.".to_string()),
    }))
}
