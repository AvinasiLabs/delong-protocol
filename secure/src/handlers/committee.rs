//! Handlers for committee-related operations.

use crate::{
    app_state::AppState,
    auth::check_admin,
    models::{
        blockchain::ENTITY_TYPE_COMMITTEE,
        committee::{CommitteeMember, UpsertCommitteeMemberRequest},
        BlockchainTransaction,
    },
    services::key_ctx::{KeyContext, KeyKind, KEY_CTX_TEE_CONTRACT_OWNER},
};
use axum::{
    extract::{Query, State},
    Json,
};
use common::{ApiResult, ApiResponse, PaginatedResponse, PaginationParams};
use std::sync::Arc;
use tracing::{error, info};
use tracing::instrument;

/// List committee members with pagination and filtering
pub async fn list_committee_members(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> ApiResult<Json<ApiResponse<PaginatedResponse<CommitteeMemberInfo>>>> {
    info!(
        page = params.page,
        limit = params.limit,
        "Listing committee members"
    );

    let (members, total) =
        CommitteeMember::get_confirmed_paginated(&state.db, params.page as i64, params.limit as i64)
            .await?;

    let member_infos = members.into_iter().map(CommitteeMemberInfo::from).collect();
    let response = PaginatedResponse::new(member_infos, params.page, params.limit, total as u64);

    Ok(Json(ApiResponse::success(response)))
}

#[instrument(skip(state, req), fields(member_wallet = %req.member_wallet, is_approved = %req.is_approved))]
pub async fn upsert_committee_member(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpsertCommitteeMemberRequest>,
) -> ApiResult<Json<ApiResponse<String>>> {
    info!("Upserting committee member, aligning with Go logic");

    // Perform authorization check
    check_admin().await?;

    // The entire operation is a single logical unit, so we use a transaction.
    let mut tx = state.db.begin().await?;

    let member = CommitteeMember::upsert(&mut tx, &req.member_wallet, req.is_approved).await?;

    // Call the smart contract
    let key_context = KeyContext::new(
        KeyKind::EthAccount,
        KEY_CTX_TEE_CONTRACT_OWNER,
        "Upserting a committee member",
    );
    let tx_hash = state
        .ctr_caller_service
        .upsert_committee_member(&req.member_wallet, req.is_approved, &key_context)
        .await
        .map_err(|e| {
            tracing::error!("Contract caller error: {}", e);
            common::ApiError::InternalError
        })?;

    let args = serde_json::json!({
        "member_wallet": req.member_wallet,
        "is_approved": req.is_approved
    });

    BlockchainTransaction::create(&mut tx, &tx_hash, member.id, ENTITY_TYPE_COMMITTEE, &args)
        .await?;

    tx.commit().await?;

    info!(tx_hash = %tx_hash, "Successfully upserted committee member");
    Ok(Json(ApiResponse::success(tx_hash)))
} 