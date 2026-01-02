use crate::state::AppState;
use recordroute_common::Result;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tracing::{info, warn};

pub async fn start_websocket_server(state: Arc<AppState>) -> Result<()> {
    let addr = format!("0.0.0.0:{}", state.config.websocket_port);
    let listener = TcpListener::bind(&addr).await?;
    info!("WebSocket server listening on ws://{}", addr);

    while let Ok((stream, peer)) = listener.accept().await {
        info!("WebSocket connection from: {}", peer);
        let _state = state.clone();

        tokio::spawn(async move {
            match accept_async(stream).await {
                Ok(_ws_stream) => {
                    info!("WebSocket handshake successful: {}", peer);
                    // TODO: Handle WebSocket messages
                }
                Err(e) => {
                    warn!("WebSocket handshake failed: {}", e);
                }
            }
        });
    }

    Ok(())
}
