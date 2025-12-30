use actix_web::{get, post, web, HttpResponse};

use crate::state::AppState;
use crate::types::DeleteRequest;

#[get("/history")]
pub async fn get_history(state: web::Data<std::sync::Arc<AppState>>) -> actix_web::Result<HttpResponse> {
    let records = state.history.read().await.get_active_records();
    Ok(HttpResponse::Ok().json(records))
}

#[post("/delete")]
pub async fn delete_history(
    req: web::Json<DeleteRequest>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> actix_web::Result<HttpResponse> {
    state.history.write().await.delete_records(&req.ids)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Records deleted successfully"
    })))
}
