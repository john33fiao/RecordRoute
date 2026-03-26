use crate::app::{self, submit_ffmpeg_job};
use axum::body::Bytes;
use axum::http::HeaderMap;
use std::fs;
use std::path::Path;

pub(crate) struct UploadedFile {
    pub bytes: Vec<u8>,
}

pub(crate) fn persist_uploaded_file_and_submit(
    repo_root: &Path,
    bytes: &[u8],
) -> Result<app::FfmpegJobSubmission, String> {
    let uploads_dir = repo_root.join("db").join("uploads");
    fs::create_dir_all(&uploads_dir).map_err(|error| {
        format!(
            "failed to create upload directory {}: {error}",
            uploads_dir.display()
        )
    })?;

    let file_hash = stable_content_hash(bytes);
    let upload_path = uploads_dir.join(format!("{file_hash}.bin"));
    if !upload_path.is_file() {
        fs::write(&upload_path, bytes)
            .map_err(|error| format!("failed to persist uploaded file: {error}"))?;
    }

    submit_ffmpeg_job(repo_root, &upload_path)
}

pub(crate) fn parse_uploaded_file(
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<UploadedFile, String> {
    let content_type = headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| "missing content-type header".to_string())?;
    if !content_type.starts_with("multipart/form-data") {
        return Err("content-type must be multipart/form-data".to_string());
    }

    let boundary = parse_boundary(content_type)?;
    let marker = format!("--{boundary}").into_bytes();
    let separator = {
        let mut value = b"\r\n".to_vec();
        value.extend_from_slice(&marker);
        value
    };

    let mut position = 0usize;
    let mut file_bytes = None;

    while let Some(marker_index) = find_subsequence(&body[position..], &marker) {
        position += marker_index + marker.len();

        if body
            .get(position..position + 2)
            .is_some_and(|suffix| suffix == b"--")
        {
            break;
        }

        if body
            .get(position..position + 2)
            .is_none_or(|suffix| suffix != b"\r\n")
        {
            return Err("invalid multipart body format".to_string());
        }
        position += 2;

        let header_end_offset = find_subsequence(&body[position..], b"\r\n\r\n")
            .ok_or_else(|| "invalid multipart body headers".to_string())?;
        let header_end = position + header_end_offset;
        let header_text = std::str::from_utf8(&body[position..header_end])
            .map_err(|_| "invalid multipart header encoding".to_string())?;
        position = header_end + 4;

        let content_end_offset = find_subsequence(&body[position..], &separator)
            .ok_or_else(|| "invalid multipart body content terminator".to_string())?;
        let content_end = position + content_end_offset;
        let content = body[position..content_end].to_vec();
        position = content_end;

        if !header_text.contains("name=\"file\"") {
            continue;
        }
        if file_bytes.is_some() {
            return Err("multipart field 'file' must appear only once".to_string());
        }
        if content.is_empty() {
            return Err("uploaded file is empty".to_string());
        }
        file_bytes = Some(content);
    }

    let bytes = file_bytes.ok_or_else(|| "multipart field 'file' is required".to_string())?;
    Ok(UploadedFile { bytes })
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_boundary(content_type: &str) -> Result<String, String> {
    for piece in content_type.split(';').map(str::trim) {
        if let Some(boundary) = piece.strip_prefix("boundary=") {
            let boundary = boundary.trim_matches('"').to_string();
            if boundary.is_empty() {
                return Err("multipart boundary is empty".to_string());
            }
            return Ok(boundary);
        }
    }
    Err("multipart boundary is missing".to_string())
}

fn stable_content_hash(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
