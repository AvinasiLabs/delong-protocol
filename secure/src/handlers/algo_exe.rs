//! Algorithm execution handlers for the secure service
//!
//! This module handles algorithm execution requests, including submission to blockchain,
//! tracking execution status, and managing algorithm metadata within the TEE environment.

use crate::{
    app_state::AppState,
    models::{
        algo_exe::{
            AlgoExe, AlgoExeWithAlgo, CreateAlgoExeRequest, SubmitAlgoExeRequest,
            SubmitAlgoExeResponse,
        },
        algorithm::{Algorithm, CreateAlgorithmRequest},
        blockchain::{BlockchainTransaction, ENTITY_TYPE_EXECUTION},
    },
    services::key_ctx::{KeyContext, KeyKind, KEY_CTX_TEE_CONTRACT_OWNER},
};
use axum::extract::{Path, Query, State};
use axum::Json;
use common::{ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginationParams};
use std::sync::Arc;
use tracing::{error, info, instrument};

/// Submit algorithm execution request
///
/// This handler processes algorithm submission requests in the TEE environment.
/// It validates the GitHub repository, uploads algorithm to IPFS, creates database records,
/// and submits the transaction to the blockchain.
#[instrument(skip(state, payload), fields(github_repo = %payload.github_repo, scientist_wallet = %payload.scientist_wallet))]
pub async fn submit_algorithm_execution(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SubmitAlgoExeRequest>,
) -> ApiResult<Json<ApiResponse<SubmitAlgoExeResponse>>> {
    info!("Processing algorithm execution submission, aligning with Go logic");

    let (algo_link, repo_name) =
        build_github_download_url(&payload.github_repo, &payload.commit_hash)
            .map_err(|_| ApiError::BadRequest)?;

    // Step 1 & 2: Check if algorithm exists and determine its CID.
    let (algorithm, algo_cid) = match Algorithm::get_by_link(&state.db, &algo_link).await? {
        Some(existing_algo) => {
            info!(algo_id = %existing_algo.id, "Found existing algorithm");
            (existing_algo, existing_algo.cid.clone())
        }
        None => {
            info!("Algorithm not found, creating a new one");

            // In the Go reference, the code is downloaded and uploaded to IPFS.
            // We simulate this process here.
            let new_algo_cid = state.ipfs_service.upload_stream().await.map_err(|e| {
                error!("IPFS service error: {}", e);
                ApiError::InternalError
            })?;

            let create_algo_req = CreateAlgorithmRequest {
                name: repo_name,
                algo_link: algo_link.clone(),
                cid: new_algo_cid.clone(),
            };
            let new_algo = Algorithm::create(&state.db, create_algo_req).await?;
            info!(algo_id = %new_algo.id, "Created new algorithm record");
            (new_algo, new_algo_cid)
        }
    };

    // Step 3: Start a transaction to create the execution and blockchain records.
    let mut tx = state.db.begin().await?;

    let create_exe_req = CreateAlgoExeRequest {
        algo_id: algorithm.id,
        used_dataset: payload.dataset.clone(),
        scientist_wallet: payload.scientist_wallet.clone(),
    };

    let algo_exe = AlgoExe::create(&mut tx, create_exe_req).await?;

    // Step 4: Call the smart contract.
    let key_context = KeyContext::new(
        KeyKind::EthAccount,
        KEY_CTX_TEE_CONTRACT_OWNER,
        "Submitting an algorithm execution",
    );
    let tx_hash = state
        .ctr_caller_service
        .submit_algorithm_execution(
            algo_exe.id,
            &payload.scientist_wallet,
            &algo_cid,
            &payload.dataset,
            &key_context,
        )
        .await
        .map_err(|e| {
            error!("Contract caller error: {}", e);
            ApiError::InternalError
        })?;

    // Step 5: Create the blockchain transaction record.
    let args = serde_json::json!({
        "algo_id": algorithm.id,
        "algo_cid": algo_cid,
        "dataset": payload.dataset
    });
    BlockchainTransaction::create(&mut tx, &tx_hash, algo_exe.id, ENTITY_TYPE_EXECUTION, &args)
        .await?;

    // Step 6: Commit the transaction.
    tx.commit().await?;

    info!(tx_hash = %tx_hash, "Successfully submitted algorithm execution");
    Ok(Json(ApiResponse::success(SubmitAlgoExeResponse {
        id: algo_exe.id,
        tx_hash,
        status: "submitted".to_string(),
    })))
}

/// List algorithm executions with pagination
#[instrument(skip(state), fields(page = %params.page, limit = %params.limit))]
pub async fn list_executions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> ApiResult<Json<ApiResponse<PaginatedResponse<AlgoExeWithAlgo>>>> {
    info!("Listing algorithm executions");
    let (executions, total) =
        AlgoExe::get_paginated(&state.db, params.page as i64, params.limit as i64).await?;
    let response = PaginatedResponse::new(executions, params.page, params.limit, total as u64);
    Ok(Json(ApiResponse::success(response)))
}

/// Get a specific execution by ID
#[instrument(skip(state), fields(execution_id = %id))]
pub async fn get_execution(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<Json<ApiResponse<AlgoExe>>> {
    info!("Getting algorithm execution by ID");

    let execution = AlgoExe::get_by_id(&state.db, id).await?.ok_or(ApiError::NotFound)?;

    info!("Algorithm execution retrieved successfully");
    Ok(Json(ApiResponse::success(execution)))
}

fn build_github_download_url(repo_url: &str, commit_hash: &str) -> Result<(String, String), ()> {
    let repo_name = repo_url
        .trim_end_matches('/')
        .split('/')
        .last()
        .ok_or(())?
        .replace(".git", "");

    let download_url = if repo_url.contains("github.com") {
        format!(
            "{}/archive/{}.zip",
            repo_url.trim_end_matches(".git"),
            commit_hash
        )
    } else {
        return Err(());
    };

    Ok((download_url, repo_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_github_download_url() {
        let (download_url, repo_name) =
            build_github_download_url("https://github.com/user/repo", "abc123").unwrap();

        assert_eq!(
            download_url,
            "https://github.com/user/repo/archive/abc123.zip"
        );
        assert_eq!(repo_name, "repo");
    }
} 