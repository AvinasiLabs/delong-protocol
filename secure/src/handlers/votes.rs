use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use common::ApiResponse;
use crate::AppState;
use crate::services::VoteService;

/// Query parameters for listing votes
#[derive(Debug, Deserialize)]
pub struct ListVotesQuery {
    pub execution_id: Option<i32>,
    pub voter_wallet: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

/// Request to cast a vote
#[derive(Debug, Deserialize, Serialize)]
pub struct CastVoteRequest {
    pub voter_wallet: String,
    pub decision: String, // "APPROVE" or "REJECT"
}

/// Response for a vote
#[derive(Debug, Serialize)]
pub struct VoteResponse {
    pub id: i32,
    pub execution_id: i32,
    pub voter_wallet: String,
    pub decision: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Vote tally response
#[derive(Debug, Serialize)]
pub struct VoteTallyResponse {
    pub execution_id: i32,
    pub total_votes: i32,
    pub approve_votes: i32,
    pub reject_votes: i32,
    pub is_complete: bool,
    pub is_approved: bool,
    pub required_majority_percent: f64,
}

/// List votes with optional filtering
pub async fn list_votes(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListVotesQuery>,
) -> Json<ApiResponse<Vec<VoteResponse>>> {
    info!("Listing votes");

    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20);

    let votes_result = if let Some(execution_id) = query.execution_id {
        VoteService::get_votes_for_execution(&state.db_pool, execution_id, page as i32, page_size as i32).await
    } else if let Some(voter_wallet) = query.voter_wallet {
        VoteService::get_votes_by_voter(&state.db_pool, &voter_wallet, page as i32, page_size as i32).await
    } else {
        // Return empty list if no filter specified
        Ok(vec![])
    };

    match votes_result {
        Ok(votes) => {
            let response_votes: Vec<VoteResponse> = votes.into_iter().map(|vote| {
                VoteResponse {
                    id: vote.id as i32,
                    execution_id: vote.execution_id as i32,
                    voter_wallet: vote.voter_wallet,
                    decision: vote.decision,
                    created_at: vote.created_at,
                }
            }).collect();

            Json(ApiResponse::success(response_votes))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list votes");
            Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to list votes".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
}

/// Cast a vote for an algorithm execution
pub async fn cast_vote(
    State(state): State<Arc<AppState>>,
    Path(execution_id): Path<i32>,
    Json(request): Json<CastVoteRequest>,
) -> Json<ApiResponse<VoteResponse>> {
    info!(execution_id = %execution_id, voter = %request.voter_wallet, decision = %request.decision, "Casting vote");

    // Validate decision
    if request.decision != "APPROVE" && request.decision != "REJECT" {
        return Json(ApiResponse {
            code: common::ResponseCode::BadRequest,
            data: None,
            message: "Decision must be either 'APPROVE' or 'REJECT'".to_string(),
            request_id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    // Cast the vote
    let vote_request = crate::services::vote::CastVoteRequest {
        execution_id: execution_id as i64,
        voter_wallet: request.voter_wallet.clone(),
        decision: request.decision.clone(),
    };
    
    match VoteService::cast_vote(&state.db_pool, vote_request).await {
        Ok(vote) => {
            let response = VoteResponse {
                id: vote.id as i32,
                execution_id: vote.execution_id as i32,
                voter_wallet: vote.voter_wallet,
                decision: vote.decision,
                created_at: vote.created_at,
            };
            Json(ApiResponse::success(response))
        }
        Err(e) => {
            tracing::error!(error = %e, execution_id = %execution_id, voter = %request.voter_wallet, "Failed to cast vote");
            Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to cast vote".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
}

/// Get vote tally for an execution
pub async fn get_vote_tally(
    State(state): State<Arc<AppState>>,
    Path(execution_id): Path<i32>,
) -> Json<ApiResponse<VoteTallyResponse>> {
    info!(execution_id = %execution_id, "Getting vote tally");

    match VoteService::get_vote_tally(&state.db_pool, execution_id as i64).await {
        Ok(tally) => {
            // Check if voting is complete and approved
            let is_complete = VoteService::is_voting_complete(&state.db_pool, execution_id as i64).await.unwrap_or(false);
            let is_approved = if is_complete {
                VoteService::is_execution_approved(&state.db_pool, execution_id as i64).await.unwrap_or(false)
            } else {
                false
            };

            let response = VoteTallyResponse {
                execution_id,
                total_votes: tally.total_votes as i32,
                approve_votes: tally.approve_votes as i32,
                reject_votes: tally.reject_votes as i32,
                is_complete,
                is_approved,
                required_majority_percent: 66.7, // Default majority threshold
            };

            Json(ApiResponse::success(response))
        }
        Err(e) => {
            tracing::error!(error = %e, execution_id = %execution_id, "Failed to get vote tally");
            Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to get vote tally".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
}

/// Get votes for a specific execution
pub async fn get_execution_votes(
    State(state): State<Arc<AppState>>,
    Path(execution_id): Path<i32>,
    Query(query): Query<ListVotesQuery>,
) -> Json<ApiResponse<Vec<VoteResponse>>> {
    info!(execution_id = %execution_id, "Getting votes for execution");

    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20);

    match VoteService::get_votes_for_execution(&state.db_pool, execution_id, page as i32, page_size as i32).await {
        Ok(votes) => {
            let response_votes: Vec<VoteResponse> = votes.into_iter().map(|vote| {
                VoteResponse {
                    id: vote.id as i32,
                    execution_id: vote.execution_id as i32,
                    voter_wallet: vote.voter_wallet,
                    decision: vote.decision,
                    created_at: vote.created_at,
                }
            }).collect();

            Json(ApiResponse::success(response_votes))
        }
        Err(e) => {
            tracing::error!(error = %e, execution_id = %execution_id, "Failed to get execution votes");
            Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to get execution votes".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
}

/// Check if a specific voter has voted on an execution
pub async fn check_voter_voted(
    State(state): State<Arc<AppState>>,
    Path((execution_id, voter_wallet)): Path<(i32, String)>,
) -> Json<ApiResponse<bool>> {
    info!(execution_id = %execution_id, voter = %voter_wallet, "Checking if voter has voted");

    match VoteService::has_voter_voted(&state.db_pool, execution_id, &voter_wallet).await {
        Ok(has_voted) => Json(ApiResponse::success(has_voted)),
        Err(e) => {
            tracing::error!(error = %e, execution_id = %execution_id, voter = %voter_wallet, "Failed to check if voter has voted");
            Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to check voting status".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
} 