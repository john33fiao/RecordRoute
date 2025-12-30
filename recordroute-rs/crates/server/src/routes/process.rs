use actix_web::{post, web, HttpResponse};

use crate::state::AppState;
use crate::types::{ProcessRequest, ProcessResponse};

#[post("/process")]
pub async fn process(
    req: web::Json<ProcessRequest>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> actix_web::Result<HttpResponse> {
    let task_type = if req.run_stt {
        "stt"
    } else if req.run_summarize {
        "summary"
    } else if req.run_embed {
        "embedding"
    } else {
        return Ok(HttpResponse::BadRequest().body("No task specified"));
    };

    let task_id = state
        .job_manager
        .create_task(task_type.to_string(), req.file_uuid.clone())
        .await;

    // TODO: Actually run the workflow
    // For now, just create the task

    Ok(HttpResponse::Ok().json(ProcessResponse {
        task_id,
        message: format!("Task created for {}", task_type),
    }))
}
