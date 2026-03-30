use axum::http::{HeaderValue, header};
use axum::response::{Html, IntoResponse, Response};

const INDEX_HTML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/index.html"));
const APP_JS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/app.js"));
const APP_CSS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/app.css"));

pub(crate) async fn get_index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

pub(crate) async fn get_app_js() -> Response {
    asset_response("text/javascript; charset=utf-8", APP_JS)
}

pub(crate) async fn get_app_css() -> Response {
    asset_response("text/css; charset=utf-8", APP_CSS)
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
