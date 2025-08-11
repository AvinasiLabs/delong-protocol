//! WebSocket connection handler for task-specific notifications

use crate::infra::Notifier;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// Query parameters for WebSocket connection
#[derive(Debug, Deserialize)]
pub struct WsQuery {
    /// Task ID (transaction hash) for this connection
    pub task_id: String,
}

/// WebSocket handler state
#[derive(Clone)]
pub struct WsState {
    pub notifier: Arc<Notifier>,
}

/// Handle WebSocket upgrade request
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsQuery>,
    State(state): State<WsState>,
) -> impl IntoResponse {
    info!("WebSocket upgrade request for task_id: {}", params.task_id);

    ws.on_upgrade(move |socket| handle_socket(socket, params.task_id, state.notifier))
}

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket, task_id: String, notifier: Arc<Notifier>) {
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Register this connection
    notifier.register(task_id.clone(), tx).await;

    // Split the WebSocket
    use futures_util::StreamExt;
    let (ws_tx, mut ws_rx) = socket.split();

    // Task to forward messages from channel to WebSocket
    let forward_task = tokio::spawn(async move {
        use futures_util::SinkExt;

        let mut ws_tx = ws_tx;
        while let Some(msg) = rx.recv().await {
            if let Err(e) = ws_tx.send(msg).await {
                error!("Failed to send WebSocket message: {}", e);
                break;
            }
        }
    });

    // Task to handle incoming messages (mainly for ping/pong)
    let task_id_for_receive = task_id.clone();
    let receive_task = tokio::spawn(async move {
        while let Some(msg) = ws_rx.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    debug!("Received text message: {}", text);
                    // We don't expect any meaningful text messages from client
                }
                Ok(Message::Binary(_)) => {
                    debug!("Received binary message");
                    // Binary messages not supported
                }
                Ok(Message::Ping(_)) => {
                    // Axum handles ping/pong automatically
                }
                Ok(Message::Pong(_)) => {
                    // Pong received
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket closing for task_id: {}", task_id_for_receive);
                    break;
                }
                Err(e) => {
                    error!("WebSocket error for task_id {}: {}", task_id_for_receive, e);
                    break;
                }
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = forward_task => {
            debug!("Forward task completed for task_id: {}", task_id);
        }
        _ = receive_task => {
            debug!("Receive task completed for task_id: {}", task_id);
        }
    }

    // Clean up
    notifier.unregister(&task_id).await;
    info!("WebSocket connection closed for task_id: {}", task_id);
}

/// Create WebSocket routes
pub fn ws_routes() -> axum::Router<WsState> {
    use axum::routing::get;

    axum::Router::new().route("/ws", get(websocket_handler))
}
