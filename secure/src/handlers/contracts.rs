use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use tracing::info;

use common::{
    ApiResult, 
    ApiResponse,
    models::blockchain::{
        SubmitTransactionRequest,
        TransactionResponse,
        TransactionQuery,
        SyncStatusResponse,
        ContractStatsResponse,
        EventsQuery,
        BlockchainEventResponse,
        BlockchainTransaction,
    },
};
use crate::AppState;

fn to_transaction_response(tx: BlockchainTransaction) -> TransactionResponse {
    TransactionResponse {
        transaction: tx,
    }
}

/// Submit a transaction to the blockchain
pub async fn submit_transaction(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SubmitTransactionRequest>,
) -> ApiResult<Json<ApiResponse<String>>> {
    info!(
        transaction_type = %request.transaction_type,
        from_address = %request.from_address,
        "Submitting blockchain transaction"
    );

    let tx_hash = state.blockchain_sync_service.submit_transaction(
        &request.transaction_type,
        request.data,
        &request.from_address
    ).await?;
    
    Ok(Json(ApiResponse::success(tx_hash)))
}

/// Get transaction status by hash
pub async fn get_transaction_status(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<String>,
) -> ApiResult<Json<ApiResponse<TransactionResponse>>> {
    info!(tx_hash = %tx_hash, "Getting transaction status");
    
    let transaction = state.blockchain_sync_service.get_transaction_status(&tx_hash).await?;
    
    Ok(Json(ApiResponse::success(to_transaction_response(transaction))))
}

/// List blockchain transactions
pub async fn list_transactions(
    State(state): State<Arc<AppState>>,
    Query(_query): Query<TransactionQuery>,
) -> ApiResult<Json<ApiResponse<Vec<TransactionResponse>>>> {
    info!("Listing blockchain transactions");
    
    // The query param is not used in the service method, so I pass a default limit.
    // The service should be improved to handle filtering.
    let transactions = state.blockchain_sync_service.list_transactions(Some(100)).await?;

    let response_transactions = transactions.into_iter()
        .map(to_transaction_response)
        .collect();

    Ok(Json(ApiResponse::success(response_transactions)))
}

/// Get blockchain synchronization status
pub async fn get_sync_status(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ApiResponse<SyncStatusResponse>>> {
    info!("Getting blockchain sync status");

    let sync_status = state.blockchain_sync_service.get_sync_status().await?;
    
    let response = SyncStatusResponse {
        is_connected: sync_status.is_connected,
        last_sync_block: sync_status.last_sync_block,
        current_block: sync_status.current_block,
        blocks_behind: sync_status.blocks_behind,
        transactions_pending: sync_status.transactions_pending,
        events_pending: sync_status.events_pending,
    };
    
    Ok(Json(ApiResponse::success(response)))
}

/// Get contract statistics
pub async fn get_contract_stats(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ApiResponse<ContractStatsResponse>>> {
    info!("Getting contract statistics");
    
    let transactions = state.blockchain_sync_service.list_transactions(None).await?;
    
    let mut pending = 0;
    let mut confirmed = 0;
    let mut failed = 0;

    for tx in &transactions {
        match tx.status.as_deref() {
            Some("PENDING") => pending += 1,
            Some("CONFIRMED") => confirmed += 1,
            Some("FAILED") => failed += 1,
            _ => {}
        }
    }

    let stats = ContractStatsResponse {
        total_transactions: transactions.len() as u32,
        pending_transactions: pending,
        confirmed_transactions: confirmed,
        failed_transactions: failed,
    };

    Ok(Json(ApiResponse::success(stats)))
}

/// Get blockchain events
pub async fn get_blockchain_events(
    State(state): State<Arc<AppState>>,
    Query(_query): Query<EventsQuery>,
) -> ApiResult<Json<ApiResponse<Vec<BlockchainEventResponse>>>> {
    info!("Getting blockchain events");

    // The service's listen_for_events doesn't take filters yet.
    let events = state.blockchain_sync_service.listen_for_events(vec![]).await?;

    let response_events = events.into_iter().map(|event| {
        BlockchainEventResponse {
            event_type: event.event_type,
            timestamp: event.timestamp.unwrap_or_default(),
            data: event.event_data.unwrap_or_default(),
        }
    }).collect();

    Ok(Json(ApiResponse::success(response_events)))
} 