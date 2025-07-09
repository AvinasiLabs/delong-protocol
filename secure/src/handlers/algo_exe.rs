//! Algorithm execution handlers for the secure service
//!
//! This module handles algorithm execution requests, including submission to blockchain,
//! tracking execution status, and managing algorithm metadata within the TEE environment.

use axum::extract::{Path, Query, State};
use axum::response::Json;
use axum::http::StatusCode;
use common::{
    ApiResponse, ApiResult, PaginationParams, PaginatedResponse,
    AlgoExeData, AlgoExeSubmissionRequest, AlgoExeSubmissionResponse,
};
use std::sync::Arc;
use tracing::{info, error, warn, instrument};
// use chrono::Utc; // Will be used when implementing actual time tracking

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
) -> Result<Json<ApiResponse<AlgoExeSubmissionResponse>>, StatusCode> {
    info!("Processing algorithm execution submission");

    // Validate input parameters
    if let Err(e) = validate_submission_request(&payload) {
        error!("Invalid submission request: {}", e);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Build GitHub download URL
    let (algo_link, repo_name) = match build_github_download_url(&payload.github_repo, &payload.commit_hash) {
        Ok(urls) => urls,
        Err(e) => {
            error!("Failed to build GitHub download URL: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Check if algorithm already exists
    let algo_result = check_or_create_algorithm(&state, &algo_link, &repo_name).await;
    let algo_id = match algo_result {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to process algorithm: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Create algorithm execution record
    let execution_id = match create_algorithm_execution(&state, algo_id, &payload).await {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to create algorithm execution: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Submit to blockchain (mock implementation for now)
    let tx_hash = match submit_to_blockchain(&state, execution_id, &payload).await {
        Ok(hash) => hash,
        Err(e) => {
            error!("Failed to submit to blockchain: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Create blockchain transaction record
    if let Err(e) = create_blockchain_transaction(&state, &tx_hash, execution_id).await {
        error!("Failed to create blockchain transaction record: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    info!(execution_id = execution_id, tx_hash = %tx_hash, "Algorithm execution submitted successfully");

    Ok(Json(ApiResponse::success(AlgoExeSubmissionResponse {
        id: execution_id as u64,
        tx_hash,
        status: "submitted".to_string(),
        message: "Algorithm execution submitted successfully".to_string(),
    })))
}

/// List algorithm executions with pagination
#[instrument(skip(state), fields(page = %params.page, limit = %params.limit))]
pub async fn list_executions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ApiResponse<PaginatedResponse<AlgoExeData>>>, StatusCode> {
    info!("Listing algorithm executions");

    match get_algorithm_executions_with_info(&state, params.page, params.limit).await {
        Ok(result) => {
            info!(total_items = result.total_items, "Algorithm executions retrieved successfully");
            Ok(Json(ApiResponse::success(result)))
        }
        Err(e) => {
            error!("Failed to list algorithm executions: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get a specific execution by ID
#[instrument(skip(state), fields(execution_id = %id))]
pub async fn get_execution(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<AlgoExeData>>, StatusCode> {
    info!("Getting algorithm execution by ID");

    match get_algorithm_execution_by_id(&state, id).await {
        Ok(Some(execution)) => {
            info!("Algorithm execution retrieved successfully");
            Ok(Json(ApiResponse::success(execution)))
        }
        Ok(None) => {
            warn!("Algorithm execution not found");
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            error!("Failed to get algorithm execution: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Helper functions

fn validate_submission_request(payload: &AlgoExeSubmissionRequest) -> Result<(), String> {
    if payload.github_repo.is_empty() {
        return Err("GitHub repository URL is required".to_string());
    }
    
    if payload.commit_hash.is_empty() {
        return Err("Commit hash is required".to_string());
    }
    
    if payload.scientist_wallet.is_empty() {
        return Err("Scientist wallet address is required".to_string());
    }
    
    if payload.dataset.is_empty() {
        return Err("Dataset identifier is required".to_string());
    }

    // Validate Ethereum address format
    if !payload.scientist_wallet.starts_with("0x") || payload.scientist_wallet.len() != 42 {
        return Err("Invalid Ethereum wallet address format".to_string());
    }

    Ok(())
}

fn build_github_download_url(repo_url: &str, commit_hash: &str) -> Result<(String, String), String> {
    // Extract repository name from URL
    let repo_name = repo_url
        .trim_end_matches('/')
        .split('/')
        .last()
        .ok_or("Invalid repository URL")?
        .replace(".git", "");

    // Build download URL for the specific commit
    let download_url = if repo_url.contains("github.com") {
        format!("{}/archive/{}.zip", repo_url.trim_end_matches(".git"), commit_hash)
    } else {
        return Err("Only GitHub repositories are supported".to_string());
    };

    Ok((download_url, repo_name))
}

async fn check_or_create_algorithm(_state: &AppState, algo_link: &str, _repo_name: &str) -> ApiResult<i64> {
    // TODO: Implement database check for existing algorithm
    // For now, generate a mock algorithm ID
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
    // For now, generate a mock execution ID
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
    // For now, return a mock transaction hash
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
    // For now, return empty result
    let result = PaginatedResponse::new(vec![], page, limit, 0);
    Ok(result)
}

async fn get_algorithm_execution_by_id(
    _state: &AppState,
    _id: i64,
) -> ApiResult<Option<AlgoExeData>> {
    // TODO: Implement database query
    // For now, return None
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