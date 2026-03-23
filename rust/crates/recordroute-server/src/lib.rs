use axum::{routing::get, Json, Router};
use recordroute_core::config::AppConfig;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: AppConfig,
}

impl AppState {
    pub fn from_env() -> Self {
        Self {
            config: AppConfig::from_env(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct HealthResponse<'a> {
    pub status: &'a str,
    pub db_root: String,
    pub model_root: String,
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .with_state(state)
}

async fn health(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<HealthResponse<'static>> {
    Json(HealthResponse {
        status: "ok",
        db_root: state.config.db_root.display().to_string(),
        model_root: state.config.model_root.display().to_string(),
    })
}

pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .try_init();
}
