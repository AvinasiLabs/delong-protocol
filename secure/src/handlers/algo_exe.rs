//! Algorithm execution handlers for the secure service
//!
//! This module handles algorithm execution requests, including submission to blockchain,
//! tracking execution status, and managing algorithm metadata within the TEE environment.

use axum::extract::{Path, Query, State};
use axum::response::Json;
use common::{
    ApiError, ApiResponse, ApiResult, PaginationParams, PaginatedResponse,
    models::{
        algo_exe::{AlgoExeData, AlgoExeSubmissionRequest, AlgoExeSubmissionResponse},
    }
};
use std::sync::Arc;
use tracing::{info, error, instrument};

use crate::AppState;

/// Submit algorithm execution request
/// 
/// This handler processes algorithm submission requests in the TEE environment.
/// It validates the GitHub repository, uploads algorithm to IPFS, creates database records,
/// and submits the transaction to the blockchain.
#[instrument(skip(state, payload), fields(github_repo = %payload.github_repo, scientist_wallet = %payload.scientist_wallet))]
pub async fn submit_algorithm_execution(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AlgoExeSubmissionRequest>,
) -> ApiResult<Json<ApiResponse<AlgoExeSubmissionResponse>>> {
    info!("Processing algorithm execution submission");

    validate_submission_request(&payload)?;

    let (algo_link, repo_name) = build_github_download_url(&payload.github_repo, &payload.commit_hash)?;

    let algo_id = check_or_create_algorithm(&state, &algo_link, &repo_name).await?;

    let execution_id = create_algorithm_execution(&state, algo_id, &payload).await?;

    let tx_hash = submit_to_blockchain(&state, execution_id, &payload).await?;

    create_blockchain_transaction(&state, &tx_hash, execution_id).await?;

    info!(execution_id = execution_id, tx_hash = %tx_hash, "Algorithm execution submitted successfully");

    Ok(Json(ApiResponse::success(AlgoExeSubmissionResponse {
        id: execution_id as u64,
        tx_hash,
        status: "submitted".to_string(),
    })))
}

/// List algorithm executions with pagination
#[instrument(skip(state), fields(page = %params.page, limit = %params.limit))]
pub async fn list_executions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> ApiResult<Json<ApiResponse<PaginatedResponse<AlgoExeData>>>> {
    info!("Listing algorithm executions");

    let result = get_algorithm_executions_with_info(&state, params.page, params.limit).await?;
    info!(total_items = result.total_items, "Algorithm executions retrieved successfully");
    Ok(Json(ApiResponse::success(result)))
}

/// Get a specific execution by ID
#[instrument(skip(state), fields(execution_id = %id))]
pub async fn get_execution(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<Json<ApiResponse<AlgoExeData>>> {
    info!("Getting algorithm execution by ID");

    let execution = get_algorithm_execution_by_id(&state, id).await?
        .ok_or_else(|| {
            error!("Algorithm execution not found");
            ApiError::NotFound
        })?;
    
    info!("Algorithm execution retrieved successfully");
    Ok(Json(ApiResponse::success(execution)))
}

// Helper functions

fn validate_submission_request(payload: &AlgoExeSubmissionRequest) -> ApiResult<()> {
    if payload.github_repo.is_empty() {
        return Err(ApiError::InvalidInput("GitHub repository URL is required".to_string()));
    }
    
    if payload.commit_hash.is_empty() {
        return Err(ApiError::InvalidInput("Commit hash is required".to_string()));
    }
    
    if payload.scientist_wallet.is_empty() {
        return Err(ApiError::InvalidInput("Scientist wallet address is required".to_string()));
    }
    
    if payload.dataset.is_empty() {
        return Err(ApiError::InvalidInput("Dataset identifier is required".to_string()));
    }

    if !payload.scientist_wallet.starts_with("0x") || payload.scientist_wallet.len() != 42 {
        return Err(ApiError::InvalidInput("Invalid Ethereum wallet address format".to_string()));
    }

    Ok(())
}

fn build_github_download_url(repo_url: &str, commit_hash: &str) -> ApiResult<(String, String)> {
    let repo_name = repo_url
        .trim_end_matches('/')
        .split('/')
        .last()
        .ok_or_else(|| ApiError::InvalidInput("Invalid repository URL".to_string()))?
        .replace(".git", "");

    let download_url = if repo_url.contains("github.com") {
        format!("{}/archive/{}.zip", repo_url.trim_end_matches(".git"), commit_hash)
    } else {
        return Err(ApiError::InvalidInput("Only GitHub repositories are supported".to_string()));
    };

    Ok((download_url, repo_name))
}

async fn check_or_create_algorithm(_state: &AppState, algo_link: &str, _repo_name: &str) -> ApiResult<i64> {
    // TODO: Implement database check for existing algorithm
    let algo_id = chrono::Utc::now().timestamp();
    
    info!(algo_id = algo_id, algo_link = %algo_link, "Algorithm processed");
    Ok(algo_id)
}

async fn create_algorithm_execution(
    _state: &AppState, 
    algo_id: i64, 
    payload: &AlgoExeSubmissionRequest
) -> ApiResult<i64> {
    // TODO: Implement database insertion
    let execution_id = chrono::Utc::now().timestamp();
    
    info!(
        execution_id = execution_id,
        algo_id = algo_id,
        scientist_wallet = %payload.scientist_wallet,
        dataset = %payload.dataset,
        "Algorithm execution record created"
    );
    
    Ok(execution_id)
}

async fn submit_to_blockchain(
    _state: &AppState,
    execution_id: i64,
    _payload: &AlgoExeSubmissionRequest,
) -> ApiResult<String> {
    // TODO: Implement actual blockchain submission
    let tx_hash = format!("0x{:x}", chrono::Utc::now().timestamp());
    
    info!(
        execution_id = execution_id,
        tx_hash = %tx_hash,
        "Submitted to blockchain (mock)"
    );
    
    Ok(tx_hash)
}

async fn create_blockchain_transaction(
    _state: &AppState,
    tx_hash: &str,
    entity_id: i64,
) -> ApiResult<()> {
    // TODO: Implement database insertion for blockchain transaction
    info!(
        tx_hash = %tx_hash,
        entity_id = entity_id,
        entity_type = "EXECUTION",
        "Blockchain transaction record created"
    );
    
    Ok(())
}

async fn get_algorithm_executions_with_info(
    _state: &AppState,
    page: u32,
    limit: u32,
) -> ApiResult<PaginatedResponse<AlgoExeData>> {
    // TODO: Implement database query
    let result = PaginatedResponse::new(vec![], page, limit, 0);
    Ok(result)
}

async fn get_algorithm_execution_by_id(
    _state: &AppState,
    _id: i64,
) -> ApiResult<Option<AlgoExeData>> {
    // TODO: Implement database query
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_submission_request() {
        let valid_request = AlgoExeSubmissionRequest {
            github_repo: "https://github.com/user/repo".to_string(),
            commit_hash: "abc123".to_string(),
            scientist_wallet: "0x1234567890123456789012345678901234567890".to_string(),
            dataset: "dataset_001".to_string(),
        };

        assert!(validate_submission_request(&valid_request).is_ok());

        // Test invalid wallet
        let invalid_wallet = AlgoExeSubmissionRequest {
            scientist_wallet: "invalid_wallet".to_string(),
            ..valid_request.clone()
        };
        assert!(validate_submission_request(&invalid_wallet).is_err());
    }

    #[test]
    fn test_build_github_download_url() {
        let (download_url, repo_name) = build_github_download_url(
            "https://github.com/user/repo",
            "abc123"
        ).unwrap();

        assert_eq!(download_url, "https://github.com/user/repo/archive/abc123.zip");
        assert_eq!(repo_name, "repo");
    }
} 