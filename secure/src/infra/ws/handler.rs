use axum::{extract::ws::WebSocketUpgrade, response::Response};
use futures_util::StreamExt;
use std::sync::Arc;
use tracing::{error, info};

use super::hub::Hub;

/// Handle WebSocket upgrade requests
pub async fn ws_handler(ws: WebSocketUpgrade, task_id: String, hub: Arc<Hub>) -> Response {
    info!("WebSocket connection request for task_id: {}", task_id);

    ws.on_upgrade(move |socket| handle_socket(socket, task_id, hub))
}

/// Handle the WebSocket connection
async fn handle_socket(socket: axum::extract::ws::WebSocket, task_id: String, hub: Arc<Hub>) {
    let (sender, mut receiver) = socket.split();

    // Register the connection with the hub
    if let Err(e) = hub.register(task_id.clone(), sender).await {
        error!("Failed to register WebSocket connection: {}", e);
        return;
    }

    // Read messages until connection closes
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(_) => {
                // We don't process incoming messages, just keep the connection alive
            }
            Err(e) => {
                error!("WebSocket error for task_id {}: {}", task_id, e);
                break;
            }
        }
    }

    // Clean up when connection closes
    hub.remove(&task_id).await;
    info!("WebSocket connection closed for task_id: {}", task_id);
}
