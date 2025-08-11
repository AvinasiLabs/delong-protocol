//! Simple notification system for WebSocket connections
//!
//! This module provides a minimal notification service that maps task IDs (transaction hashes)
//! to WebSocket connections, allowing targeted message delivery.

use axum::extract::ws::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// Simple notifier that maps task IDs to WebSocket connections
#[derive(Clone)]
pub struct Notifier {
    /// Map of task_id (tx_hash) to connection sender
    connections: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Message>>>>,
}

impl Notifier {
    /// Create a new notifier
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a connection with a task ID
    pub async fn register(&self, task_id: String, sender: mpsc::UnboundedSender<Message>) {
        let mut conns = self.connections.write().await;
        conns.insert(task_id.clone(), sender);
        info!("Registered connection for task_id: {}", task_id);
    }

    /// Unregister a connection
    pub async fn unregister(&self, task_id: &str) {
        let mut conns = self.connections.write().await;
        if conns.remove(task_id).is_some() {
            info!("Unregistered connection for task_id: {}", task_id);
        }
    }

    /// Send a message to a specific task ID
    pub async fn send_to_task(&self, task_id: &str, message: String) -> bool {
        let conns = self.connections.read().await;

        if let Some(sender) = conns.get(task_id) {
            if let Err(e) = sender.send(Message::Text(message.into())) {
                warn!("Failed to send message to task_id {}: {}", task_id, e);
                false
            } else {
                debug!("Message sent to task_id: {}", task_id);
                true
            }
        } else {
            debug!("No connection found for task_id: {}", task_id);
            false
        }
    }

    /// Push transaction result to a specific task
    pub async fn push_tx_result(
        &self,
        tx_hash: String,
        transaction: &impl serde::Serialize,
    ) -> Result<(), String> {
        // Create the message envelope
        let envelope = serde_json::json!({
            "type": "tx_result",
            "tx_hash": tx_hash,
            "payload": transaction,
            "timestamp": chrono::Utc::now().timestamp(),
        });

        let message = serde_json::to_string(&envelope)
            .map_err(|e| format!("Failed to serialize message: {}", e))?;

        // Send to the connection identified by tx_hash
        if !self.send_to_task(&tx_hash, message).await {
            return Err(format!("No connection found for tx_hash: {}", tx_hash));
        }

        Ok(())
    }

    /// Get the number of active connections
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Clean up dead connections
    pub async fn cleanup_dead_connections(&self) -> usize {
        let mut conns = self.connections.write().await;
        let initial_count = conns.len();

        // Remove closed connections
        conns.retain(|task_id, sender| {
            if sender.is_closed() {
                info!("Removing dead connection for task_id: {}", task_id);
                false
            } else {
                true
            }
        });

        initial_count - conns.len()
    }
}

impl Default for Notifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_notifier_basic() {
        let notifier = Notifier::new();

        // Create a channel
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Register connection
        let task_id = "0x123abc".to_string();
        notifier.register(task_id.clone(), tx).await;

        assert_eq!(notifier.connection_count().await, 1);

        // Send message
        let sent = notifier
            .send_to_task(&task_id, "test message".to_string())
            .await;
        assert!(sent);

        // Verify message received
        let msg = rx.try_recv().unwrap();
        assert!(matches!(msg, Message::Text(_)));

        // Unregister
        notifier.unregister(&task_id).await;
        assert_eq!(notifier.connection_count().await, 0);
    }
}
