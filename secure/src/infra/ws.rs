//! WebSocket connection management
//!
//! This module provides WebSocket connection management functionality,
//! including connection tracking, message routing, and task-based notifications.

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, mpsc};
use tracing::{debug, error, info};

/// WebSocket hub for managing connections
pub struct WsHub {
    /// Active connections mapped by task ID
    connections: Arc<RwLock<HashMap<String, mpsc::Sender<WsMessage>>>>,
    /// Pending messages for tasks that haven't connected yet
    pending_messages: Arc<Mutex<HashMap<String, Vec<WsMessage>>>>,
}

/// Message types that can be sent over WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    /// Task status update
    TaskStatus {
        task_id: String,
        status: String,
        progress: Option<f32>,
    },
    /// Task completed
    TaskCompleted {
        task_id: String,
        success: bool,
        result: Option<serde_json::Value>,
        error: Option<String>,
    },
    /// General notification
    Notification { level: String, message: String },
    /// Heartbeat/ping
    Ping,
    /// Heartbeat/pong
    Pong,
    /// Error message
    Error { code: String, message: String },
}

impl WsHub {
    /// Create a new WebSocket hub
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            pending_messages: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a new WebSocket connection for a task
    pub async fn register(&self, task_id: String, socket: WebSocket) {
        info!("Registering WebSocket connection for task: {}", task_id);

        let (tx, mut rx) = mpsc::channel::<WsMessage>(32);

        // Check for pending messages
        let pending = {
            let mut pending_map = self.pending_messages.lock().await;
            pending_map.remove(&task_id).unwrap_or_default()
        };

        // Store the connection
        {
            let mut connections = self.connections.write().await;
            connections.insert(task_id.clone(), tx);
        }

        // Split the WebSocket
        let (mut ws_tx, mut ws_rx) = socket.split();

        // Send any pending messages
        for msg in pending {
            if let Ok(json) = serde_json::to_string(&msg) {
                let _ = ws_tx.send(Message::Text(json.into())).await;
            }
        }

        let task_id_clone = task_id.clone();
        let hub = self.clone();

        // Spawn task to handle outgoing messages
        let send_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match serde_json::to_string(&msg) {
                    Ok(json) => {
                        if let Err(e) = ws_tx.send(Message::Text(json.into())).await {
                            error!("Failed to send WebSocket message: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to serialize message: {}", e);
                    }
                }
            }
        });

        // Spawn task to handle incoming messages
        let recv_task = tokio::spawn(async move {
            while let Some(result) = ws_rx.next().await {
                match result {
                    Ok(Message::Text(text)) => {
                        debug!("Received text message: {}", text);
                        // Handle incoming messages if needed
                        if let Ok(msg) = serde_json::from_str::<WsMessage>(&text) {
                            match msg {
                                WsMessage::Ping => {
                                    hub.notify(&task_id_clone, WsMessage::Pong).await;
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("WebSocket closed for task: {}", task_id_clone);
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Wait for either task to complete
        tokio::select! {
            _ = send_task => {},
            _ = recv_task => {},
        }

        // Remove the connection
        self.unregister(&task_id).await;
    }

    /// Unregister a WebSocket connection
    pub async fn unregister(&self, task_id: &str) {
        info!("Unregistering WebSocket connection for task: {}", task_id);
        let mut connections = self.connections.write().await;
        connections.remove(task_id);
    }

    /// Send a notification to a specific task
    pub async fn notify(&self, task_id: &str, message: WsMessage) {
        let connections = self.connections.read().await;

        if let Some(tx) = connections.get(task_id) {
            if let Err(e) = tx.send(message).await {
                error!("Failed to send message to task {}: {}", task_id, e);
            }
        } else {
            // Store message for later delivery
            let mut pending = self.pending_messages.lock().await;
            pending
                .entry(task_id.to_string())
                .or_insert_with(Vec::new)
                .push(message);
        }
    }

    /// Broadcast a message to all connected clients
    pub async fn broadcast(&self, message: WsMessage) {
        let connections = self.connections.read().await;

        for (task_id, tx) in connections.iter() {
            if let Err(e) = tx.send(message.clone()).await {
                error!("Failed to broadcast to task {}: {}", task_id, e);
            }
        }
    }

    /// Get the number of active connections
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Check if a task has an active connection
    pub async fn is_connected(&self, task_id: &str) -> bool {
        self.connections.read().await.contains_key(task_id)
    }
}

impl Clone for WsHub {
    fn clone(&self) -> Self {
        Self {
            connections: self.connections.clone(),
            pending_messages: self.pending_messages.clone(),
        }
    }
}

impl Default for WsHub {
    fn default() -> Self {
        Self::new()
    }
}

/// WebSocket handler for Axum
pub async fn ws_handler(ws: WebSocketUpgrade, task_id: String, hub: Arc<WsHub>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, task_id, hub))
}

/// Handle a WebSocket connection
async fn handle_socket(socket: WebSocket, task_id: String, hub: Arc<WsHub>) {
    hub.register(task_id, socket).await;
}

/// Task notifier for sending task-related notifications
pub struct TaskNotifier {
    hub: Arc<WsHub>,
}

impl TaskNotifier {
    /// Create a new task notifier
    pub fn new(hub: Arc<WsHub>) -> Self {
        Self { hub }
    }

    /// Notify task status update
    pub async fn notify_status(&self, task_id: &str, status: &str, progress: Option<f32>) {
        self.hub
            .notify(
                task_id,
                WsMessage::TaskStatus {
                    task_id: task_id.to_string(),
                    status: status.to_string(),
                    progress,
                },
            )
            .await;
    }

    /// Notify task completion
    pub async fn notify_completed(
        &self,
        task_id: &str,
        success: bool,
        result: Option<serde_json::Value>,
        error: Option<String>,
    ) {
        self.hub
            .notify(
                task_id,
                WsMessage::TaskCompleted {
                    task_id: task_id.to_string(),
                    success,
                    result,
                    error,
                },
            )
            .await;
    }

    /// Send a general notification
    pub async fn notify(&self, task_id: &str, level: &str, message: &str) {
        self.hub
            .notify(
                task_id,
                WsMessage::Notification {
                    level: level.to_string(),
                    message: message.to_string(),
                },
            )
            .await;
    }

    /// Send an error message
    pub async fn notify_error(&self, task_id: &str, code: &str, message: &str) {
        self.hub
            .notify(
                task_id,
                WsMessage::Error {
                    code: code.to_string(),
                    message: message.to_string(),
                },
            )
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hub_creation() {
        let hub = WsHub::new();
        assert_eq!(hub.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_pending_messages() {
        let hub = WsHub::new();
        let task_id = "test-task";

        // Send message before connection
        hub.notify(
            task_id,
            WsMessage::Notification {
                level: "info".to_string(),
                message: "Test message".to_string(),
            },
        )
        .await;

        // Check pending messages
        {
            let pending = hub.pending_messages.lock().await;
            assert!(pending.contains_key(task_id));
            assert_eq!(pending.get(task_id).unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn test_task_notifier() {
        let hub = Arc::new(WsHub::new());
        let notifier = TaskNotifier::new(hub.clone());

        let task_id = "test-task";

        // Test various notifications
        notifier.notify_status(task_id, "running", Some(0.5)).await;
        notifier.notify_completed(task_id, true, None, None).await;
        notifier.notify(task_id, "info", "Test notification").await;
        notifier
            .notify_error(task_id, "TEST_ERROR", "Test error message")
            .await;

        // All messages should be pending
        let pending = hub.pending_messages.lock().await;
        assert_eq!(pending.get(task_id).map(|v| v.len()).unwrap_or(0), 4);
    }
}
