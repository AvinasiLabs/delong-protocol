//! Vote handlers
//!
//! This module provides HTTP handlers for querying votes and setting voting duration.

use axum::extract::State;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use crate::{models::vote::Vote, routes::AppState};
use alloy::primitives::U256;
use avinapi::prelude::{
    data, paginated, AppError, JsonResult, PaginatedResult, PaginationQuery, ValidatedJson,
    ValidatedQuery,
};
use validator::Validate;

/// Query parameters for filtering votes
#[derive(Debug, Deserialize, Validate)]
pub struct VoteFilterQuery {
    /// The algorithm CID to filter votes
    #[validate(regex(path = "crate::IPFS_CID_REGEX", message = "Invalid IPFS CID format"))]
    pub algo_cid: String,
}

/// Request for setting voting duration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SetVotingDurationRequest {
    /// Duration in seconds
    #[validate(range(
        min = 60,
        max = 604800,
        message = "Duration must be between 60 seconds (1 minute) and 604800 seconds (7 days)"
    ))]
    pub duration: u64,
}

/// Response for setting voting duration
#[derive(Debug, Serialize)]
pub struct SetVotingDurationResponse {
    /// Transaction hash
    pub tx_hash: String,
}

/// List votes by algorithm CID
#[instrument(skip(state))]
#[axum::debug_handler]
pub async fn list_votes(
    State(state): State<AppState>,
    ValidatedQuery(filter): ValidatedQuery<VoteFilterQuery>,
    ValidatedQuery(params): ValidatedQuery<PaginationQuery>,
) -> PaginatedResult<Vote> {
    let (votes, total) = Vote::find_by_algo_cid_paginated(
        state.db.pool(),
        &filter.algo_cid,
        params.page,
        params.per_page,
    )
    .await?;

    paginated!(votes, total, params.page, params.per_page)
}

/// Set voting duration (requires admin)
#[instrument(skip(state))]
#[axum::debug_handler]
pub async fn set_voting_duration(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<SetVotingDurationRequest>,
) -> JsonResult<SetVotingDurationResponse> {
    // TODO: Check admin status from authentication context
    // For now, we'll skip this check in development

    // Submit to blockchain
    let tx_receipt = state
        .contract_caller
        .set_voting_duration(U256::from(req.duration))
        .await
        .map_err(|e| AppError::Internal(format!("Failed to set voting duration: {}", e)))?;

    let tx_hash = tx_receipt;

    info!(
        "Voting duration set to {} seconds with tx hash: {}",
        req.duration, tx_hash
    );

    data!(SetVotingDurationResponse { tx_hash })
}
