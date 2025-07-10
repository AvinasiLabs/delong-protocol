use crate::{
    models::{
        vote::{CastVoteRequest, Vote, VoteQuery},
        AppState, BlockchainTransaction, CommitteeMember, ENTITY_TYPE_VOTE,
    },
    services::key_ctx::{KeyContext, KeyKind},
};
use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use common::{ApiError, ApiResult, ApiResponse, PaginatedResponse, PaginationParams};
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info, instrument};

/// Cast a new vote, requires committee member approval
#[instrument(skip_all, fields(execution_id = %req.execution_id, voter = %req.voter_wallet, decision = %req.decision))]
pub async fn cast_vote(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CastVoteRequest>,
) -> ApiResult<Json<ApiResponse<Vote>>> {
    info!("Starting vote casting process");

    // In a real app, voter_wallet would come from JWT. Here we trust the request.
    let voter_wallet = &req.voter_wallet;

    // 1. Check if the voter is an approved committee member
    let is_approved = CommitteeMember::is_approved_member(&state.db, voter_wallet).await?;
    if !is_approved {
        error!(
            "Permission denied: Wallet {} is not an approved committee member.",
            voter_wallet
        );
        return Err(ApiError::Forbidden);
    }

    // 2. Start transaction
    let mut tx = state.db.begin().await?;

    // 3. Orchestrate vote creation within the transaction
    let new_vote = match async {
        // 3a. Check if the member has already voted for this execution
        if Vote::has_voted(&mut tx, req.execution_id, voter_wallet).await? {
            error!(
                "Vote rejected: Wallet {} has already voted on execution {}",
                voter_wallet, req.execution_id
            );
            return Err(ApiError::AlreadyExists(
                "You have already voted on this execution".to_string(),
            ));
        }

        // 3b. (Simulated) Call smart contract to cast the vote
        info!("Casting vote on blockchain...");
        let key_context =
            KeyContext::new(KeyKind::EthAccount, voter_wallet, "Casting a vote");
        let tx_hash = state
            .ctr_caller_service
            .cast_vote(req.execution_id, voter_wallet, &req.decision, &key_context)
            .await?;

        // 3c. Create the vote record in the database
        info!("Creating vote record in database...");
        let vote = Vote::create_in_tx(&mut tx, &req).await?;

        // 3d. Create the corresponding blockchain transaction record
        info!("Logging blockchain transaction...");
        let args = json!({
            "execution_id": req.execution_id,
            "voter_wallet": voter_wallet,
            "decision": req.decision,
        });
        BlockchainTransaction::create(&mut tx, &tx_hash, vote.id, ENTITY_TYPE_VOTE, &args).await?;

        Ok(vote)
    }
    .await
    {
        Ok(vote) => vote,
        Err(e) => {
            error!("Error during vote casting, rolling back: {}", e);
            tx.rollback().await?;
            return Err(e.into());
        }
    };

    // 4. Commit transaction
    tx.commit().await?;

    info!("Successfully cast vote with ID: {}", new_vote.id);
    Ok(Json(ApiResponse::success(new_vote)))
}

/// Get a vote by its ID
#[instrument(skip(state), fields(id = %id))]
pub async fn get_vote(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<Json<ApiResponse<Vote>>> {
    info!("Getting vote by ID");

    let vote = Vote::get_by_id(&state.db, id)
        .await?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(ApiResponse::success(vote)))
}

/// List votes with optional filtering and pagination
#[instrument(skip(state))]
pub async fn list_votes(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
    Query(vote_query): Query<VoteQuery>,
) -> ApiResult<Json<ApiResponse<PaginatedResponse<Vote>>>> {
    info!(
        voter_wallet = ?vote_query.voter_wallet,
        execution_id = ?vote_query.execution_id,
        "Listing votes"
    );

    let (votes, total) =
        Vote::list(&state.db, params.page as i64, params.limit as i64, vote_query).await?;

    let response = PaginatedResponse::new(votes, params.page, params.limit, total as u64);
    Ok(Json(ApiResponse::success(response)))
}
