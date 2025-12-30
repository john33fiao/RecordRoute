use actix_multipart::Multipart;
use actix_web::{post, web, HttpResponse};
use futures_util::StreamExt;
use std::io::Write;
use uuid::Uuid;

use crate::state::AppState;
use crate::types::{HistoryRecord, UploadResponse};

#[post("/upload")]
pub async fn upload(
    mut payload: Multipart,
    state: web::Data<std::sync::Arc<AppState>>,
) -> actix_web::Result<HttpResponse> {
    let mut filename = String::new();
    let file_uuid = Uuid::new_v4().to_string();
    let mut saved_path = None;

    while let Some(field) = payload.next().await {
        let mut field = field?;
        let content_disposition = field.content_disposition();

        if let Some(name) = content_disposition.get_name() {
            if name == "file" {
                filename = content_disposition
                    .get_filename()
                    .unwrap_or("unknown")
                    .to_string();

                let file_ext = std::path::Path::new(&filename)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                
                let save_filename = format!("{}.{}", file_uuid, file_ext);
                let filepath = state.config.upload_dir.join(&save_filename);

                let mut f = std::fs::File::create(&filepath)?;

                while let Some(chunk) = field.next().await {
                    let data = chunk?;
                    f.write_all(&data)?;
                }

                saved_path = Some(filepath.to_string_lossy().to_string());
            }
        }
    }

    if let Some(path) = saved_path {
        let record = HistoryRecord::new(file_uuid.clone(), filename.clone());
        state.history.write().await.add_record(record)
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

        Ok(HttpResponse::Ok().json(UploadResponse {
            file_uuid,
            filename,
            path,
        }))
    } else {
        Ok(HttpResponse::BadRequest().body("No file uploaded"))
    }
}
