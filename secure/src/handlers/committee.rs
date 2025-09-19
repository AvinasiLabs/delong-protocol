//! Committee management handlers
//!
//! This module provides HTTP handlers for managing committee members,
//! including listing, setting member status, and checking membership.

use alloy::primitives::Address;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{info, instrument};

use crate::{
    models::{
        blockchain_transaction::{CreateTransaction, EntityType},
        committee::Committee,
    },
    routes::AppState,
};
use avinapi::prelude::{
    data, paginated, AppError, JsonResult, PaginatedResult, PaginationQuery, ValidatedJson,
    ValidatedQuery,
};
use validator::Validate;

/// Request for setting committee member status
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SetCommitteeMemberRequest {
    /// Wallet address of the member
    #[validate(regex(
        path = "crate::ETHEREUM_ADDRESS_REGEX",
        message = "Invalid Ethereum address format"
    ))]
    pub wallet: String,
    /// Approval status
    pub is_approved: bool,
}

/// Response for setting committee member
#[derive(Debug, Serialize)]
pub struct SetCommitteeMemberResponse {
    /// Transaction hash
    pub tx_hash: String,
}

/// Query parameters for checking membership
#[derive(Debug, Deserialize, Validate)]
pub struct IsMemberQuery {
    /// Wallet address to check
    #[validate(regex(
        path = "crate::ETHEREUM_ADDRESS_REGEX",
        message = "Invalid Ethereum address format"
    ))]
    pub wallet: String,
}

/// Set committee member status (requires admin)
#[instrument(skip(state))]
pub async fn set_committee_member(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<SetCommitteeMemberRequest>,
) -> JsonResult<SetCommitteeMemberResponse> {
    // TODO: Check admin status from authentication context
    // For now, we'll skip this check in development

    // Validate wallet address
    let member_address = Address::from_str(&req.wallet)
        .map_err(|_| AppError::Validation("Invalid wallet address".into()))?;

    // Start database transaction
    let mut tx = state.db.pool().begin().await?;

    // Upsert committee member in database within transaction
    // First check if member exists
    let existing = sqlx::query_as!(
        Committee,
        "SELECT * FROM committee WHERE wallet = $1",
        req.wallet.to_lowercase()
    )
    .fetch_optional(&mut *tx)
    .await?;

    let member = if let Some(existing) = existing {
        // Update existing member
        sqlx::query!(
            "UPDATE committee SET is_approved = $1, updated_at = NOW() WHERE id = $2",
            req.is_approved,
            existing.id
        )
        .execute(&mut *tx)
        .await?;

        Committee {
            id: existing.id,
            wallet: existing.wallet,
            is_approved: req.is_approved,
            created_at: existing.created_at,
            updated_at: chrono::Utc::now(),
        }
    } else {
        // Create new member
        sqlx::query_as!(
            Committee,
            r#"
            INSERT INTO committee (wallet, is_approved)
            VALUES ($1, $2)
            RETURNING *
            "#,
            req.wallet.to_lowercase(),
            req.is_approved
        )
        .fetch_one(&mut *tx)
        .await?
    };

    // Submit to blockchain
    let tx_receipt = if req.is_approved {
        state
            .contract_caller
            .add_committee_member(member_address)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to submit to blockchain: {}", e)))?
    } else {
        state
            .contract_caller
            .remove_committee_member(member_address)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to submit to blockchain: {}", e)))?
    };

    let tx_hash = tx_receipt;

    info!(
        "Committee member {} with tx hash: {}",
        if req.is_approved { "added" } else { "removed" },
        tx_hash
    );

    // Create blockchain transaction record
    let create_tx = CreateTransaction {
        tx_hash: tx_hash.clone(),
        entity_id: member.id as i64,
        entity_type: EntityType::Committee,
    };

    CreateTransaction::create(&mut tx, create_tx).await?;

    // Commit transaction
    tx.commit().await?;

    data!(SetCommitteeMemberResponse { tx_hash })
}

/// List confirmed committee members
#[instrument(skip(state))]
pub async fn list_committee_members(
    State(state): State<AppState>,
    ValidatedQuery(params): ValidatedQuery<PaginationQuery>,
) -> PaginatedResult<Committee> {
    let result = Committee::get_confirmed_members(state.db.pool(), params).await?;

    paginated!(result.items, result.total, result.n_page, result.per_page)
}

/// Get committee member by ID
#[instrument(skip(state))]
pub async fn get_committee_member(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> JsonResult<Committee> {
    let member = Committee::get_confirmed_by_id(state.db.pool(), id)
        .await?
        .ok_or_else(|| AppError::NotFound("Committee member not found".into()))?;

    data!(member)
}

/// Check if wallet is a committee member
#[instrument(skip(state))]
pub async fn is_committee_member(
    State(state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<IsMemberQuery>,
) -> JsonResult<bool> {
    let member = Committee::get_by_wallet(state.db.pool(), &query.wallet).await?;

    let is_member = member.map(|m| m.is_approved).unwrap_or(false);

    data!(is_member)
}
