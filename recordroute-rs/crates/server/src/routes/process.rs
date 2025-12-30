use actix_web::{post, web, HttpResponse};

use crate::state::AppState;
use crate::types::{ProcessRequest, ProcessResponse};
use crate::workflow::WorkflowOptions;

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

    // Spawn workflow execution in background
    let state_clone = state.clone();
    let req_clone = req.into_inner();
    let task_id_clone = task_id.clone();

    tokio::spawn(async move {
        // Find the uploaded file
        let file_path = state_clone.config.upload_dir.join(&req_clone.file_uuid);

        // Check if file exists with common extensions
        let possible_files = vec![
            file_path.with_extension("mp3"),
            file_path.with_extension("wav"),
            file_path.with_extension("m4a"),
            file_path.with_extension("mp4"),
            file_path.clone(),
        ];

        let actual_file = possible_files
            .into_iter()
            .find(|p| p.exists());

        match actual_file {
            Some(file_path) => {
                let options = WorkflowOptions {
                    run_stt: req_clone.run_stt,
                    run_summarize: req_clone.run_summarize,
                    run_embed: req_clone.run_embed,
                    stt_model: req_clone.stt_model,
                    summary_model: req_clone.summary_model,
                    language: None,
                };

                match state_clone
                    .workflow
                    .execute(&req_clone.file_uuid, &file_path, options, &task_id_clone)
                    .await
                {
                    Ok(_result) => {
                        state_clone.job_manager.complete_task(&task_id_clone).await;
                    }
                    Err(e) => {
                        state_clone
                            .job_manager
                            .fail_task(&task_id_clone, e.to_string())
                            .await;
                    }
                }
            }
            None => {
                state_clone
                    .job_manager
                    .fail_task(&task_id_clone, format!("File not found: {}", req_clone.file_uuid))
                    .await;
            }
        }
    });

    Ok(HttpResponse::Ok().json(ProcessResponse {
        task_id,
        message: format!("Task started for {}", task_type),
    }))
}
