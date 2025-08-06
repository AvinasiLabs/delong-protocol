use axum::extract::ws::{Message, WebSocket};
use futures_util::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream},
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

type WsSender = SplitSink<WebSocket, Message>;
#[allow(dead_code)]
type WsReceiver = SplitStream<WebSocket>;

/// WebSocket Hub for managing connections and notifications
pub struct Hub {
    /// Active connections mapped by task_id
    connections: Arc<Mutex<HashMap<String, WsSender>>>,
    /// Buffered messages for task_ids that haven't connected yet
    buffer: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}

impl Hub {
    /// Create a new Hub instance
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            buffer: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a new WebSocket connection
    pub async fn register(
        &self,
        task_id: String,
        mut sender: WsSender,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut connections = self.connections.lock().await;
        let mut buffer = self.buffer.lock().await;

        // Check if there's a buffered message for this task_id
        if let Some(msg) = buffer.remove(&task_id) {
            // Send the buffered message
            let text = serde_json::to_string(&msg)?;
            sender.send(Message::Text(text.into())).await?;
            sender.close().await?;
            info!("Sent buffered message to task_id: {}", task_id);
        } else {
            // Store the connection for future messages
            connections.insert(task_id.clone(), sender);
            info!("Registered connection for task_id: {}", task_id);
        }

        Ok(())
    }

    /// Notify a task with a message
    pub async fn notify<T: Serialize>(
        &self,
        task_id: String,
        payload: T,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut connections = self.connections.lock().await;
        let mut buffer = self.buffer.lock().await;

        let value = serde_json::to_value(&payload)?;

        if let Some(mut sender) = connections.remove(&task_id) {
            // Connection exists, send the message
            let text = serde_json::to_string(&value)?;
            if let Err(e) = sender.send(Message::Text(text.into())).await {
                error!("Failed to send message to task_id {}: {}", task_id, e);
            }

            // Close the connection after sending
            if let Err(e) = sender.close().await {
                error!("Failed to close connection for task_id {}: {}", task_id, e);
            }

            info!("Notified and closed connection for task_id: {}", task_id);
        } else {
            // No connection yet, buffer the message
            buffer.insert(task_id.clone(), value);
            info!("Buffered message for task_id: {}", task_id);
        }

        Ok(())
    }

    /// Remove a connection (called when client disconnects)
    pub async fn remove(&self, task_id: &str) {
        let mut connections = self.connections.lock().await;
        let mut buffer = self.buffer.lock().await;

        connections.remove(task_id);
        buffer.remove(task_id);

        info!("Removed connection and buffer for task_id: {}", task_id);
    }

    /// Get the number of active connections
    pub async fn connection_count(&self) -> usize {
        self.connections.lock().await.len()
    }

    /// Get the number of buffered messages
    pub async fn buffer_count(&self) -> usize {
        self.buffer.lock().await.len()
    }
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Hub {
    fn clone(&self) -> Self {
        Self {
            connections: Arc::clone(&self.connections),
            buffer: Arc::clone(&self.buffer),
        }
    }
}
