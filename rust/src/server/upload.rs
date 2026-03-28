use crate::error::{AppError, AppResult};
use crate::{audio_store::ImportedSource, index::SourceKind};
use axum::extract::Multipart;
use axum::extract::multipart::MultipartError;
use axum::extract::multipart::MultipartRejection;
use axum::http::StatusCode;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

const KIBIBYTE: usize = 1024;
const MEBIBYTE: usize = 1024 * KIBIBYTE;
pub(crate) const UPLOAD_FILE_MAX_BYTES: usize = 512 * MEBIBYTE;
const UPLOAD_MULTIPART_OVERHEAD_BYTES: usize = MEBIBYTE;
pub(crate) const DEFAULT_UPLOAD_LIMITS: UploadLimits = UploadLimits::new(
    UPLOAD_FILE_MAX_BYTES,
    UPLOAD_FILE_MAX_BYTES + UPLOAD_MULTIPART_OVERHEAD_BYTES,
);

#[derive(Debug, Clone, Copy)]
pub(crate) struct UploadLimits {
    pub file_max_bytes: usize,
    pub request_max_bytes: usize,
}

impl UploadLimits {
    pub(crate) const fn new(file_max_bytes: usize, request_max_bytes: usize) -> Self {
        Self {
            file_max_bytes,
            request_max_bytes,
        }
    }
}

#[derive(Debug)]
struct PendingUpload {
    temp_path: PathBuf,
    file_hash: String,
    file_name: Option<String>,
}

pub(crate) async fn persist_uploaded_file(
    repo_root: &Path,
    upload_limits: UploadLimits,
    multipart: Result<Multipart, MultipartRejection>,
) -> AppResult<ImportedSource> {
    let mut multipart = multipart.map_err(classify_multipart_rejection)?;
    let temp_dir = crate::audio_store::AudioStore::new(repo_root)
        .map_err(AppError::internal)?
        .spool_root()
        .join("uploads");
    fs::create_dir_all(&temp_dir).await.map_err(|error| {
        AppError::internal(format!(
            "failed to create upload directory {}: {error}",
            temp_dir.display()
        ))
    })?;

    let mut pending_upload = None;

    loop {
        let next_field = multipart
            .next_field()
            .await
            .map_err(|error| classify_multipart_error(error, upload_limits.file_max_bytes));
        let maybe_field = match next_field {
            Ok(field) => field,
            Err(error) => {
                cleanup_pending_upload(&mut pending_upload).await;
                return Err(error);
            }
        };
        let Some(mut field) = maybe_field else {
            break;
        };

        if field.name() != Some("file") {
            if let Err(error) = discard_field(&mut field, upload_limits.file_max_bytes).await {
                cleanup_pending_upload(&mut pending_upload).await;
                return Err(error);
            }
            continue;
        }

        if pending_upload.is_some() {
            cleanup_pending_upload(&mut pending_upload).await;
            return Err(AppError::bad_request(
                "multipart field 'file' must appear only once",
            ));
        }

        let upload =
            match stream_file_field(&temp_dir, &mut field, upload_limits.file_max_bytes).await {
                Ok(upload) => upload,
                Err(error) => {
                    cleanup_pending_upload(&mut pending_upload).await;
                    return Err(error);
                }
            };
        pending_upload = Some(upload);
    }

    let pending_upload = pending_upload
        .ok_or_else(|| AppError::bad_request("multipart field 'file' is required"))?;
    finalize_pending_upload(repo_root, pending_upload).await
}

pub(crate) fn upload_limit_message(file_max_bytes: usize) -> String {
    format!(
        "업로드 가능한 최대 파일 크기는 {}입니다.",
        format_size(file_max_bytes)
    )
}

async fn discard_field(
    field: &mut axum::extract::multipart::Field<'_>,
    file_max_bytes: usize,
) -> AppResult<()> {
    while field
        .chunk()
        .await
        .map_err(|error| classify_multipart_error(error, file_max_bytes))?
        .is_some()
    {}
    Ok(())
}

async fn stream_file_field(
    temp_dir: &Path,
    field: &mut axum::extract::multipart::Field<'_>,
    file_max_bytes: usize,
) -> AppResult<PendingUpload> {
    let temp_path = temp_dir.join(format!("upload-{}.part", Uuid::now_v7()));
    let file_name = field.file_name().map(str::to_string);
    let mut writer = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp_path)
        .await
        .map_err(|error| {
            AppError::internal(format!(
                "failed to create temporary upload file {}: {error}",
                temp_path.display()
            ))
        })?;

    let mut bytes_written = 0usize;
    let mut hasher = StableContentHasher::new();

    let result = async {
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|error| classify_multipart_error(error, file_max_bytes))?
        {
            bytes_written = bytes_written
                .checked_add(chunk.len())
                .ok_or_else(|| AppError::payload_too_large(upload_limit_message(file_max_bytes)))?;
            if bytes_written > file_max_bytes {
                return Err(AppError::payload_too_large(upload_limit_message(
                    file_max_bytes,
                )));
            }

            hasher.update(&chunk);
            writer.write_all(&chunk).await.map_err(|error| {
                AppError::internal(format!(
                    "failed to write temporary upload file {}: {error}",
                    temp_path.display()
                ))
            })?;
        }

        if bytes_written == 0 {
            return Err(AppError::bad_request("uploaded file is empty"));
        }

        writer.flush().await.map_err(|error| {
            AppError::internal(format!(
                "failed to flush temporary upload file {}: {error}",
                temp_path.display()
            ))
        })?;
        drop(writer);

        Ok(PendingUpload {
            temp_path: temp_path.clone(),
            file_hash: hasher.finish(),
            file_name,
        })
    }
    .await;

    if result.is_err() {
        cleanup_temp_file(&temp_path).await;
    }

    result
}

async fn finalize_pending_upload(
    repo_root: &Path,
    pending_upload: PendingUpload,
) -> AppResult<ImportedSource> {
    let _ = pending_upload.file_hash;
    let imported = crate::audio_store::AudioStore::new(repo_root)
        .and_then(|store| {
            store.publish_source(
                &pending_upload.temp_path,
                pending_upload.file_name.as_deref(),
                SourceKind::Upload,
            )
        })
        .map_err(AppError::internal);
    cleanup_temp_file(&pending_upload.temp_path).await;
    imported
}

async fn cleanup_pending_upload(pending_upload: &mut Option<PendingUpload>) {
    if let Some(upload) = pending_upload.take() {
        cleanup_temp_file(&upload.temp_path).await;
    }
}

async fn cleanup_temp_file(path: &Path) {
    match fs::remove_file(path).await {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(_) => {}
    }
}

fn classify_multipart_rejection(_: MultipartRejection) -> AppError {
    AppError::bad_request("invalid multipart body")
}

fn classify_multipart_error(error: MultipartError, file_max_bytes: usize) -> AppError {
    match error.status() {
        StatusCode::BAD_REQUEST => AppError::bad_request("invalid multipart body"),
        StatusCode::PAYLOAD_TOO_LARGE => {
            AppError::payload_too_large(upload_limit_message(file_max_bytes))
        }
        _ => AppError::internal(format!(
            "failed to parse multipart body: {}",
            error.body_text()
        )),
    }
}

fn format_size(bytes: usize) -> String {
    if bytes >= MEBIBYTE && bytes % MEBIBYTE == 0 {
        format!("{}MB", bytes / MEBIBYTE)
    } else if bytes >= KIBIBYTE && bytes % KIBIBYTE == 0 {
        format!("{}KB", bytes / KIBIBYTE)
    } else {
        format!("{bytes}B")
    }
}

struct StableContentHasher {
    hash: u64,
}

impl StableContentHasher {
    fn new() -> Self {
        Self {
            hash: 0xcbf29ce484222325,
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.hash ^= u64::from(*byte);
            self.hash = self.hash.wrapping_mul(0x100000001b3);
        }
    }

    fn finish(self) -> String {
        format!("{:016x}", self.hash)
    }
}
