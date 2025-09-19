//! Algorithm execution handlers
//!
//! This module provides HTTP handlers for managing algorithm executions,
//! including submission, listing, and retrieval of execution details.

use crate::{
    infra::ai_audit::AiAuditService,
    middleware::AuthenticatedUser,
    models::{
        algorithm::{Algo, CreateAlgo},
        algorithm_execution::AlgorithmExecution,
        blockchain_transaction::BlockchainTransaction,
        Create, FindById,
    },
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
    pub wallet: String,
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
pub async fn submit_execution(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    ValidatedJson(req): ValidatedJson<SubmitAlgoExeRequest>,
) -> JsonResult<SubmitAlgoExeResponse> {
    // Validate scientist wallet address
    let scientist_address = Address::from_str(&req.wallet)
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

            // Perform AI audit on the algorithm
            let ai_audit_service = AiAuditService::mock();
            let audit_result = ai_audit_service.audit_algorithm(&algo_cid).await?;

            info!(
                "AI audit completed for CID {}: approved={}, risk_score={}",
                algo_cid, audit_result.approved, audit_result.risk_score
            );

            // Check if audit passed
            if !audit_result.approved {
                return Err(AppError::Validation(format!(
                    "Algorithm failed AI audit: {}",
                    audit_result.message
                )));
            }

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
    // Since AI audit already passed, we can set status directly to approved
    let algo_exe = sqlx::query!(
        r#"
        INSERT INTO algorithm_execution (
            algo_id, algo_name, algo_cid, algo_link,
            dataset_id, dataset_name, dataset_cid,
            wallet, user_id,
            review_status, execution_status,
            submitted_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'approved', 'queued', NOW())
        RETURNING id
        "#,
        algo.id,
        Some(algo.name.clone()),
        algo.cid.clone(),
        Some(algo.algo_link.clone()),
        None::<i64>, // dataset_id will be resolved later
        req.dataset.clone(),
        None::<String>, // dataset_cid will be resolved later
        req.wallet.clone(),
        auth_user.user_id.parse().unwrap_or(0) // None::<i64> // user_id
    )
    .fetch_one(&mut *tx)
    .await?;

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

    // Commit transaction first
    tx.commit().await?;

    // Create blockchain transaction record using conditional insert
    // This will not overwrite if a confirmed record already exists
    let was_updated =
        BlockchainTransaction::upsert_for_execution(state.db.pool(), &tx_hash, algo_exe.id).await?;

    if was_updated {
        info!(
            "Blockchain transaction record created for execution {}",
            algo_exe.id
        );
    } else {
        info!("Blockchain transaction record already exists and is confirmed, skipped update");
    }

    info!("Algorithm execution {} submitted", algo_exe.id);

    data!(SubmitAlgoExeResponse { tx_hash })
}

/// List all algorithm executions with pagination
#[instrument(skip(state))]
pub async fn list_executions(
    State(state): State<AppState>,
    ValidatedQuery(params): ValidatedQuery<PaginationQuery>,
) -> PaginatedResult<AlgorithmExecution> {
    // Get executions from the new algorithm_execution table
    let executions = sqlx::query_as!(
        AlgorithmExecution,
        r#"
        SELECT
            id, algo_id, algo_name, algo_cid, algo_link,
            dataset_id, dataset_name, dataset_cid,
            wallet, user_id,
            review_status, execution_status,
            vote_start_time, vote_end_time,
            submitted_at, started_at, completed_at,
            success, output, error_message, error_type, container_exit_code,
            runtime_seconds, created_at, updated_at
        FROM algorithm_execution
        ORDER BY submitted_at DESC
        LIMIT $1 OFFSET $2
        "#,
        params.get_limit() as i64,
        params.get_offset() as i64
    )
    .fetch_all(state.db.pool())
    .await?;

    let total = sqlx::query_scalar!("SELECT COUNT(*) as \"count!\" FROM algorithm_execution")
        .fetch_one(state.db.pool())
        .await?;

    paginated!(
        executions,
        total as u64,
        params.get_page(),
        params.get_per_page()
    )
}

/// Get a specific algorithm execution by ID
#[instrument(skip(state))]
pub async fn get_execution(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> JsonResult<AlgorithmExecution> {
    let algo_exe = AlgorithmExecution::find_by_id(state.db.pool(), id as i64)
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
