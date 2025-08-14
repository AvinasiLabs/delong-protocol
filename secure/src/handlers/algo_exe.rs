//! Algorithm execution handlers
//!
//! This module provides HTTP handlers for managing algorithm executions,
//! including submission, listing, and retrieval of execution details.

use crate::{
    models::{
        algo::{Algo, CreateAlgo},
        algo_exe::{AlgoExe, AlgoExeWithAlgo, CreateAlgoExe},
        blockchain_transaction::{CreateTransaction, EntityType},
        pg_types::{ExecutionStatus, ReviewStatus},
    },
    models::{Create, FindById},
    routes::AppState,
};
use alloy::primitives::Address;
use avinapi::prelude::{
    data, paginated, AppError, JsonResult, PaginatedResult, PaginationQuery, ValidatedJson,
    ValidatedQuery,
};
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{info, instrument};
use validator::Validate;

/// Request structure for submitting algorithm execution
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SubmitAlgoExeRequest {
    /// Scientist's Ethereum wallet address
    #[validate(regex(
        path = "crate::ETHEREUM_ADDRESS_REGEX",
        message = "Invalid Ethereum address format"
    ))]
    pub scientist_wallet: String,
    /// Dataset name to use for execution
    #[validate(length(
        min = 1,
        max = 100,
        message = "Dataset name must be between 1 and 100 characters"
    ))]
    pub dataset: String,
    /// GitHub repository URL
    #[validate(url(message = "Invalid GitHub repository URL"))]
    #[validate(regex(
        path = "crate::GITHUB_URL_REGEX",
        message = "Must be a valid GitHub URL"
    ))]
    pub github_repo: String,
    /// Git commit hash
    #[validate(regex(
        path = "crate::GIT_COMMIT_REGEX",
        message = "Invalid git commit hash format"
    ))]
    pub commit_hash: String,
}

/// Response structure for algorithm execution submission
#[derive(Debug, Serialize)]
pub struct SubmitAlgoExeResponse {
    /// Transaction hash on blockchain
    pub tx_hash: String,
}

/// Submit a new algorithm execution
#[instrument(skip(state))]
#[axum::debug_handler]
pub async fn submit_algo_exe(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<SubmitAlgoExeRequest>,
) -> JsonResult<SubmitAlgoExeResponse> {
    // Validate scientist wallet address
    let scientist_address = Address::from_str(&req.scientist_wallet)
        .map_err(|_| AppError::Validation("Invalid scientist wallet address".into()))?;

    // Build GitHub download URL
    let (algo_link, repo_name) = build_github_download_url(&req.github_repo, &req.commit_hash)?;

    // Check if an algorithm already exists
    let algo = match Algo::find_by_link(state.db.pool(), &algo_link).await? {
        Some(existing_algo) => {
            info!("Algorithm already exists with ID: {}", existing_algo.id);
            existing_algo
        }
        None => {
            info!("New algorithm, downloading from GitHub");

            // Download algorithm from GitHub
            let response = reqwest::get(&algo_link).await.map_err(|e| {
                AppError::Validation(format!("Failed to download algorithm: {}", e))
            })?;

            if !response.status().is_success() {
                return Err(AppError::Validation(
                    "Invalid GitHub repository or commit hash".into(),
                ));
            }

            // Upload to IPFS
            let algo_bytes = response.bytes().await.map_err(|e| {
                AppError::Internal(format!("Failed to read algorithm content: {}", e))
            })?;

            // Upload to IPFS
            use ipfs_api_backend_hyper::IpfsApi;
            use std::io::Cursor;
            let add_response = state
                .ipfs_client
                .add(Cursor::new(algo_bytes.to_vec()))
                .await
                .map_err(|e| AppError::Internal(format!("Failed to upload to IPFS: {}", e)))?;

            let algo_cid = add_response.hash;

            info!("Algorithm uploaded to IPFS with CID: {}", algo_cid);

            // Create an algorithm record
            let create_algo = CreateAlgo {
                name: repo_name,
                algo_link,
                cid: algo_cid.clone(),
            };

            <Algo as Create>::create(state.db.pool(), create_algo).await?
        }
    };

    // Start database transaction
    let mut tx = state.db.pool().begin().await?;

    // Create an algorithm execution record
    let create_exe = CreateAlgoExe {
        algo_id: algo.id,
        status: ExecutionStatus::Queued,
        used_dataset: req.dataset.clone(),
        scientist_wallet: req.scientist_wallet.clone(),
        review_status: ReviewStatus::Reviewing,
    };

    let algo_exe = AlgoExe::create_with_tx(&mut tx, create_exe).await?;

    // Submit to blockchain with execution_id
    let tx_hash = state
        .contract_caller
        .submit_algorithm(
            algo_exe.id,
            scientist_address,
            algo.cid.clone(),
            req.dataset.clone(),
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to submit to blockchain: {}", e)))?;

    info!(
        "Algorithm submitted to blockchain with tx hash: {}",
        tx_hash
    );

    // Create a blockchain transaction record
    let create_tx = CreateTransaction {
        tx_hash: tx_hash.clone(),
        entity_id: algo_exe.id,
        entity_type: EntityType::Execution,
    };

    CreateTransaction::create(&mut tx, create_tx).await?;

    // Commit transaction
    tx.commit().await?;

    info!("Algorithm execution {} submitted", algo_exe.id);

    data!(SubmitAlgoExeResponse { tx_hash })
}

/// List all algorithm executions with pagination
#[instrument(skip(state))]
#[axum::debug_handler]
pub async fn list_algo_exes(
    State(state): State<AppState>,
    ValidatedQuery(params): ValidatedQuery<PaginationQuery>,
) -> PaginatedResult<AlgoExeWithAlgo> {
    // Use AlgoExeWithAlgo to get algorithm info along with execution data
    let (items, total) =
        AlgoExeWithAlgo::list(state.db.pool(), None, params.page, params.per_page).await?;

    paginated!(items, total, params.page, params.per_page)
}

/// Get a specific algorithm execution by ID
#[instrument(skip(state))]
pub async fn get_algo_exe(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> JsonResult<AlgoExe> {
    let algo_exe = AlgoExe::find_by_id(state.db.pool(), id as i64)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Algorithm execution {} not found", id)))?;

    data!(algo_exe)
}

/// Build GitHub download URL from a repository and commit hash
fn build_github_download_url(
    github_repo: &str,
    commit_hash: &str,
) -> Result<(String, String), AppError> {
    // Extract owner and repo name from GitHub URL
    // Expected format: https://github.com/owner/repo or github.com/owner/repo
    let repo_path = github_repo
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("github.com/")
        .trim_end_matches(".git")
        .trim_end_matches('/');

    let parts: Vec<&str> = repo_path.split('/').collect();
    if parts.len() < 2 {
        return Err(AppError::Validation("Invalid GitHub repository URL".into()));
    }

    let owner = parts[0];
    let repo_name = parts[1];

    // Build download URL
    let download_url = format!(
        "https://github.com/{}/{}/archive/{}.tar.gz",
        owner, repo_name, commit_hash
    );

    Ok((download_url, repo_name.to_string()))
}
