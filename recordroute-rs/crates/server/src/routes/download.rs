use actix_web::{get, web};
use actix_files::NamedFile;

use crate::state::AppState;

#[get("/download/{filename:.*}")]
pub async fn download(
    path: web::Path<String>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> actix_web::Result<NamedFile> {
    let filename = path.into_inner();
    
    // Try different directories
    let possible_paths = vec![
        state.config.upload_dir.join(&filename),
        state.config.db_base_path.join("whisper_output").join(&filename),
        state.config.db_base_path.join(&filename),
    ];

    for filepath in possible_paths {
        if filepath.exists() {
            return Ok(NamedFile::open(filepath)?);
        }
    }

    Err(actix_web::error::ErrorNotFound("File not found"))
}
