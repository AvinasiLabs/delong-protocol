//! Contract metadata handlers
//!
//! This module provides HTTP handlers for managing contract metadata.

use axum::extract::State;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{models::contract::ContractMeta, routes::AppState};
use avinapi::prelude::JsonResult;

/// Response for contract metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct ContractMetaResponse {
    pub id: i64,
    pub name: String,
    pub address: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// List all contract metadata
#[instrument(skip(state))]
pub async fn list_contracts(
    State(state): State<AppState>,
) -> JsonResult<Vec<ContractMetaResponse>> {
    // Get all contracts from database
    let contracts = ContractMeta::get_all(state.db.pool()).await?;

    // Convert to response format
    let response: Vec<ContractMetaResponse> = contracts
        .into_iter()
        .map(|contract| ContractMetaResponse {
            id: contract.id,
            name: contract.name,
            address: contract.address,
            created_at: contract.created_at,
        })
        .collect();

    avinapi::data!(response)
}
