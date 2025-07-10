use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use tracing::info;

use common::{
    ApiResult,
    ApiResponse,
    models::vote::{CastVoteRequest, VoteData, VoteSummary, VoteQuery, VoteDecision},
};
use crate::AppState;
use crate::services::vote::{VoteService, Vote as DbVote};

fn to_vote_data(vote: DbVote) -> VoteData {
    VoteData {
        id: vote.id as u64,
        algo_cid: vote.execution_id.to_string(), // This needs to be fetched from algo table
        voter: vote.voter_wallet,
        approve: vote.decision == "APPROVE",
        voted_at: vote.created_at.to_rfc3339(),
        created_at: vote.created_at.to_rfc3339(),
        updated_at: vote.created_at.to_rfc3339(), // No updated_at in db model
    }
}

/// List votes with optional filtering
pub async fn list_votes(
    State(state): State<Arc<AppState>>,
    Query(query): Query<VoteQuery>,
) -> ApiResult<Json<ApiResponse<Vec<VoteData>>>> {
    info!("Listing votes");

    let page = query.page;
    let page_size = query.limit;

    // The service layer needs to be updated to take VoteQuery
    let votes = if let Some(voter) = query.voter {
        VoteService::get_votes_by_voter(&state.db_pool, &voter, page as i32, page_size as i32).await?
    } else {
        vec![]
    };

    let response_votes = votes.into_iter().map(to_vote_data).collect();
    Ok(Json(ApiResponse::success(response_votes)))
}

/// Cast a vote for an algorithm execution
pub async fn cast_vote(
    State(state): State<Arc<AppState>>,
    Path(execution_id): Path<i64>,
    Json(request): Json<CastVoteRequest>,
) -> ApiResult<Json<ApiResponse<VoteData>>> {
    info!(execution_id = %execution_id, voter = %request.signature.as_deref().unwrap_or(""), decision = ?request.decision, "Casting vote");

    let decision_str = match request.decision {
        VoteDecision::Approve => "APPROVE",
        VoteDecision::Reject => "REJECT",
        VoteDecision::Abstain => "ABSTAIN",
    };

    let vote_request = crate::services::vote::CastVoteRequest {
        execution_id,
        voter_wallet: "0x...".to_string(), // This should come from auth context
        decision: decision_str.to_string(),
    };
    
    let vote = VoteService::cast_vote(&state.db_pool, vote_request).await?;
    Ok(Json(ApiResponse::success(to_vote_data(vote))))
}

/// Get vote tally for an execution
pub async fn get_vote_tally(
    State(state): State<Arc<AppState>>,
    Path(execution_id): Path<i64>,
) -> ApiResult<Json<ApiResponse<VoteSummary>>> {
    info!(execution_id = %execution_id, "Getting vote tally");

    let tally = VoteService::get_vote_tally(&state.db_pool, execution_id).await?;
    let is_complete = VoteService::is_voting_complete(&state.db_pool, execution_id).await?;
    let is_approved = if is_complete {
        VoteService::is_execution_approved(&state.db_pool, execution_id).await?
    } else {
        false
    };

    let summary = VoteSummary {
        algo_cid: execution_id.to_string(),
        total_votes: tally.total_votes as u32,
        approve_votes: tally.approve_votes as u32,
        reject_votes: tally.reject_votes as u32,
        abstain_votes: 0, // Not supported in service yet
        approval_percentage: if tally.total_votes > 0 {
            (tally.approve_votes as f64 / tally.total_votes as f64) * 100.0
        } else {
            0.0
        },
        passed: is_approved,
    };

    Ok(Json(ApiResponse::success(summary)))
}

/// Get votes for a specific execution
pub async fn get_execution_votes(
    State(state): State<Arc<AppState>>,
    Path(execution_id): Path<i64>,
    Query(query): Query<VoteQuery>,
) -> ApiResult<Json<ApiResponse<Vec<VoteData>>>> {
    info!(execution_id = %execution_id, "Getting votes for execution");

    let page = query.page;
    let page_size = query.limit;

    let votes = VoteService::get_votes_for_execution(&state.db_pool, execution_id as i32, page as i32, page_size as i32).await?;

    let response_votes = votes.into_iter().map(to_vote_data).collect();

    Ok(Json(ApiResponse::success(response_votes)))
}

/// Check if a specific voter has voted on an execution
pub async fn check_voter_voted(
    State(state): State<Arc<AppState>>,
    Path((execution_id, voter_wallet)): Path<(i64, String)>,
) -> ApiResult<Json<ApiResponse<bool>>> {
    info!(execution_id = %execution_id, voter = %voter_wallet, "Checking if voter has voted");

    let has_voted = VoteService::has_voter_voted(&state.db_pool, execution_id as i32, &voter_wallet).await?;
    Ok(Json(ApiResponse::success(has_voted)))
} 