//! Blockchain transaction models for DeLong Protocol
//!
//! This module contains models for tracking blockchain transactions
//! and their confirmation status across different entity types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Blockchain transaction status constants
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TransactionStatus {
    /// Transaction submitted but not confirmed
    Pending,
    /// Transaction confirmed on chain
    Confirmed,
    /// Transaction failed
    Failed,
}

impl std::fmt::Display for TransactionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransactionStatus::Pending => write!(f, "PENDING"),
            TransactionStatus::Confirmed => write!(f, "CONFIRMED"),
            TransactionStatus::Failed => write!(f, "FAILED"),
        }
    }
}

/// Entity types that can have associated blockchain transactions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum EntityType {
    /// Algorithm execution
    Execution,
    /// Committee member vote
    Vote,
    /// Committee member
    Committee,
    /// Test report
    TestReport,
    /// Static dataset
    StaticDataset,
    /// Data usage record
    DataUsage,
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntityType::Execution => write!(f, "EXECUTION"),
            EntityType::Vote => write!(f, "VOTE"),
            EntityType::Committee => write!(f, "COMMITTEE"),
            EntityType::TestReport => write!(f, "TEST_REPORT"),
            EntityType::StaticDataset => write!(f, "STATIC_DATASET"),
            EntityType::DataUsage => write!(f, "DATAUSAGE"),
        }
    }
}

/// Blockchain transaction record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BlockchainTransaction {
    /// Primary key
    pub id: i64,
    /// Ethereum transaction hash (66 characters including 0x prefix)
    pub tx_hash: String,
    /// ID of the associated entity
    pub entity_id: i64,
    /// Type of the associated entity
    pub entity_type: Option<String>,
    /// Transaction status
    pub status: Option<String>,
    /// Block number where transaction was confirmed
    pub block_number: Option<i64>,
    /// Timestamp of the block
    pub block_timestamp: Option<DateTime<Utc>>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new blockchain transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTransactionRequest {
    /// Transaction hash
    pub tx_hash: String,
    /// Entity ID
    pub entity_id: i64,
    /// Entity type
    pub entity_type: EntityType,
    /// Optional initial status (defaults to Pending)
    pub status: Option<TransactionStatus>,
    /// Optional block number
    pub block_number: Option<i64>,
    /// Optional block timestamp
    pub block_timestamp: Option<DateTime<Utc>>,
}

/// Request to update transaction status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTransactionStatusRequest {
    /// New transaction status
    pub status: TransactionStatus,
    /// Block number (when confirmed)
    pub block_number: Option<i64>,
    /// Block timestamp (when confirmed)
    pub block_timestamp: Option<DateTime<Utc>>,
}

/// Response containing transaction information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResponse {
    /// Transaction data
    pub transaction: BlockchainTransaction,
}

/// Query parameters for filtering transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionQuery {
    /// Filter by entity type
    pub entity_type: Option<EntityType>,
    /// Filter by status
    pub status: Option<TransactionStatus>,
    /// Filter by entity ID
    pub entity_id: Option<i64>,
    /// Page number (1-indexed)
    pub page: Option<u32>,
    /// Page size (default: 20, max: 100)
    pub limit: Option<u32>,
}

impl TransactionQuery {
    /// Create a new transaction query with default values
    pub fn new() -> Self {
        Self {
            entity_type: None,
            status: None,
            entity_id: None,
            page: Some(1),
            limit: Some(20),
        }
    }

    /// Filter by entity type
    pub fn with_entity_type(mut self, entity_type: EntityType) -> Self {
        self.entity_type = Some(entity_type);
        self
    }

    /// Filter by status
    pub fn with_status(mut self, status: TransactionStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Filter by entity ID
    pub fn with_entity_id(mut self, entity_id: i64) -> Self {
        self.entity_id = Some(entity_id);
        self
    }
}

impl Default for TransactionQuery {
    fn default() -> Self {
        Self::new()
    }
}

/// Constants for building JOIN queries with confirmed transactions
pub const JOIN_CONFIRMED_TX: &str = r#"
JOIN blockchain_transactions bt
ON bt.entity_id = {table}.id
   AND bt.status = 'CONFIRMED'
   AND bt.entity_type = ?
"#;

/// Helper function to validate transaction hash format
pub fn is_valid_tx_hash(hash: &str) -> bool {
    hash.len() == 66 && hash.starts_with("0x") && hash[2..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Request to submit a blockchain transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitTransactionRequest {
    pub transaction_type: String,
    pub data: serde_json::Value,
    pub from_address: String,
}

/// Response with blockchain synchronization status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatusResponse {
    pub is_connected: bool,
    pub last_sync_block: u64,
    pub current_block: u64,
    pub blocks_behind: u64,
    pub transactions_pending: u32,
    pub events_pending: u32,
}

/// Response with blockchain contract statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractStatsResponse {
    pub total_transactions: u32,
    pub pending_transactions: u32,
    pub confirmed_transactions: u32,
    pub failed_transactions: u32,
}

/// Query parameters for blockchain events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventsQuery {
    pub event_type: Option<String>,
    pub from_block: Option<u64>,
    pub to_block: Option<u64>,
}

/// Response for a single blockchain event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainEventResponse {
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub data: serde_json::Value,
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_status_display() {
        assert_eq!(TransactionStatus::Pending.to_string(), "PENDING");
        assert_eq!(TransactionStatus::Confirmed.to_string(), "CONFIRMED");
        assert_eq!(TransactionStatus::Failed.to_string(), "FAILED");
    }

    #[test]
    fn test_entity_type_display() {
        assert_eq!(EntityType::Execution.to_string(), "EXECUTION");
        assert_eq!(EntityType::Vote.to_string(), "VOTE");
        assert_eq!(EntityType::Committee.to_string(), "COMMITTEE");
    }

    #[test]
    fn test_tx_hash_validation() {
        assert!(is_valid_tx_hash("0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"));
        assert!(!is_valid_tx_hash("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"));
        assert!(!is_valid_tx_hash("0x123"));
        assert!(!is_valid_tx_hash("0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdeg"));
    }

    #[test]
    fn test_transaction_query_builder() {
        let query = TransactionQuery::new()
            .with_entity_type(EntityType::Execution)
            .with_status(TransactionStatus::Confirmed);

        assert_eq!(query.entity_type, Some(EntityType::Execution));
        assert_eq!(query.status, Some(TransactionStatus::Confirmed));
        assert_eq!(query.page, Some(1));
        assert_eq!(query.limit, Some(20));
    }
} 