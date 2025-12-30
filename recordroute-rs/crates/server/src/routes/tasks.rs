use actix_web::{get, post, web, HttpResponse};

use crate::state::AppState;
use crate::types::CancelTaskRequest;

#[get("/tasks")]
pub async fn get_tasks(state: web::Data<std::sync::Arc<AppState>>) -> actix_web::Result<HttpResponse> {
    let tasks = state.job_manager.get_tasks().await;
    Ok(HttpResponse::Ok().json(tasks))
}

#[post("/cancel")]
pub async fn cancel_task(
    req: web::Json<CancelTaskRequest>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> actix_web::Result<HttpResponse> {
    let cancelled = state.job_manager.cancel_task(&req.task_id).await;
    
    if cancelled {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": "Task cancelled"
        })))
    } else {
        Ok(HttpResponse::NotFound().body("Task not found"))
    }
}
