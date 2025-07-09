use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use common::ApiResponse;
use crate::AppState;
use crate::services::blockchain_sync::BlockchainSyncService;

/// Query parameters for listing transactions
#[derive(Debug, Deserialize)]
pub struct ListTransactionsQuery {
    pub entity_type: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
}

/// Request to submit a blockchain transaction
#[derive(Debug, Deserialize, Serialize)]
pub struct SubmitTransactionRequest {
    pub transaction_type: String,
    pub data: serde_json::Value,
    pub from_address: String,
}

/// Response for a blockchain transaction
#[derive(Debug, Serialize)]
pub struct TransactionResponse {
    pub tx_hash: String,
    pub entity_id: i32,
    pub entity_type: String,
    pub status: String,
    pub block_number: Option<i64>,
    pub block_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Contract status response
#[derive(Debug, Serialize)]
pub struct ContractStatusResponse {
    pub is_connected: bool,
    pub last_sync_block: u64,
    pub current_block: u64,
    pub blocks_behind: u64,
    pub transactions_pending: u32,
    pub events_pending: u32,
}

/// Submit a transaction to the blockchain
pub async fn submit_transaction(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<SubmitTransactionRequest>,
) -> Json<ApiResponse<String>> {
    info!(
        transaction_type = %request.transaction_type,
        from_address = %request.from_address,
        "Submitting blockchain transaction"
    );

    // Get blockchain sync service
    let blockchain_sync = match BlockchainSyncService::new().await {
        Ok(sync) => sync,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create blockchain sync service");
            return Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to initialize blockchain service".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    };
    
    // Submit transaction
    match blockchain_sync.submit_transaction(
        &request.transaction_type,
        request.data,
        &request.from_address
    ).await {
        Ok(tx_hash) => Json(ApiResponse::success(tx_hash)),
        Err(e) => {
            tracing::error!(error = %e, "Failed to submit transaction");
            Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to submit transaction".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
}

/// Get transaction status by hash
pub async fn get_transaction_status(
    State(_state): State<Arc<AppState>>,
    Path(tx_hash): Path<String>,
) -> Json<ApiResponse<TransactionResponse>> {
    info!(tx_hash = %tx_hash, "Getting transaction status");

    let blockchain_sync = match BlockchainSyncService::new().await {
        Ok(sync) => sync,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create blockchain sync service");
            return Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to initialize blockchain service".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    };
    
    match blockchain_sync.get_transaction_status(&tx_hash).await {
        Ok(transaction) => {
            let response = TransactionResponse {
                tx_hash: transaction.tx_hash,
                entity_id: transaction.entity_id as i32,
                entity_type: transaction.entity_type.unwrap_or_default(),
                status: transaction.status.unwrap_or_default(),
                block_number: transaction.block_number,
                block_timestamp: transaction.block_timestamp,
                created_at: transaction.created_at,
                updated_at: transaction.updated_at,
            };
            Json(ApiResponse::success(response))
        }
        Err(e) => {
            tracing::error!(error = %e, tx_hash = %tx_hash, "Failed to get transaction status");
            Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to get transaction status".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
}

/// List blockchain transactions
pub async fn list_transactions(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<ListTransactionsQuery>,
) -> Json<ApiResponse<Vec<TransactionResponse>>> {
    info!("Listing blockchain transactions");

    let blockchain_sync = match BlockchainSyncService::new().await {
        Ok(sync) => sync,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create blockchain sync service");
            return Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to initialize blockchain service".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    };
    
    match blockchain_sync.list_transactions(query.limit).await {
        Ok(transactions) => {
            let response_transactions: Vec<TransactionResponse> = transactions.into_iter()
                .filter(|tx| {
                    // Filter by entity_type if specified
                    if let Some(ref entity_type) = query.entity_type {
                        if tx.entity_type.as_ref() != Some(entity_type) {
                            return false;
                        }
                    }
                    
                    // Filter by status if specified
                    if let Some(ref status) = query.status {
                        if tx.status.as_ref() != Some(status) {
                            return false;
                        }
                    }
                    
                    true
                })
                .map(|transaction| TransactionResponse {
                    tx_hash: transaction.tx_hash,
                    entity_id: transaction.entity_id as i32,
                    entity_type: transaction.entity_type.unwrap_or_default(),
                    status: transaction.status.unwrap_or_default(),
                    block_number: transaction.block_number,
                    block_timestamp: transaction.block_timestamp,
                    created_at: transaction.created_at,
                    updated_at: transaction.updated_at,
                })
                .collect();

            Json(ApiResponse::success(response_transactions))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list transactions");
            Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to list transactions".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
}

/// Get blockchain synchronization status
pub async fn get_sync_status(
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<ContractStatusResponse>> {
    info!("Getting blockchain sync status");

    let blockchain_sync = match BlockchainSyncService::new().await {
        Ok(sync) => sync,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create blockchain sync service");
            return Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to initialize blockchain service".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    };
    
    match blockchain_sync.get_sync_status().await {
        Ok(sync_status) => {
            let response = ContractStatusResponse {
                is_connected: sync_status.is_connected,
                last_sync_block: sync_status.last_sync_block,
                current_block: sync_status.current_block,
                blocks_behind: sync_status.blocks_behind,
                transactions_pending: sync_status.transactions_pending,
                events_pending: sync_status.events_pending,
            };
            Json(ApiResponse::success(response))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to get sync status");
            Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to get sync status".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
}

/// Get contract interaction statistics
pub async fn get_contract_stats(
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<ContractStatsResponse>> {
    info!("Getting contract statistics");

    let blockchain_sync = match BlockchainSyncService::new().await {
        Ok(sync) => sync,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create blockchain sync service");
            return Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to initialize blockchain service".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    };
    
    match blockchain_sync.list_transactions(Some(1000)).await {
        Ok(transactions) => {
            let total_transactions = transactions.len() as u32;
            let pending_transactions = transactions.iter()
                .filter(|tx| tx.status.as_deref() == Some("PENDING"))
                .count() as u32;
            let confirmed_transactions = transactions.iter()
                .filter(|tx| tx.status.as_deref() == Some("CONFIRMED"))
                .count() as u32;
            let failed_transactions = transactions.iter()
                .filter(|tx| tx.status.as_deref() == Some("FAILED"))
                .count() as u32;

            let stats = ContractStatsResponse {
                total_transactions,
                pending_transactions,
                confirmed_transactions,
                failed_transactions,
            };
            Json(ApiResponse::success(stats))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to get contract statistics");
            Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to get contract statistics".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
}

/// Contract statistics response
#[derive(Debug, Serialize)]
pub struct ContractStatsResponse {
    pub total_transactions: u32,
    pub pending_transactions: u32,
    pub confirmed_transactions: u32,
    pub failed_transactions: u32,
}

/// Get blockchain events
pub async fn get_blockchain_events(
    State(_state): State<Arc<AppState>>,
    Query(_query): Query<EventsQuery>,
) -> Json<ApiResponse<Vec<BlockchainEventResponse>>> {
    info!("Getting blockchain events");

    let blockchain_sync = match BlockchainSyncService::new().await {
        Ok(sync) => sync,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create blockchain sync service");
            return Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to initialize blockchain service".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    };
    
    // Use empty filter for now - in production this would filter by event types
    match blockchain_sync.listen_for_events(vec![]).await {
        Ok(events) => {
            let response_events: Vec<BlockchainEventResponse> = events.into_iter()
                .map(|event| BlockchainEventResponse {
                    event_type: format!("{:?}", event),
                    timestamp: chrono::Utc::now(), // Placeholder
                    data: serde_json::json!({}), // Placeholder
                })
                .collect();

            Json(ApiResponse::success(response_events))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to get blockchain events");
            Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to get blockchain events".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
}

/// Query parameters for blockchain events
#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    pub event_type: Option<String>,
    pub from_block: Option<u64>,
    pub to_block: Option<u64>,
}

/// Blockchain event response
#[derive(Debug, Serialize)]
pub struct BlockchainEventResponse {
    pub event_type: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub data: serde_json::Value,
} 