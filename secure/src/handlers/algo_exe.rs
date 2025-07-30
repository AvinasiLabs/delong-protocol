//! Algorithm execution handlers
//!
//! This module provides HTTP handlers for managing algorithm executions,
//! including submission, listing, and retrieval of execution details.

use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use ethers::types::Address;
use serde::{Deserialize, Serialize};
use std::{str::FromStr, sync::Arc};
use tracing::{info, instrument};

use crate::{
    models::{
        Create, FindById,
        algo::{Algo, CreateAlgo},
        algo_exe::{AlgoExe, AlgoExeWithAlgo, CreateAlgoExe},
        blockchain_transaction::{CreateTransaction, EntityType},
        pg_types::{AlgoExeStatus, AlgoReviewStatus},
    },
    routes::AppState,
};
use avinapi::query::pagination::{PaginatedData, PaginationMeta, PaginationQuery};
use avinapi::response::JsonResult;

/// Request structure for submitting algorithm execution
#[derive(Debug, Deserialize)]
pub struct SubmitAlgoExeRequest {
    /// Scientist's Ethereum wallet address
    pub scientist_wallet: String,
    /// Dataset name to use for execution
    pub dataset: String,
    /// GitHub repository URL
    pub github_repo: String,
    /// Git commit hash
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
pub async fn submit_algo_exe(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SubmitAlgoExeRequest>,
) -> JsonResult<SubmitAlgoExeResponse> {
    // Validate scientist wallet address
    let scientist_address = Address::from_str(&req.scientist_wallet).map_err(|_| {
        avinapi::error::AppError::Validation("Invalid scientist wallet address".into())
    })?;

    // Build GitHub download URL
    let (algo_link, repo_name) = build_github_download_url(&req.github_repo, &req.commit_hash)?;

    // Check if algorithm already exists
    let algo = match Algo::find_by_link(state.db.pool(), &algo_link).await? {
        Some(existing_algo) => {
            info!("Algorithm already exists with ID: {}", existing_algo.id);
            existing_algo
        }
        None => {
            info!("New algorithm, downloading from GitHub");

            // Download algorithm from GitHub
            let response = reqwest::get(&algo_link).await.map_err(|e| {
                avinapi::error::AppError::Validation(format!("Failed to download algorithm: {}", e))
            })?;

            if !response.status().is_success() {
                return Err(avinapi::error::AppError::Validation(
                    "Invalid GitHub repository or commit hash".into(),
                ));
            }

            // Upload to IPFS
            let algo_bytes = response.bytes().await.map_err(|e| {
                avinapi::error::AppError::Internal(format!(
                    "Failed to read algorithm content: {}",
                    e
                ))
            })?;

            // Upload to IPFS
            use ipfs_api_backend_hyper::IpfsApi;
            use std::io::Cursor;
            let add_response = state
                .ipfs_client
                .add(Cursor::new(algo_bytes.to_vec()))
                .await
                .map_err(|e| {
                    avinapi::error::AppError::Internal(format!("Failed to upload to IPFS: {}", e))
                })?;

            let algo_cid = add_response.hash;

            info!("Algorithm uploaded to IPFS with CID: {}", algo_cid);

            // Create algorithm record
            let create_algo = CreateAlgo {
                name: repo_name,
                algo_link: algo_link.clone(),
                cid: algo_cid.clone(),
            };

            <Algo as Create>::create(state.db.pool(), create_algo).await?
        }
    };

    // Start database transaction
    let mut tx = state.db.pool().begin().await?;

    // Create algorithm execution record
    let create_exe = CreateAlgoExe {
        algo_id: algo.id,
        status: AlgoExeStatus::Queued,
        used_dataset: req.dataset.clone(),
        scientist_wallet: req.scientist_wallet.clone(),
        review_status: AlgoReviewStatus::Reviewing,
    };

    let algo_exe = AlgoExe::create(&mut tx, create_exe).await?;

    // Submit to blockchain
    let tx_receipt = state
        .contract_caller
        .submit_algorithm(
            algo_exe.id as u64,
            scientist_address,
            &algo.cid,
            &req.dataset,
        )
        .await
        .map_err(|e| {
            avinapi::error::AppError::Internal(format!("Failed to submit to blockchain: {}", e))
        })?;

    let tx_hash = format!("{:?}", tx_receipt.transaction_hash);

    info!(
        "Algorithm submitted to blockchain with tx hash: {}",
        tx_hash
    );

    // Create blockchain transaction record
    let create_tx = CreateTransaction {
        tx_hash: tx_hash.clone(),
        entity_id: algo_exe.id,
        entity_type: EntityType::Execution,
    };

    CreateTransaction::create(&mut tx, create_tx).await?;

    // Commit transaction
    tx.commit().await?;

    avinapi::data!(SubmitAlgoExeResponse { tx_hash })
}

/// List algorithm executions with pagination
#[instrument(skip(state))]
pub async fn list_algo_exes(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationQuery>,
) -> JsonResult<PaginatedData<AlgoExeWithAlgo>> {
    let page = params.page;
    let per_page = params.per_page;

    let (items, total) = AlgoExe::list_with_algo_info(state.db.pool(), page, per_page).await?;

    // Create pagination metadata
    let meta = PaginationMeta::new(page, per_page, total as u64);

    // Return paginated response
    let response = PaginatedData::with_meta(items, meta);

    avinapi::data!(response)
}

/// Get a specific algorithm execution by ID
#[instrument(skip(state))]
pub async fn get_algo_exe(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> JsonResult<AlgoExe> {
    let algo_exe = AlgoExe::find_by_id(state.db.pool(), id as i64)
        .await?
        .ok_or_else(|| {
            avinapi::error::AppError::NotFound(format!("Algorithm execution {} not found", id))
        })?;

    avinapi::data!(algo_exe)
}

/// Build GitHub download URL from repository and commit hash
fn build_github_download_url(
    github_repo: &str,
    commit_hash: &str,
) -> Result<(String, String), avinapi::error::AppError> {
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
        return Err(avinapi::error::AppError::Validation(
            "Invalid GitHub repository URL".into(),
        ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::infra::{
        contracts::{ContractAddresses, ContractCaller, ContractConfig},
        db::Database,
        tee::{KeyVault, TappdAdapter},
    };
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        routing::{get, post},
    };
    use tower::ServiceExt;

    // Helper function to create test state
    async fn create_test_state() -> Arc<AppState> {
        // Load test configuration
        let config = Config {
            database: crate::config::DatabaseConfig {
                url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                    "postgres://postgres:password@localhost/test_delong".to_string()
                }),
                max_connections: 5,
                min_connections: 1,
                connect_timeout: 30,
                idle_timeout: 600,
            },
            server: crate::config::ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8090,
                workers: None,
            },
            chain: crate::config::ChainConfig {
                rpc_url: "http://localhost:8545".to_string(),
                chain_id: 31337,
                contract_address: "0x0000000000000000000000000000000000000000".to_string(),
                sync_interval: 60,
                sync_batch_size: 1000,
                block_batch_size: 100,
            },
            ipfs: crate::config::IpfsConfig {
                api_url: "http://localhost:5001".to_string(),
                gateway_url: "http://localhost:8080".to_string(),
                timeout: 30,
            },
            tee: crate::config::TeeConfig {
                enabled: false,
                attestation_provider: "sgx".to_string(),
                measurement_file: None,
            },
            runtime: crate::config::RuntimeConfig {
                max_execution_time: 3600,
                max_memory: 1024,
                worker_threads: 4,
                queue_size: 100,
                max_concurrent_executions: 10,
                working_directory: "/tmp/delong-runtime".to_string(),
                python_path: "python3".to_string(),
                poll_interval: 60,
            },
        };

        // Create database connection
        let db = Database::new(&config.database)
            .await
            .expect("Failed to connect to test database");

        // Run migrations
        sqlx::migrate!("./migrations")
            .run(db.pool())
            .await
            .expect("Failed to run migrations");

        // Create test IPFS client
        let ipfs_client = ipfs_api_backend_hyper::IpfsClient::default();

        // Create contract caller
        let contract_config = ContractConfig {
            http_url: config.chain.rpc_url.clone(),
            ws_url: config.chain.rpc_url.replace("http://", "ws://"),
            chain_id: config.chain.chain_id,
            addresses: ContractAddresses {
                data_contribution: config.chain.contract_address.parse().unwrap(),
                algorithm_review: config.chain.contract_address.parse().unwrap(),
            },
            funding_threshold_eth: 0.1,
            top_up_amount_eth: 1.0,
        };

        let key_vault = Arc::new(KeyVault::new(Box::new(TappdAdapter::new())));
        let contract_caller = ContractCaller::new(contract_config, key_vault, None)
            .await
            .expect("Failed to create contract caller");

        Arc::new(AppState::new(db, config, ipfs_client, contract_caller))
    }

    // Helper function to create test app
    fn create_test_app(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/algoexes", post(submit_algo_exe))
            .route("/algoexes", get(list_algo_exes))
            .route("/algoexes/:id", get(get_algo_exe))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_submit_algo_exe_invalid_wallet() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let request_body = serde_json::json!({
            "scientist_wallet": "invalid_wallet",
            "dataset": "test_dataset",
            "github_repo": "https://github.com/test/repo",
            "commit_hash": "abc123"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/algoexes")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(
            json["message"]
                .as_str()
                .unwrap()
                .contains("Invalid scientist wallet")
        );
    }

    #[tokio::test]
    async fn test_submit_algo_exe_invalid_github_url() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let request_body = serde_json::json!({
            "scientist_wallet": "0x1234567890123456789012345678901234567890",
            "dataset": "test_dataset",
            "github_repo": "not_a_github_url",
            "commit_hash": "abc123"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/algoexes")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(
            json["message"]
                .as_str()
                .unwrap()
                .contains("Invalid GitHub repository")
        );
    }

    #[tokio::test]
    async fn test_list_algo_exes_empty() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/algoexes")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(json["data"].is_array());
        assert_eq!(json["data"].as_array().unwrap().len(), 0);
        assert!(json["meta"].is_object());
    }

    #[tokio::test]
    async fn test_list_algo_exes_with_pagination() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/algoexes?page=2&per_page=5")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(json["data"].is_array());
        assert_eq!(json["meta"]["page"], 2);
        assert_eq!(json["meta"]["per_page"], 5);
    }

    #[tokio::test]
    async fn test_get_algo_exe_not_found() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/algoexes/999999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(json["message"].as_str().unwrap().contains("not found"));
    }

    #[tokio::test]
    #[ignore = "Requires IPFS daemon and blockchain running"]
    async fn test_submit_algo_exe_success() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let request_body = serde_json::json!({
            "scientist_wallet": "0x1234567890123456789012345678901234567890",
            "dataset": "test_dataset",
            "github_repo": "https://github.com/ethereum/go-ethereum",
            "commit_hash": "master"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/algoexes")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(json["tx_hash"].is_string());
    }

    #[test]
    fn test_build_github_download_url() {
        // Test valid URLs
        let test_cases = vec![
            (
                "https://github.com/owner/repo",
                "abc123",
                "https://github.com/owner/repo/archive/abc123.tar.gz",
                "repo",
            ),
            (
                "github.com/owner/repo",
                "def456",
                "https://github.com/owner/repo/archive/def456.tar.gz",
                "repo",
            ),
            (
                "https://github.com/owner/repo.git",
                "ghi789",
                "https://github.com/owner/repo/archive/ghi789.tar.gz",
                "repo",
            ),
            (
                "https://github.com/owner/repo/",
                "jkl012",
                "https://github.com/owner/repo/archive/jkl012.tar.gz",
                "repo",
            ),
        ];

        for (repo_url, commit, expected_url, expected_name) in test_cases {
            let (url, name) = build_github_download_url(repo_url, commit).unwrap();
            assert_eq!(url, expected_url);
            assert_eq!(name, expected_name);
        }

        // Test invalid URLs
        let invalid_cases = vec![
            "not-a-url",
            "https://gitlab.com/owner/repo",
            "owner/repo/extra/path",
        ];

        for invalid_url in invalid_cases {
            assert!(build_github_download_url(invalid_url, "commit").is_err());
        }
    }
}
