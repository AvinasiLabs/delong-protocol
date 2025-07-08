//! WebSocket notification data models
//!
//! This module contains all data structures related to WebSocket real-time notifications,
//! including blockchain transaction updates, system events, and connection management.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// WebSocket notification message types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(tag = "type", content = "data")]
pub enum NotificationMessage {
    /// Blockchain transaction status update
    BlockchainTransaction(BlockchainTransactionNotification),

    /// Algorithm execution status update
    AlgorithmExecution {
        id: u64,
        status: String,
        result: Option<String>,
        error_msg: Option<String>,
        progress: Option<u8>, // 0-100 percentage
    },

    /// Committee voting update
    VotingUpdate {
        algo_cid: String,
        vote_count: u32,
        approval_count: u32,
        rejection_count: u32,
        status: String,
        ends_at: Option<String>,
    },

    /// Dataset upload/processing update
    DatasetUpdate {
        dataset_id: String,
        status: String,
        progress: Option<u8>, // 0-100 percentage
        message: Option<String>,
    },

    /// System health notification
    SystemHealth {
        service: String,
        status: String,
        message: String,
        timestamp: String,
    },

    /// Connection acknowledgment
    ConnectionAck {
        client_id: String,
        timestamp: String,
        server_version: Option<String>,
    },

    /// Error notification
    Error {
        error_code: String,
        message: String,
        details: Option<String>,
    },

    /// Ping/Pong for keep-alive
    Ping,
    Pong,
}

/// Blockchain transaction notification data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct BlockchainTransactionNotification {
    /// Unique identifier for the notification
    pub id: u64,
    /// Transaction hash on blockchain
    pub tx_hash: String,
    /// Entity ID associated with the transaction
    pub entity_id: String,
    /// Type of entity (e.g., "algorithm", "dataset", "vote")
    pub entity_type: String,
    /// Current transaction status
    pub status: TransactionStatus,
    /// Number of confirmations (if applicable)
    pub confirmations: Option<u32>,
    /// Block number where transaction was included
    pub block_number: Option<u64>,
    /// Gas used for the transaction
    pub gas_used: Option<u64>,
    /// Creation timestamp in RFC3339 format
    pub created_at: String,
    /// Last update timestamp in RFC3339 format
    pub updated_at: String,
}

/// Transaction status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub enum TransactionStatus {
    /// Transaction is pending in mempool
    #[serde(rename = "pending")]
    Pending,
    /// Transaction is being mined
    #[serde(rename = "mining")]
    Mining,
    /// Transaction confirmed on blockchain
    #[serde(rename = "confirmed")]
    Confirmed,
    /// Transaction failed
    #[serde(rename = "failed")]
    Failed,
    /// Transaction was reverted
    #[serde(rename = "reverted")]
    Reverted,
}

/// WebSocket client connection information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct WebSocketClient {
    /// Unique client identifier
    pub id: String,
    /// Connection timestamp
    pub connected_at: String,
    /// Last ping timestamp (if any)
    pub last_ping: Option<String>,
    /// Client user agent (if available)
    pub user_agent: Option<String>,
    /// Client IP address (if available)
    pub ip_address: Option<String>,
    /// Subscription topics
    pub subscriptions: Vec<String>,
}

/// WebSocket connection statistics
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ConnectionStats {
    /// Total number of active connections
    pub active_connections: u32,
    /// Total messages sent
    pub messages_sent: u64,
    /// Total messages received
    pub messages_received: u64,
    /// Number of failed message deliveries
    pub failed_deliveries: u64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: u32,
}

/// Subscription request from client
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct SubscriptionRequest {
    /// Topics to subscribe to
    pub topics: Vec<String>,
    /// Client identifier
    pub client_id: String,
}

/// Subscription response to client
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct SubscriptionResponse {
    /// Whether subscription was successful
    pub success: bool,
    /// Subscribed topics
    pub subscribed_topics: Vec<String>,
    /// Error message if subscription failed
    pub error: Option<String>,
}

impl NotificationMessage {
    /// Create a blockchain transaction notification
    pub fn blockchain_transaction(
        id: u64,
        tx_hash: String,
        entity_id: String,
        entity_type: String,
        status: TransactionStatus,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        NotificationMessage::BlockchainTransaction(BlockchainTransactionNotification {
            id,
            tx_hash,
            entity_id,
            entity_type,
            status,
            confirmations: None,
            block_number: None,
            gas_used: None,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Create an algorithm execution notification
    pub fn algorithm_execution(id: u64, status: String) -> Self {
        NotificationMessage::AlgorithmExecution {
            id,
            status,
            result: None,
            error_msg: None,
            progress: None,
        }
    }

    /// Create a voting update notification
    pub fn voting_update(
        algo_cid: String,
        vote_count: u32,
        approval_count: u32,
        status: String,
    ) -> Self {
        NotificationMessage::VotingUpdate {
            algo_cid,
            vote_count,
            approval_count,
            rejection_count: vote_count.saturating_sub(approval_count),
            status,
            ends_at: None,
        }
    }

    /// Create a system health notification
    pub fn system_health(service: String, status: String, message: String) -> Self {
        NotificationMessage::SystemHealth {
            service,
            status,
            message,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Create a connection acknowledgment
    pub fn connection_ack(client_id: String) -> Self {
        NotificationMessage::ConnectionAck {
            client_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
            server_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }
    }

    /// Create an error notification
    pub fn error(error_code: String, message: String) -> Self {
        NotificationMessage::Error {
            error_code,
            message,
            details: None,
        }
    }

    /// Check if the message is a control message (ping/pong/ack)
    pub fn is_control_message(&self) -> bool {
        matches!(
            self,
            NotificationMessage::Ping
                | NotificationMessage::Pong
                | NotificationMessage::ConnectionAck { .. }
        )
    }

    /// Get the message type as string
    pub fn message_type(&self) -> &'static str {
        match self {
            NotificationMessage::BlockchainTransaction(_) => "blockchain_transaction",
            NotificationMessage::AlgorithmExecution { .. } => "algorithm_execution",
            NotificationMessage::VotingUpdate { .. } => "voting_update",
            NotificationMessage::DatasetUpdate { .. } => "dataset_update",
            NotificationMessage::SystemHealth { .. } => "system_health",
            NotificationMessage::ConnectionAck { .. } => "connection_ack",
            NotificationMessage::Error { .. } => "error",
            NotificationMessage::Ping => "ping",
            NotificationMessage::Pong => "pong",
        }
    }
}

impl TransactionStatus {
    /// Check if the transaction is in a final state
    pub fn is_final(&self) -> bool {
        matches!(
            self,
            TransactionStatus::Confirmed | TransactionStatus::Failed | TransactionStatus::Reverted
        )
    }

    /// Check if the transaction is still pending
    pub fn is_pending(&self) -> bool {
        matches!(self, TransactionStatus::Pending | TransactionStatus::Mining)
    }

    /// Check if the transaction was successful
    pub fn is_successful(&self) -> bool {
        matches!(self, TransactionStatus::Confirmed)
    }
}

impl BlockchainTransactionNotification {
    /// Create a new blockchain transaction notification
    pub fn new(
        id: u64,
        tx_hash: String,
        entity_id: String,
        entity_type: String,
        status: TransactionStatus,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id,
            tx_hash,
            entity_id,
            entity_type,
            status,
            confirmations: None,
            block_number: None,
            gas_used: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Update the transaction status
    pub fn update_status(&mut self, status: TransactionStatus) {
        self.status = status;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Set confirmation details
    pub fn set_confirmation_details(&mut self, confirmations: u32, block_number: u64) {
        self.confirmations = Some(confirmations);
        self.block_number = Some(block_number);
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Validate the transaction hash format
    pub fn validate_tx_hash(&self) -> Result<(), String> {
        if self.tx_hash.is_empty() {
            return Err("Transaction hash cannot be empty".to_string());
        }

        if !self.tx_hash.starts_with("0x") {
            return Err("Transaction hash must start with '0x'".to_string());
        }

        if self.tx_hash.len() != 66 {
            return Err("Transaction hash must be 66 characters long".to_string());
        }

        let hex_part = &self.tx_hash[2..];
        if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("Transaction hash contains invalid hexadecimal characters".to_string());
        }

        Ok(())
    }
}

impl WebSocketClient {
    /// Create a new WebSocket client
    pub fn new(id: String) -> Self {
        Self {
            id,
            connected_at: chrono::Utc::now().to_rfc3339(),
            last_ping: None,
            user_agent: None,
            ip_address: None,
            subscriptions: Vec::new(),
        }
    }

    /// Update the last ping timestamp
    pub fn update_ping(&mut self) {
        self.last_ping = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Add a subscription topic
    pub fn subscribe(&mut self, topic: String) {
        if !self.subscriptions.contains(&topic) {
            self.subscriptions.push(topic);
        }
    }

    /// Remove a subscription topic
    pub fn unsubscribe(&mut self, topic: &str) {
        self.subscriptions.retain(|t| t != topic);
    }

    /// Check if client is subscribed to a topic
    pub fn is_subscribed_to(&self, topic: &str) -> bool {
        self.subscriptions.contains(&topic.to_string())
    }
}

impl SubscriptionRequest {
    /// Create a new subscription request
    pub fn new(client_id: String, topics: Vec<String>) -> Self {
        Self { topics, client_id }
    }

    /// Validate the subscription request
    pub fn validate(&self) -> Result<(), String> {
        if self.client_id.trim().is_empty() {
            return Err("Client ID cannot be empty".to_string());
        }

        if self.topics.is_empty() {
            return Err("At least one topic must be specified".to_string());
        }

        if self.topics.len() > 50 {
            return Err("Cannot subscribe to more than 50 topics".to_string());
        }

        for topic in &self.topics {
            if topic.trim().is_empty() {
                return Err("Topic name cannot be empty".to_string());
            }
            if topic.len() > 100 {
                return Err("Topic name cannot exceed 100 characters".to_string());
            }
        }

        Ok(())
    }
}

/// Generate a unique request ID for WebSocket operations
pub fn generate_client_id() -> String {
    format!("ws_{}", uuid::Uuid::new_v4().simple())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_message_creation() {
        let tx_notification = NotificationMessage::blockchain_transaction(
            1,
            "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12".to_string(),
            "entity_123".to_string(),
            "algorithm".to_string(),
            TransactionStatus::Confirmed,
        );

        match tx_notification {
            NotificationMessage::BlockchainTransaction(ref data) => {
                assert_eq!(data.id, 1);
                assert_eq!(data.entity_id, "entity_123");
                assert_eq!(data.status, TransactionStatus::Confirmed);
            }
            _ => panic!("Wrong notification type"),
        }

        assert_eq!(tx_notification.message_type(), "blockchain_transaction");
        assert!(!tx_notification.is_control_message());
    }

    #[test]
    fn test_algorithm_execution_notification() {
        let algo_notification =
            NotificationMessage::algorithm_execution(42, "completed".to_string());

        match algo_notification {
            NotificationMessage::AlgorithmExecution { id, ref status, .. } => {
                assert_eq!(id, 42);
                assert_eq!(status, "completed");
            }
            _ => panic!("Wrong notification type"),
        }

        assert_eq!(algo_notification.message_type(), "algorithm_execution");
    }

    #[test]
    fn test_control_messages() {
        let ping = NotificationMessage::Ping;
        let pong = NotificationMessage::Pong;
        let ack = NotificationMessage::connection_ack("client_123".to_string());

        assert!(ping.is_control_message());
        assert!(pong.is_control_message());
        assert!(ack.is_control_message());

        assert_eq!(ping.message_type(), "ping");
        assert_eq!(pong.message_type(), "pong");
        assert_eq!(ack.message_type(), "connection_ack");
    }

    #[test]
    fn test_transaction_status_checks() {
        assert!(TransactionStatus::Confirmed.is_final());
        assert!(TransactionStatus::Failed.is_final());
        assert!(!TransactionStatus::Pending.is_final());

        assert!(TransactionStatus::Pending.is_pending());
        assert!(TransactionStatus::Mining.is_pending());
        assert!(!TransactionStatus::Confirmed.is_pending());

        assert!(TransactionStatus::Confirmed.is_successful());
        assert!(!TransactionStatus::Failed.is_successful());
    }

    #[test]
    fn test_blockchain_transaction_notification() {
        let mut notification = BlockchainTransactionNotification::new(
            1,
            "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
            "entity_123".to_string(),
            "algorithm".to_string(),
            TransactionStatus::Pending,
        );

        assert!(notification.validate_tx_hash().is_ok());
        assert_eq!(notification.status, TransactionStatus::Pending);

        notification.update_status(TransactionStatus::Confirmed);
        assert_eq!(notification.status, TransactionStatus::Confirmed);

        notification.set_confirmation_details(12, 1000000);
        assert_eq!(notification.confirmations, Some(12));
        assert_eq!(notification.block_number, Some(1000000));
    }

    #[test]
    fn test_invalid_transaction_hash() {
        let notification = BlockchainTransactionNotification::new(
            1,
            "invalid_hash".to_string(),
            "entity_123".to_string(),
            "algorithm".to_string(),
            TransactionStatus::Pending,
        );

        assert!(notification.validate_tx_hash().is_err());
    }

    #[test]
    fn test_websocket_client() {
        let mut client = WebSocketClient::new("client_123".to_string());
        assert_eq!(client.id, "client_123");
        assert!(client.subscriptions.is_empty());

        client.subscribe("blockchain_updates".to_string());
        client.subscribe("algorithm_events".to_string());
        assert_eq!(client.subscriptions.len(), 2);
        assert!(client.is_subscribed_to("blockchain_updates"));

        client.unsubscribe("blockchain_updates");
        assert_eq!(client.subscriptions.len(), 1);
        assert!(!client.is_subscribed_to("blockchain_updates"));

        client.update_ping();
        assert!(client.last_ping.is_some());
    }

    #[test]
    fn test_subscription_request_validation() {
        // Valid request
        let valid_request = SubscriptionRequest::new(
            "client_123".to_string(),
            vec!["topic1".to_string(), "topic2".to_string()],
        );
        assert!(valid_request.validate().is_ok());

        // Empty client ID
        let empty_client = SubscriptionRequest::new("".to_string(), vec!["topic1".to_string()]);
        assert!(empty_client.validate().is_err());

        // No topics
        let no_topics = SubscriptionRequest::new("client_123".to_string(), vec![]);
        assert!(no_topics.validate().is_err());

        // Too many topics
        let too_many_topics = SubscriptionRequest::new(
            "client_123".to_string(),
            (0..51).map(|i| format!("topic_{}", i)).collect(),
        );
        assert!(too_many_topics.validate().is_err());
    }

    #[test]
    fn test_serialization_deserialization() {
        let notification = NotificationMessage::blockchain_transaction(
            1,
            "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12".to_string(),
            "entity_123".to_string(),
            "algorithm".to_string(),
            TransactionStatus::Confirmed,
        );

        let json = serde_json::to_string(&notification).unwrap();
        let deserialized: NotificationMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(notification, deserialized);

        let client = WebSocketClient::new("client_123".to_string());
        let json = serde_json::to_string(&client).unwrap();
        let deserialized: WebSocketClient = serde_json::from_str(&json).unwrap();
        assert_eq!(client, deserialized);
    }

    #[test]
    fn test_generate_client_id() {
        let id1 = generate_client_id();
        let id2 = generate_client_id();

        assert!(id1.starts_with("ws_"));
        assert!(id2.starts_with("ws_"));
        assert_ne!(id1, id2); // Should be unique
    }
}
