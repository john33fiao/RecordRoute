//! RecordRoute HTTP/WebSocket Server
//!
//! Actix-web based REST API and WebSocket server

mod error;
pub mod history;
pub mod job_manager;
pub mod routes;
pub mod state;
pub mod types;
pub mod websocket;
pub mod workflow;

use actix_cors::Cors;
use actix_files as fs;
use actix_web::{middleware, web, App, HttpServer};
use recordroute_common::{AppConfig, Result};
use state::AppState;
use std::sync::Arc;
use tracing::info;

/// Start the HTTP and WebSocket server
pub async fn start_server(config: AppConfig) -> Result<()> {
    info!("Starting RecordRoute server on {}", config.server_bind_address());

    // Create application state (downloads model if needed)
    let app_state = Arc::new(AppState::new(config.clone()).await?);

    // Start WebSocket server in background
    let ws_state = app_state.clone();
    tokio::spawn(async move {
        if let Err(e) = websocket::start_websocket_server(ws_state).await {
            tracing::error!("WebSocket server error: {}", e);
        }
    });

    // Start HTTP server
    let bind_addr = config.server_bind_address();

    HttpServer::new(move || {
        let cors = Cors::permissive(); // TODO: Configure CORS properly

        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(middleware::Logger::default())
            .wrap(cors)
            // API routes
            .service(routes::upload::upload)
            .service(routes::process::process)
            .service(routes::history::get_history)
            .service(routes::history::delete_history)
            .service(routes::download::download)
            .service(routes::tasks::get_tasks)
            .service(routes::tasks::cancel_task)
            .service(routes::search::search)
            .service(routes::search::search_stats)
            // Static files and index
            .service(fs::Files::new("/", "frontend").index_file("upload.html"))
    })
    .bind(&bind_addr)?
    .run()
    .await?;

    Ok(())
}
