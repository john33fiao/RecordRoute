use super::types::AppState;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use std::ffi::OsStr;
use std::path::{Component, Path as FsPath, PathBuf};

const INDEX_HTML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/index.html"));
const APP_JS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/app.js"));
const APP_CSS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/app.css"));
const FRONTEND_BUILD_DIR: &str = "frontend/build";

pub(crate) async fn get_index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

pub(crate) async fn get_app_js() -> Response {
    asset_response("text/javascript; charset=utf-8", APP_JS)
}

pub(crate) async fn get_app_css() -> Response {
    asset_response("text/css; charset=utf-8", APP_CSS)
}

pub(crate) async fn get_new_index(State(state): State<AppState>) -> Response {
    serve_frontend_file(&state.repo_root, None).await
}

pub(crate) async fn get_new_file(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Response {
    serve_frontend_file(&state.repo_root, Some(path)).await
}

fn asset_response(content_type: &'static str, body: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(content_type)),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        ],
        body,
    )
        .into_response()
}

async fn serve_frontend_file(repo_root: &FsPath, request_path: Option<String>) -> Response {
    let path = match resolve_frontend_file_path(repo_root, request_path.as_deref()) {
        Some(path) => path,
        None => return text_response(StatusCode::NOT_FOUND, "frontend asset not found"),
    };
    if path.is_dir() {
        return text_response(StatusCode::NOT_FOUND, "frontend asset not found");
    }

    let body = match tokio::fs::read(&path).await {
        Ok(body) => body,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return text_response(StatusCode::NOT_FOUND, "frontend asset not found");
        }
        Err(_) => {
            return text_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "frontend asset unavailable",
            );
        }
    };

    (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(content_type_for_path(&path)),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        ],
        body,
    )
        .into_response()
}

fn resolve_frontend_file_path(repo_root: &FsPath, request_path: Option<&str>) -> Option<PathBuf> {
    let build_dir = repo_root.join(FRONTEND_BUILD_DIR);
    let relative_path = match request_path {
        None | Some("") => PathBuf::from("index.html"),
        Some(path) => sanitize_frontend_relative_path(path)?,
    };
    Some(build_dir.join(relative_path))
}

fn sanitize_frontend_relative_path(path: &str) -> Option<PathBuf> {
    let path = path.trim_matches('/');
    if path.is_empty() {
        return Some(PathBuf::from("index.html"));
    }
    if path.contains('\\') {
        return None;
    }

    let candidate = PathBuf::from(path);
    if candidate.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }

    Some(candidate)
}

fn content_type_for_path(path: &FsPath) -> &'static str {
    match path.extension().and_then(OsStr::to_str) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("map") => "application/json; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn text_response(status: StatusCode, body: &'static str) -> Response {
    (
        status,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        ],
        body,
    )
        .into_response()
}
