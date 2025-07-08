//! WebSocket notification handlers
//!
//! This module contains handlers for WebSocket real-time notifications,
//! including blockchain transaction status updates and other system events.

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::{config::GatewayConfig, routes::AppState};

use core::{NotificationMessage, TransactionStatus, generate_client_id};

#[cfg(test)]
use core::BlockchainTransactionNotification;

/// WebSocket connection manager
pub type ConnectionManager = Arc<Mutex<HashMap<String, broadcast::Sender<NotificationMessage>>>>;

/// Handler for WebSocket connection upgrade
///
/// GET /api/ws
/// Upgrades HTTP connection to WebSocket for real-time notifications
#[utoipa::path(
    get,
    path = "/api/ws",
    tag = "websocket",
    summary = "WebSocket connection",
    description = "Upgrade HTTP connection to WebSocket for real-time notifications and blockchain transaction updates",
    responses(
        (status = 101, description = "WebSocket connection upgraded successfully"),
        (status = 400, description = "Invalid WebSocket upgrade request"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn websocket_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    info!("WebSocket connection request received");

    ws.on_upgrade(move |socket| handle_websocket_connection(socket, state.config))
}

/// Handle individual WebSocket connection
async fn handle_websocket_connection(socket: WebSocket, _config: GatewayConfig) {
    let client_id = generate_client_id();
    info!(
        "New WebSocket connection established: client_id={}",
        client_id
    );

    let (mut sender, mut receiver) = socket.split();

    // Create broadcast channel for this client
    let (tx, mut rx) = broadcast::channel::<NotificationMessage>(100);

    // Send connection acknowledgment
    let ack_message = NotificationMessage::ConnectionAck {
        client_id: client_id.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        server_version: Some(env!("CARGO_PKG_VERSION").to_string()),
    };

    if let Ok(ack_json) = serde_json::to_string(&ack_message) {
        if let Err(e) = sender.send(Message::Text(ack_json.into())).await {
            error!("Failed to send connection acknowledgment: {}", e);
            return;
        }
    }

    // Spawn task to handle outgoing messages
    let client_id_clone = client_id.clone();
    let outgoing_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            match serde_json::to_string(&msg) {
                Ok(json) => {
                    if let Err(e) = sender.send(Message::Text(json.into())).await {
                        error!(
                            "Failed to send message to client {}: {}",
                            client_id_clone, e
                        );
                        break;
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to serialize message for client {}: {}",
                        client_id_clone, e
                    );
                }
            }
        }
    });

    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Err(e) = handle_websocket_message(&client_id, &text, &tx).await {
                    error!("Error handling WebSocket message from {}: {}", client_id, e);
                }
            }
            Ok(Message::Binary(_)) => {
                warn!("Binary messages not supported for client {}", client_id);
            }
            Ok(Message::Ping(_data)) => {
                info!("Received ping from client {}", client_id);
                // Echo back as pong - axum handles this automatically
                if let Err(e) = tx.send(NotificationMessage::Pong) {
                    error!("Failed to send pong to client {}: {}", client_id, e);
                }
            }
            Ok(Message::Pong(_)) => {
                info!("Received pong from client {}", client_id);
            }
            Ok(Message::Close(_)) => {
                info!("Client {} closed connection", client_id);
                break;
            }
            Err(e) => {
                error!("WebSocket error for client {}: {}", client_id, e);
                break;
            }
        }
    }

    // Clean up
    outgoing_task.abort();
    info!("WebSocket connection closed for client {}", client_id);
}

/// Handle incoming WebSocket message
async fn handle_websocket_message(
    client_id: &str,
    text: &str,
    sender: &broadcast::Sender<NotificationMessage>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Received message from client {}: {}", client_id, text);

    // Try to parse as notification message
    match serde_json::from_str::<NotificationMessage>(text) {
        Ok(NotificationMessage::Ping) => {
            info!("Received ping from client {}", client_id);
            sender.send(NotificationMessage::Pong)?;
        }
        Ok(msg) => {
            info!(
                "Received structured message from client {}: {:?}",
                client_id, msg
            );
            // Echo back or process as needed
        }
        Err(_) => {
            // Handle as plain text or command
            if text.trim() == "ping" {
                sender.send(NotificationMessage::Pong)?;
            } else {
                warn!("Unknown message format from client {}: {}", client_id, text);
            }
        }
    }

    Ok(())
}

/// Broadcast notification to all connected clients
pub async fn broadcast_notification(
    connections: &ConnectionManager,
    notification: NotificationMessage,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let connections_guard = connections
        .lock()
        .map_err(|e| format!("Mutex lock failed: {}", e))?;
    let mut sent_count = 0;
    let mut failed_clients = Vec::new();

    for (client_id, sender) in connections_guard.iter() {
        match sender.send(notification.clone()) {
            Ok(_) => {
                sent_count += 1;
            }
            Err(e) => {
                error!("Failed to send notification to client {}: {}", client_id, e);
                failed_clients.push(client_id.clone());
            }
        }
    }

    // Clean up failed connections
    drop(connections_guard);
    if !failed_clients.is_empty() {
        let mut connections_guard = connections
            .lock()
            .map_err(|e| format!("Mutex lock failed: {}", e))?;
        for client_id in failed_clients {
            connections_guard.remove(&client_id);
            info!("Removed failed client connection: {}", client_id);
        }
    }

    info!("Broadcast notification sent to {} clients", sent_count);
    Ok(sent_count)
}

/// Create a sample blockchain transaction notification
pub fn create_blockchain_notification(
    tx_hash: &str,
    entity_id: &str,
    entity_type: &str,
    status: &str,
) -> NotificationMessage {
    let transaction_status = match status {
        "pending" => TransactionStatus::Pending,
        "mining" => TransactionStatus::Mining,
        "confirmed" => TransactionStatus::Confirmed,
        "failed" => TransactionStatus::Failed,
        "reverted" => TransactionStatus::Reverted,
        _ => TransactionStatus::Pending,
    };

    NotificationMessage::blockchain_transaction(
        rand::random::<u64>(),
        tx_hash.to_string(),
        entity_id.to_string(),
        entity_type.to_string(),
        transaction_status,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_message_serialization() {
        let notification =
            NotificationMessage::BlockchainTransaction(BlockchainTransactionNotification {
                id: 123,
                tx_hash: "0xabcdef123456".to_string(),
                entity_id: "entity_001".to_string(),
                entity_type: "algorithm".to_string(),
                status: TransactionStatus::Confirmed,
                confirmations: Some(12),
                block_number: Some(1000000),
                gas_used: Some(21000),
                created_at: "2023-01-01T00:00:00Z".to_string(),
                updated_at: "2023-01-01T00:05:00Z".to_string(),
            });

        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("BlockchainTransaction"));
        assert!(json.contains("0xabcdef123456"));
        assert!(json.contains("confirmed"));
    }

    #[test]
    fn test_notification_message_deserialization() {
        let json = r#"
        {
            "type": "BlockchainTransaction",
            "data": {
                "id": 456,
                "tx_hash": "0x987654321",
                "entity_id": "algo_789",
                "entity_type": "dataset",
                "status": "pending",
                "created_at": "2023-01-01T12:00:00Z",
                "updated_at": "2023-01-01T12:00:00Z"
            }
        }
        "#;

        let notification: NotificationMessage = serde_json::from_str(json).unwrap();
        match notification {
            NotificationMessage::BlockchainTransaction(data) => {
                assert_eq!(data.id, 456);
                assert_eq!(data.tx_hash, "0x987654321");
                assert_eq!(data.status, TransactionStatus::Pending);
            }
            _ => panic!("Wrong notification type"),
        }
    }

    #[test]
    fn test_algorithm_execution_notification() {
        let notification = NotificationMessage::AlgorithmExecution {
            id: 789,
            status: "completed".to_string(),
            result: Some("success".to_string()),
            error_msg: None,
            progress: Some(100),
        };

        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("AlgorithmExecution"));
        assert!(json.contains("completed"));
        assert!(json.contains("success"));
    }

    #[test]
    fn test_voting_update_notification() {
        let notification = NotificationMessage::VotingUpdate {
            algo_cid: "QmTest123".to_string(),
            vote_count: 5,
            approval_count: 3,
            rejection_count: 2,
            status: "active".to_string(),
            ends_at: Some("2023-01-02T00:00:00Z".to_string()),
        };

        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("VotingUpdate"));
        assert!(json.contains("QmTest123"));
        assert!(json.contains("\"vote_count\":5"));
        assert!(json.contains("\"approval_count\":3"));
    }

    #[test]
    fn test_connection_ack_notification() {
        let client_id = "test-client-123";
        let timestamp = "2023-01-01T10:30:00Z";

        let notification = NotificationMessage::ConnectionAck {
            client_id: client_id.to_string(),
            timestamp: timestamp.to_string(),
            server_version: Some("0.2.0".to_string()),
        };

        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("ConnectionAck"));
        assert!(json.contains(client_id));
        assert!(json.contains(timestamp));
    }

    #[test]
    fn test_ping_pong_messages() {
        let ping = NotificationMessage::Ping;
        let pong = NotificationMessage::Pong;

        let ping_json = serde_json::to_string(&ping).unwrap();
        let pong_json = serde_json::to_string(&pong).unwrap();

        assert_eq!(ping_json, "{\"type\":\"Ping\"}");
        assert_eq!(pong_json, "{\"type\":\"Pong\"}");
    }

    #[test]
    fn test_create_blockchain_notification() {
        let notification =
            create_blockchain_notification("0xtest123", "entity_456", "algorithm", "confirmed");

        match notification {
            NotificationMessage::BlockchainTransaction(data) => {
                assert_eq!(data.tx_hash, "0xtest123");
                assert_eq!(data.entity_id, "entity_456");
                assert_eq!(data.entity_type, "algorithm");
                assert_eq!(data.status, TransactionStatus::Confirmed);
            }
            _ => panic!("Wrong notification type"),
        }
    }

    #[tokio::test]
    async fn test_broadcast_notification_empty_connections() {
        let connections: ConnectionManager = Arc::new(Mutex::new(HashMap::new()));
        let notification = NotificationMessage::Ping;

        let result = broadcast_notification(&connections, notification).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
}
