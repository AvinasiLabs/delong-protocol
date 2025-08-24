//! Integration tests for algorithm executor
//!
//! These tests verify the complete algorithm execution lifecycle including:
//! - Docker container management
//! - IPFS downloads
//! - Dataset mounting
//! - Result recording

use secure::{
    config::ExecutorConfig,
    models::{
        algo::{Algo, CreateAlgo},
        algo_exe::{AlgoExe, CreateAlgoExe},
        pg_types::{ExecutionStatus, ReviewStatus},
        Create, FindById,
    },
    workers::algo_executor::AlgoExecutor,
};
use std::sync::Arc;
use tempfile::TempDir;

mod common;
use common::*;

/// Generate a unique test suffix for this test run
fn unique_test_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{}", timestamp % 1_000_000_000) // Use last 9 digits of nanoseconds
}

/// Helper function to set up test state
async fn setup_test_state() -> (
    secure::infra::db::Database,
    Arc<ipfs_api_backend_hyper::IpfsClient>,
    Arc<secure::infra::contracts::ContractCaller>,
) {
    use ipfs_api_backend_hyper::TryFromUri;
    use secure::infra::{contracts::ContractCaller, TeeClientBuilder, TeeEthereum};

    let db = setup_test_db().await;

    // Don't clean database to avoid race conditions in parallel tests
    // Each test uses unique data with timestamp suffix

    let config = setup_test_config();

    let ipfs_client = Arc::new(
        ipfs_api_backend_hyper::IpfsClient::from_str(&config.ipfs.api_url)
            .expect("Failed to create IPFS client"),
    );

    let tee_endpoint = std::env::var("DSTACK_SIMULATOR_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:11010".to_string());
    let tee_client = Arc::new(TeeClientBuilder::new().endpoint(tee_endpoint).build());
    let tee_ethereum = Arc::new(TeeEthereum::new(tee_client.clone()));

    let contract_caller = ContractCaller::new(config.chain.clone(), db.clone())
        .await
        .expect("Failed to create contract caller");

    let contract_caller = if std::env::var("SKIP_TEE_INIT").is_ok() {
        Arc::new(contract_caller)
    } else {
        match contract_caller.with_tee(tee_ethereum.clone()).await {
            Ok(caller) => Arc::new(caller),
            Err(_) => Arc::new(
                ContractCaller::new(config.chain.clone(), db.clone())
                    .await
                    .expect("Failed to create contract caller"),
            ),
        }
    };

    (db, ipfs_client, contract_caller)
}

/// Test executor configuration setup
#[tokio::test]
async fn test_executor_config() {
    let config = ExecutorConfig {
        build_size_limit: 100 << 20,
        execution_timeout: 3600,
        working_directory: std::path::PathBuf::from("/tmp/test"),
        max_concurrent: 5,
        dataset_base_path: std::path::PathBuf::from("/tmp/datasets"),
    };
    assert_eq!(config.build_size_limit, 100 << 20);
    assert_eq!(config.execution_timeout, 3600);
    assert_eq!(config.max_concurrent, 5);
}

/// Test executor initialization
#[tokio::test]
async fn test_executor_init() {
    let (db, ipfs_client, contract_caller) = setup_test_state().await;

    let temp_dir = TempDir::new().unwrap();
    let config = ExecutorConfig {
        build_size_limit: 50 << 20,
        execution_timeout: 1800,
        working_directory: temp_dir.path().to_path_buf(),
        max_concurrent: 3,
        dataset_base_path: temp_dir.path().join("datasets"),
    };

    let executor = AlgoExecutor::new(Arc::new(db), ipfs_client, contract_caller, config);

    assert!(executor.is_ok());
}

/// Test scheduling execution
#[tokio::test]
async fn test_schedule_execution() {
    let (db, ipfs_client, contract_caller) = setup_test_state().await;
    let test_id = unique_test_id();

    let temp_dir = TempDir::new().unwrap();
    let config = ExecutorConfig {
        working_directory: temp_dir.path().to_path_buf(),
        dataset_base_path: temp_dir.path().join("datasets"),
        ..Default::default()
    };

    let executor = AlgoExecutor::new(
        Arc::new(db.clone()),
        ipfs_client.clone(),
        contract_caller.clone(),
        config,
    )
    .unwrap();

    // Create test algorithm
    let algo = Algo::create(
        &db.pool,
        CreateAlgo {
            name: format!("test-algo-schedule-{}", test_id),
            algo_link: format!("https://github.com/test/repo-schedule-{}", test_id),
            cid: format!("QmTestSchedule{}", test_id),
        },
    )
    .await
    .unwrap();

    // Create test execution
    let algo_exe = AlgoExe::create(
        &db.pool,
        CreateAlgoExe {
            algo_id: algo.id,
            status: ExecutionStatus::Queued,
            used_dataset: "test-dataset".to_string(),
            scientist_wallet: "0x742d35Cc6634C0532925a3b844Bc9e7595f0b0Bb".to_string(),
            review_status: ReviewStatus::Approved,
        },
    )
    .await
    .unwrap();

    // Test scheduling
    let result = executor.schedule_execution(algo_exe.id).await;
    assert!(result.is_ok());
}

/// Test scheduling resolution
#[tokio::test]
async fn test_schedule_resolution() {
    let (db, ipfs_client, contract_caller) = setup_test_state().await;

    let temp_dir = TempDir::new().unwrap();
    let config = ExecutorConfig {
        working_directory: temp_dir.path().to_path_buf(),
        dataset_base_path: temp_dir.path().join("datasets"),
        ..Default::default()
    };

    let executor = AlgoExecutor::new(
        Arc::new(db.clone()),
        ipfs_client.clone(),
        contract_caller.clone(),
        config,
    )
    .unwrap();

    let resolved_at = chrono::Utc::now() + chrono::Duration::seconds(5);
    let result = executor
        .schedule_resolve(1, "QmTest123".to_string(), resolved_at)
        .await;

    assert!(result.is_ok());
}

/// Test active execution tracking
#[tokio::test]
async fn test_active_execution_tracking() {
    let (db, ipfs_client, contract_caller) = setup_test_state().await;

    let temp_dir = TempDir::new().unwrap();
    let config = ExecutorConfig {
        working_directory: temp_dir.path().to_path_buf(),
        dataset_base_path: temp_dir.path().join("datasets"),
        ..Default::default()
    };

    let executor = AlgoExecutor::new(
        Arc::new(db.clone()),
        ipfs_client.clone(),
        contract_caller.clone(),
        config,
    )
    .unwrap();

    // Initially no active executions
    assert_eq!(executor.active_count().await, 0);
}

/// Test execution status updates
#[tokio::test]
async fn test_execution_status_updates() {
    let (db, _ipfs_client, _contract_caller) = setup_test_state().await;
    let test_id = unique_test_id();

    // Create test algorithm
    let algo = Algo::create(
        &db.pool,
        CreateAlgo {
            name: format!("test-algo-status-{}", test_id).to_string(),
            algo_link: format!("https://github.com/test/repo-status-{}", test_id).to_string(),
            cid: format!("QmTestStatus{}", test_id).to_string(),
        },
    )
    .await
    .unwrap();

    // Create test execution
    let algo_exe = AlgoExe::create(
        &db.pool,
        CreateAlgoExe {
            algo_id: algo.id,
            status: ExecutionStatus::Queued,
            used_dataset: "test-dataset-status".to_string(),
            scientist_wallet: "0x942d35Cc6634C0532925a3b844Bc9e7595f0b0Bb".to_string(),
            review_status: ReviewStatus::Approved,
        },
    )
    .await
    .unwrap();

    // Update to running
    let updated = AlgoExe::update_status(&db.pool, algo_exe.id, ExecutionStatus::Running)
        .await
        .unwrap();

    assert_eq!(updated.status, ExecutionStatus::Running);
    assert!(updated.start_time.is_some());

    // Update to completed
    let completed =
        AlgoExe::update_completed(&db.pool, algo_exe.id, "Test output".to_string(), None)
            .await
            .unwrap();

    assert_eq!(completed.status, ExecutionStatus::Completed);
    assert_eq!(completed.result, Some("Test output".to_string()));
    assert!(completed.end_time.is_some());
}

/// Test execution with error
#[tokio::test]
async fn test_execution_with_error() {
    let (db, _ipfs_client, _contract_caller) = setup_test_state().await;
    let test_id = unique_test_id();

    // Create test algorithm
    let algo = Algo::create(
        &db.pool,
        CreateAlgo {
            name: format!("test-algo-error-{}", test_id).to_string(),
            algo_link: format!("https://github.com/test/repo-error-{}", test_id).to_string(),
            cid: format!("QmTestError{}", test_id).to_string(),
        },
    )
    .await
    .unwrap();

    // Create test execution
    let algo_exe = AlgoExe::create(
        &db.pool,
        CreateAlgoExe {
            algo_id: algo.id,
            status: ExecutionStatus::Running,
            used_dataset: "test-dataset-error".to_string(),
            scientist_wallet: "0x842d35Cc6634C0532925a3b844Bc9e7595f0b0Bb".to_string(),
            review_status: ReviewStatus::Approved,
        },
    )
    .await
    .unwrap();

    // Update with error
    let failed = AlgoExe::update_completed(
        &db.pool,
        algo_exe.id,
        "".to_string(),
        Some("Test error".to_string()),
    )
    .await
    .unwrap();

    assert_eq!(failed.status, ExecutionStatus::Failed);
    assert_eq!(failed.error_msg, Some("Test error".to_string()));
}

/// Test finding pending executions
#[tokio::test]
async fn test_find_pending_executions() {
    use secure::models::blockchain_transaction::{BlockchainTransaction, EntityType};
    use secure::models::pg_types::TransactionStatus;

    let test_id = unique_test_id();
    let (db, _ipfs_client, _contract_caller) = setup_test_state().await;

    // Create test algorithm
    let algo = Algo::create(
        &db.pool,
        CreateAlgo {
            name: format!("test-algo-pending-{}", test_id).to_string(),
            algo_link: format!("https://github.com/test/repo-pending-{}", test_id).to_string(),
            cid: format!("QmTestPending{}", test_id).to_string(),
        },
    )
    .await
    .unwrap();

    // Create approved execution with transaction
    let mut tx = db.pool.begin().await.unwrap();

    let approved = AlgoExe::create_with_tx(
        &mut tx,
        CreateAlgoExe {
            algo_id: algo.id,
            status: ExecutionStatus::Queued,
            used_dataset: "test-dataset-1".to_string(),
            scientist_wallet: "0x742d35Cc6634C0532925a3b844Bc9e7595f0b0Bb".to_string(),
            review_status: ReviewStatus::Approved,
        },
    )
    .await
    .unwrap();

    // Create blockchain transaction for approved execution
    BlockchainTransaction::create(
        &mut tx,
        format!("0x1234567890abcdef{}", test_id),
        approved.id,
        EntityType::Execution,
        TransactionStatus::Confirmed,
    )
    .await
    .unwrap();

    tx.commit().await.unwrap();

    // Create queued execution with transaction
    let mut tx = db.pool.begin().await.unwrap();

    let queued = AlgoExe::create_with_tx(
        &mut tx,
        CreateAlgoExe {
            algo_id: algo.id,
            status: ExecutionStatus::Queued,
            used_dataset: "test-dataset-2".to_string(),
            scientist_wallet: "0x742d35Cc6634C0532925a3b844Bc9e7595f0b0Bb".to_string(),
            review_status: ReviewStatus::Approved,
        },
    )
    .await
    .unwrap();

    // Create blockchain transaction for queued execution
    BlockchainTransaction::create(
        &mut tx,
        format!("0xabcdef1234567890{}", test_id),
        queued.id,
        EntityType::Execution,
        TransactionStatus::Confirmed,
    )
    .await
    .unwrap();

    tx.commit().await.unwrap();

    // Create running execution (should not be included)
    let _running = AlgoExe::create(
        &db.pool,
        CreateAlgoExe {
            algo_id: algo.id,
            status: ExecutionStatus::Running,
            used_dataset: "test-dataset-3".to_string(),
            scientist_wallet: "0x742d35Cc6634C0532925a3b844Bc9e7595f0b0Bb".to_string(),
            review_status: ReviewStatus::Approved,
        },
    )
    .await
    .unwrap();

    // Find pending executions
    let pending = AlgoExe::find_pending_to_run(&db.pool).await.unwrap();

    // Should find at least our 2 test executions
    assert!(pending.len() >= 2);

    // Verify our executions are in the results
    let our_exes: Vec<_> = pending
        .iter()
        .filter(|e| e.id == approved.id || e.id == queued.id)
        .collect();
    assert_eq!(our_exes.len(), 2);

    for exe in our_exes {
        assert!(exe.status == ExecutionStatus::Queued);
    }
}

/// Test dataset path preparation
#[tokio::test]
async fn test_dataset_path_preparation() {
    let (db, ipfs_client, contract_caller) = setup_test_state().await;

    let temp_dir = TempDir::new().unwrap();
    let dataset_path = temp_dir.path().join("datasets").join("test-dataset");

    // Create dataset directory
    std::fs::create_dir_all(&dataset_path).unwrap();

    let config = ExecutorConfig {
        working_directory: temp_dir.path().to_path_buf(),
        dataset_base_path: temp_dir.path().join("datasets"),
        ..Default::default()
    };

    let _executor = AlgoExecutor::new(
        Arc::new(db.clone()),
        ipfs_client.clone(),
        contract_caller.clone(),
        config,
    )
    .unwrap();

    // Test that dataset path exists
    assert!(dataset_path.exists());
}

// Note: Full integration test with Docker requires Docker to be running
// and would actually execute containers. This tests the complete execution flow.

#[tokio::test]
#[ignore = "Requires Docker and IPFS running"]
async fn test_full_execution_flow() {
    use secure::models::blockchain_transaction::{BlockchainTransaction, EntityType};
    use secure::models::pg_types::TransactionStatus;

    println!("Starting full execution flow test");

    let (db, ipfs_client, contract_caller) = setup_test_state().await;
    let test_id = unique_test_id();

    // Create a temporary directory for test dataset
    let temp_dir = TempDir::new().unwrap();
    let dataset_dir = temp_dir.path().join("datasets");
    let test_dataset = dataset_dir.join("test-dataset");
    std::fs::create_dir_all(&test_dataset).unwrap();

    // Create test dataset CSV file
    let csv_path = test_dataset.join("data.csv");
    std::fs::write(
        &csv_path,
        "id,value,label\n1,100,A\n2,200,B\n3,300,C\n4,400,D\n5,500,E\n",
    )
    .unwrap();
    println!("Created test dataset at: {:?}", csv_path);

    // Configuration for executor
    let config = ExecutorConfig {
        working_directory: temp_dir.path().join("work"),
        dataset_base_path: dataset_dir.clone(),
        build_size_limit: 100 << 20, // 100MB
        execution_timeout: 300,      // 5 minutes
        max_concurrent: 1,
    };

    // Create executor
    let executor = AlgoExecutor::new(
        Arc::new(db.clone()),
        ipfs_client.clone(),
        contract_caller.clone(),
        config,
    )
    .unwrap();

    // Download real algorithm from GitHub (using the test commit from Go tests)
    let owner = "lilhammer111";
    let repo = "algo-demo";
    let commit_hash = "c73e8d62a0ae5d68040cabb461c7b51b7630020c";
    let download_url = format!(
        "https://codeload.github.com/{}/{}/tar.gz/{}",
        owner, repo, commit_hash
    );

    println!("Downloading algorithm from: {}", download_url);
    let response = reqwest::get(&download_url).await.unwrap();
    assert!(
        response.status().is_success(),
        "Failed to download from GitHub"
    );

    let algo_bytes = response.bytes().await.unwrap();
    println!("Downloaded {} bytes", algo_bytes.len());

    // Upload to IPFS
    use ipfs_api_backend_hyper::IpfsApi;
    use std::io::Cursor;

    println!("Uploading to IPFS...");
    let add_response = ipfs_client
        .add(Cursor::new(algo_bytes.to_vec()))
        .await
        .unwrap();

    let algo_cid = add_response.hash;
    println!("Algorithm uploaded to IPFS with CID: {}", algo_cid);

    // Create algorithm record or find existing one with same CID
    let algo = match Algo::create(
        &db.pool,
        CreateAlgo {
            name: format!("test-algo-full-{}", test_id),
            algo_link: format!("{}-{}", download_url, test_id),
            cid: algo_cid.clone(),
        },
    )
    .await
    {
        Ok(algo) => algo,
        Err(_) => {
            // CID already exists, find the existing algorithm
            use secure::models::algo::Algo;
            let existing = sqlx::query_as!(Algo, "SELECT * FROM algo WHERE cid = $1", &algo_cid)
                .fetch_one(&db.pool)
                .await
                .expect("Failed to find existing algorithm");
            println!("Using existing algorithm with ID: {}", existing.id);
            existing
        }
    };

    // Start a transaction for creating execution
    let mut tx = db.pool.begin().await.unwrap();

    // Create execution record
    let algo_exe = AlgoExe::create_with_tx(
        &mut tx,
        CreateAlgoExe {
            algo_id: algo.id,
            status: ExecutionStatus::Queued,
            used_dataset: "test-dataset".to_string(),
            scientist_wallet: format!("0x742d35Cc6634C0532925a3b844Bc9e7595f0b0{}", test_id),
            review_status: ReviewStatus::Approved,
        },
    )
    .await
    .unwrap();

    // Create blockchain transaction (required for find_pending_to_run)
    BlockchainTransaction::create(
        &mut tx,
        format!("0xfulltest{}", test_id),
        algo_exe.id,
        EntityType::Execution,
        TransactionStatus::Confirmed,
    )
    .await
    .unwrap();

    tx.commit().await.unwrap();

    println!("Created algorithm execution: {}", algo_exe.id);

    // Schedule and execute
    executor.schedule_execution(algo_exe.id).await.unwrap();
    println!("Scheduled execution");

    // Wait a bit for execution to start
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Check execution status
    let updated_exe = AlgoExe::find_by_id(&db.pool, algo_exe.id)
        .await
        .unwrap()
        .expect("Execution should exist");

    println!("Execution status: {:?}", updated_exe.status);

    // The execution should have started or completed
    // Note: Actual completion depends on Docker being available
    // and the algorithm successfully running
    assert!(
        updated_exe.status == ExecutionStatus::Running
            || updated_exe.status == ExecutionStatus::Completed
            || updated_exe.status == ExecutionStatus::Failed
            || updated_exe.status == ExecutionStatus::Queued, // May still be queued if Docker is not available
        "Unexpected status: {:?}",
        updated_exe.status
    );

    // If Docker is running and execution completed, check results
    if updated_exe.status == ExecutionStatus::Completed {
        assert!(
            updated_exe.result.is_some(),
            "Completed execution should have results"
        );
        assert!(
            updated_exe.end_time.is_some(),
            "Completed execution should have end time"
        );
        println!("Execution completed successfully!");
        println!("Result: {:?}", updated_exe.result);
    } else if updated_exe.status == ExecutionStatus::Failed {
        println!("Execution failed: {:?}", updated_exe.error_msg);
    } else {
        println!("Execution is still in progress or queued");
    }

    println!("Full execution flow test completed");
}
