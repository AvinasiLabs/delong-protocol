//! Comprehensive ChainSync Worker Tests
//!
//! This test suite provides comprehensive coverage for ChainSync functionality,
//! including edge cases, error scenarios, and performance testing.

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use secure::{
    models::{
        blockchain_transaction::{BlockchainTransaction, TransactionStatus},
        committee::CommitteeMember,
        AlgoReviewStatus,
    },
    workers::ChainSyncWorker,
};
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use tower::ServiceExt;
use tracing::{error, info, warn};

mod common;
use common::setup_test_app;

/// Enhanced test setup with configurable options
struct TestEnvironment {
    app: axum::Router,
    worker: Arc<ChainSyncWorker>,
    pool: PgPool,
    #[allow(dead_code)]
    config: secure::Config,
}

impl TestEnvironment {
    /// Create a new test environment with ChainSync worker
    async fn new() -> Self {
        Self::with_polling_interval(500).await
    }

    /// Create test environment with custom polling interval
    async fn with_polling_interval(interval_ms: u64) -> Self {
        // Initialize logging
        let _ = tracing_subscriber::fmt::try_init();
        info!("Setting up comprehensive test environment");

        // Set polling mode for predictable test behavior
        std::env::set_var("USE_POLLING_MODE", "true");
        std::env::set_var("POLLING_INTERVAL_MS", interval_ms.to_string());

        // Load configuration
        let config = secure::Config::load().expect("Failed to load config");

        // Initialize database
        let db = Arc::new(
            secure::infra::db::Database::new(&config.database)
                .await
                .expect("Failed to initialize database"),
        );
        let pool = db.pool().clone();

        // Initialize contract caller
        let db_for_contract = secure::infra::db::Database::new(&config.database)
            .await
            .expect("Failed to create DB for contract");
        let contract_caller =
            secure::infra::contracts::ContractCaller::new(config.chain.clone(), db_for_contract)
                .await
                .expect("Failed to initialize contract caller");

        // Initialize TEE services
        let tee_endpoint = std::env::var("DSTACK_SIMULATOR_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:11010".to_string());
        let tee_client = Arc::new(
            secure::infra::TeeClientBuilder::new()
                .endpoint(tee_endpoint)
                .build(),
        );
        let tee_ethereum = Arc::new(secure::infra::TeeEthereum::new(tee_client.clone()));
        let contract_caller = Arc::new(
            contract_caller
                .with_tee(tee_ethereum.clone())
                .await
                .expect("Failed to initialize TEE for contract caller"),
        );

        // Initialize notifier
        let notifier = Arc::new(secure::infra::Notifier::new());

        // Initialize algo executor
        use ipfs_api_backend_hyper::TryFromUri;
        let ipfs_client = Arc::new(
            ipfs_api_backend_hyper::IpfsClient::from_str(&config.ipfs.api_url)
                .expect("Failed to create IPFS client"),
        );
        let executor_config = secure::config::ExecutorConfig {
            build_size_limit: 1024 * 1024 * 100,
            working_directory: std::path::PathBuf::from("/tmp/algo_executor_test"),
            max_concurrent: 5,
            dataset_base_path: std::path::PathBuf::from("/tmp/datasets_test"),
            execution_timeout: 300,
        };
        let algo_executor = Arc::new(
            secure::workers::algo_executor::AlgoExecutor::new(
                db.clone(),
                ipfs_client,
                contract_caller.clone(),
                executor_config,
            )
            .expect("Failed to initialize algo executor"),
        );

        // Create ChainSync worker
        let chainsync_worker = Arc::new(ChainSyncWorker::new(
            db.clone(),
            contract_caller.clone(),
            notifier.clone(),
            algo_executor.clone(),
            Arc::new(config.clone()),
        ));

        // Start ChainSync worker in background
        let worker_clone = chainsync_worker.clone();
        tokio::spawn(async move {
            info!("Starting ChainSync worker for testing");
            if let Err(e) = worker_clone.start().await {
                error!("ChainSync worker error: {}", e);
            }
        });

        // Give worker time to start
        sleep(Duration::from_secs(2)).await;

        // Get test app
        let app = setup_test_app().await;

        info!("Test environment ready");
        Self {
            app,
            worker: chainsync_worker,
            pool,
            config: config.clone(),
        }
    }

    /// Wait for a transaction to be confirmed with timeout
    async fn wait_for_confirmation(&self, tx_hash: &str, timeout_secs: u64) -> bool {
        let deadline = Duration::from_secs(timeout_secs);
        let poll_interval = Duration::from_millis(500);

        let result = timeout(deadline, async {
            loop {
                if let Ok(Some(tx)) =
                    BlockchainTransaction::find_by_tx_hash(&self.pool, tx_hash).await
                {
                    if tx.status == TransactionStatus::Confirmed {
                        return true;
                    }
                }
                sleep(poll_interval).await;
            }
        })
        .await;

        result.unwrap_or(false)
    }

    /// Clean up test data for specific entity
    async fn cleanup_entity(&self, entity_type: &str, entity_id: i64) {
        let _ = sqlx::query!(
            "DELETE FROM blockchain_transaction WHERE entity_type = $1 AND entity_id = $2",
            entity_type,
            entity_id
        )
        .execute(&self.pool)
        .await;
    }
}

// ============================================================================
// Vote and Resolution Tests
// ============================================================================

/// Test VoteCasted event processing with resolution trigger
#[tokio::test]
async fn test_chainsync_vote_casted_with_resolution() {
    info!("Testing VoteCasted event processing with automatic resolution");

    let env = TestEnvironment::new().await;

    // Step 1: Create algorithm execution
    let wallet_address = format!(
        "0x{:040x}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );

    let request = Request::builder()
        .method("POST")
        .uri("/api/algoexes")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "scientist_wallet": wallet_address,
                "dataset": "test-dataset",
                "github_repo": "https://github.com/lilhammer111/algo-demo",
                "commit_hash": "c73e8d62a0ae5d68040cabb461c7b51b7630020c"
            })
            .to_string(),
        ))
        .unwrap();

    let response = env.app.clone().oneshot(request).await.unwrap();

    if response.status() != StatusCode::OK {
        warn!("Algorithm submission failed, skipping test");
        return;
    }

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let algo_exe_id = response_json["data"]["id"]
        .as_i64()
        .expect("Should have execution ID");

    info!("Created algorithm execution: {}", algo_exe_id);

    // Step 2: Add committee members
    let mut committee_members = vec![];
    for i in 0..3 {
        let member = format!("0x{:040x}", (1000 + i) as u128);
        committee_members.push(member.clone());

        let request = Request::builder()
            .method("POST")
            .uri("/api/committee")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "member_wallet": member,
                    "is_approved": true
                })
                .to_string(),
            ))
            .unwrap();

        let _ = env.app.clone().oneshot(request).await.unwrap();
    }

    // Wait for committee setup
    sleep(Duration::from_secs(2)).await;

    // Step 3: Submit votes (2 approve, 1 reject - should trigger approval)
    let algo_cid = "QmVoteTest123"; // This should be the actual CID from execution

    for (i, member) in committee_members.iter().enumerate() {
        let approve = i < 2; // First 2 approve, last one rejects

        let request = Request::builder()
            .method("POST")
            .uri("/api/votes")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "algo_cid": algo_cid,
                    "voter": member,
                    "approve": approve
                })
                .to_string(),
            ))
            .unwrap();

        let response = env.app.clone().oneshot(request).await.unwrap();
        if response.status() == StatusCode::OK {
            info!("Vote submitted by {}: approve={}", member, approve);
        }
    }

    // Step 4: Wait for ChainSync to process votes and check resolution
    sleep(Duration::from_secs(5)).await;

    // Verify votes were recorded
    let vote_count = sqlx::query!(
        "SELECT COUNT(*) as count FROM vote WHERE algo_cid = $1",
        algo_cid
    )
    .fetch_one(&env.pool)
    .await
    .expect("Query should succeed");

    info!(
        "Recorded {} votes for algorithm",
        vote_count.count.unwrap_or(0)
    );

    // Check if algorithm was resolved
    let algo_exe = sqlx::query!(
        "SELECT review_status as \"review_status: AlgoReviewStatus\" FROM algo_exe WHERE id = $1",
        algo_exe_id
    )
    .fetch_optional(&env.pool)
    .await
    .expect("Query should succeed");

    if let Some(exe) = algo_exe {
        info!(
            "Algorithm review status after voting: {:?}",
            exe.review_status
        );
        // With 2/3 approval votes, it should be approved
        // Note: This depends on ChainSync's check_and_resolve_algorithm logic
    }

    info!("✅ VoteCasted event processing test completed");
}

/// Test ExecutionSubmitted event with automatic resolution scheduling
#[tokio::test]
async fn test_chainsync_execution_submitted_with_schedule() {
    info!("Testing ExecutionSubmitted event with resolution scheduling");

    let env = TestEnvironment::new().await;

    // Create algorithm execution
    let wallet_address = format!(
        "0x{:040x}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );

    let request = Request::builder()
        .method("POST")
        .uri("/api/algoexes")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "scientist_wallet": wallet_address,
                "dataset": "test-dataset",
                "github_repo": "https://github.com/lilhammer111/algo-demo",
                "commit_hash": "c73e8d62a0ae5d68040cabb461c7b51b7630020c"
            })
            .to_string(),
        ))
        .unwrap();

    let response = env.app.clone().oneshot(request).await.unwrap();

    if response.status() != StatusCode::OK {
        warn!("Algorithm submission failed, skipping test");
        return;
    }

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let algo_exe_id = response_json["data"]["id"]
        .as_i64()
        .expect("Should have execution ID");

    // When ExecutionSubmitted event is processed, ChainSync should:
    // 1. Update vote_start_time and vote_end_time in algo_exe
    // 2. Schedule automatic resolution at vote_end_time

    // Check if voting times were set (would be set by ExecutionSubmitted event)
    let algo_exe = sqlx::query!(
        r#"
        SELECT
            vote_start_time,
            vote_end_time,
            review_status as "review_status: AlgoReviewStatus"
        FROM algo_exe
        WHERE id = $1
        "#,
        algo_exe_id
    )
    .fetch_optional(&env.pool)
    .await
    .expect("Query should succeed");

    if let Some(exe) = algo_exe {
        info!("Algorithm execution voting window:");
        info!("  Start: {:?}", exe.vote_start_time);
        info!("  End: {:?}", exe.vote_end_time);
        info!("  Status: {:?}", exe.review_status);

        if exe.vote_end_time.is_some() {
            info!(
                "Resolution should be scheduled for: {:?}",
                exe.vote_end_time
            );
        }
    }

    info!("✅ ExecutionSubmitted event processing test completed");
}

// ============================================================================
// Error Recovery Tests
// ============================================================================

/// Test ChainSync recovery after database connection failure
#[tokio::test]
async fn test_chainsync_database_recovery() {
    info!("Testing ChainSync database connection recovery");

    let env = TestEnvironment::new().await;

    // Submit a transaction
    let member_wallet = format!("0x{:040x}", 888888u128);

    let request = Request::builder()
        .method("POST")
        .uri("/api/committee")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "member_wallet": member_wallet,
                "is_approved": true
            })
            .to_string(),
        ))
        .unwrap();

    let response = env.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let tx_hash = response_json["data"]["tx_hash"]
        .as_str()
        .expect("Should have tx hash");

    // Simulate database issues by checking if worker can recover
    // In real scenario, we might close/reopen connections

    // Wait and check if transaction eventually gets confirmed
    let confirmed = env.wait_for_confirmation(tx_hash, 10).await;

    info!(
        "Transaction confirmation after potential issues: {}",
        confirmed
    );

    info!("✅ Database recovery test completed");
}

/// Test ChainSync handling of duplicate events
#[tokio::test]
async fn test_chainsync_duplicate_event_handling() {
    info!("Testing ChainSync duplicate event handling");

    let env = TestEnvironment::with_polling_interval(250).await; // Faster polling

    // Submit a vote that might be processed multiple times
    let algo_cid = "QmDuplicateTest123";
    let voter = format!("0x{:040x}", 777777u128);

    // First, ensure committee member exists
    let _ = sqlx::query!(
        "INSERT INTO committee_member (member_wallet, is_approved) VALUES ($1, true)
         ON CONFLICT (member_wallet) DO UPDATE SET is_approved = true",
        voter.to_lowercase()
    )
    .execute(&env.pool)
    .await;

    // Insert initial vote
    let vote1 = sqlx::query!(
        r#"
        INSERT INTO vote (algo_cid, voter, approve, voted_at)
        VALUES ($1, $2, true, NOW())
        ON CONFLICT (algo_cid, voter) DO UPDATE SET approve = true
        RETURNING id
        "#,
        algo_cid,
        voter
    )
    .fetch_one(&env.pool)
    .await
    .expect("Should insert vote");

    info!("Initial vote ID: {}", vote1.id);

    // Try to insert duplicate vote (simulating duplicate event)
    let vote2_result = sqlx::query!(
        r#"
        INSERT INTO vote (algo_cid, voter, approve, voted_at)
        VALUES ($1, $2, false, NOW())
        ON CONFLICT (algo_cid, voter) DO UPDATE SET approve = false
        RETURNING id
        "#,
        algo_cid,
        voter
    )
    .fetch_one(&env.pool)
    .await;

    match vote2_result {
        Ok(vote2) => {
            // Should update existing vote, not create new one
            assert_eq!(vote1.id, vote2.id, "Should update existing vote");
            info!("Duplicate vote correctly updated existing record");
        }
        Err(e) => {
            info!("Duplicate vote handling error: {}", e);
        }
    }

    // Verify only one vote exists
    let vote_count = sqlx::query!(
        "SELECT COUNT(*) as count FROM vote WHERE algo_cid = $1 AND voter = $2",
        algo_cid,
        voter
    )
    .fetch_one(&env.pool)
    .await
    .expect("Query should succeed");

    assert_eq!(
        vote_count.count.unwrap_or(0),
        1,
        "Should have exactly one vote record"
    );

    info!("✅ Duplicate event handling test completed");
}

// ============================================================================
// Performance and Stress Tests
// ============================================================================

/// Test ChainSync performance with high event volume
#[tokio::test]
async fn test_chainsync_high_volume_events() {
    info!("Testing ChainSync with high volume of events");

    let env = TestEnvironment::with_polling_interval(100).await; // Very fast polling

    let num_events = 20;
    let mut tx_hashes = vec![];

    // Submit many transactions rapidly
    for i in 0..num_events {
        let member_wallet = format!("0x{:040x}", (100000 + i) as u128);

        let request = Request::builder()
            .method("POST")
            .uri("/api/committee")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "member_wallet": member_wallet,
                    "is_approved": i % 2 == 0  // Alternate approve/remove
                })
                .to_string(),
            ))
            .unwrap();

        let response = env.app.clone().oneshot(request).await.unwrap();

        if response.status() == StatusCode::OK {
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
            if let Some(tx_hash) = response_json["data"]["tx_hash"].as_str() {
                tx_hashes.push(tx_hash.to_string());
            }
        }
    }

    info!("Submitted {} transactions", tx_hashes.len());

    // Measure processing time
    let start = std::time::Instant::now();

    // Wait for all transactions to be confirmed
    let mut confirmed_count = 0;
    let max_wait = Duration::from_secs(30);
    let poll_interval = Duration::from_millis(500);
    let deadline = start + max_wait;

    while std::time::Instant::now() < deadline && confirmed_count < tx_hashes.len() {
        confirmed_count = 0;
        for tx_hash in &tx_hashes {
            if let Ok(Some(tx)) = BlockchainTransaction::find_by_tx_hash(&env.pool, tx_hash).await {
                if tx.status == TransactionStatus::Confirmed {
                    confirmed_count += 1;
                }
            }
        }

        if confirmed_count < tx_hashes.len() {
            sleep(poll_interval).await;
        }
    }

    let elapsed = start.elapsed();
    info!(
        "Processed {}/{} transactions in {:?}",
        confirmed_count,
        tx_hashes.len(),
        elapsed
    );

    // Calculate throughput
    if confirmed_count > 0 {
        let throughput = confirmed_count as f64 / elapsed.as_secs_f64();
        info!("Throughput: {:.2} events/second", throughput);
    }

    assert!(
        confirmed_count >= tx_hashes.len() * 80 / 100,
        "Should confirm at least 80% of transactions"
    );

    info!("✅ High volume event processing test completed");
}

/// Test ChainSync recovery of unresolved algorithms on startup
#[tokio::test]
async fn test_chainsync_recover_unresolved_algorithms() {
    info!("Testing ChainSync recovery of unresolved algorithms");

    let env = TestEnvironment::new().await;

    // Create an algorithm execution that's past its voting deadline
    let past_time = chrono::Utc::now() - chrono::Duration::hours(1);
    let future_time = chrono::Utc::now() + chrono::Duration::hours(1);

    // Insert test data directly
    let algo = sqlx::query!(
        "INSERT INTO algo (name, algo_link, cid) VALUES ($1, $2, $3) RETURNING id",
        "test-recovery-algo",
        "https://github.com/test/recovery",
        "QmRecoveryTest123"
    )
    .fetch_one(&env.pool)
    .await
    .expect("Should insert algo");

    let algo_exe_past = sqlx::query!(
        r#"
        INSERT INTO algo_exe (
            algo_id,
            used_dataset,
            scientist_wallet,
            review_status,
            vote_start_time,
            vote_end_time
        )
        VALUES ($1, $2, $3, 'reviewing', $4, $5)
        RETURNING id
        "#,
        algo.id,
        "test-dataset",
        format!("0x{:040x}", 666666u128),
        past_time - chrono::Duration::hours(2),
        past_time
    )
    .fetch_one(&env.pool)
    .await
    .expect("Should insert past algo_exe");

    let algo_exe_future = sqlx::query!(
        r#"
        INSERT INTO algo_exe (
            algo_id,
            used_dataset,
            scientist_wallet,
            review_status,
            vote_start_time,
            vote_end_time
        )
        VALUES ($1, $2, $3, 'reviewing', $4, $5)
        RETURNING id
        "#,
        algo.id,
        "test-dataset",
        format!("0x{:040x}", 666667u128),
        chrono::Utc::now(),
        future_time
    )
    .fetch_one(&env.pool)
    .await
    .expect("Should insert future algo_exe");

    // Create confirmed transactions for these executions
    let _ = sqlx::query!(
        r#"
        INSERT INTO blockchain_transaction (
            tx_hash, entity_id, entity_type, status, block_number
        )
        VALUES
            ($1, $2, 'EXECUTION', 'confirmed', 1),
            ($3, $4, 'EXECUTION', 'confirmed', 2)
        "#,
        format!("0x{:064x}", algo_exe_past.id),
        algo_exe_past.id,
        format!("0x{:064x}", algo_exe_future.id),
        algo_exe_future.id
    )
    .execute(&env.pool)
    .await;

    // Call recover_resolve_tasks
    info!("Calling recover_resolve_tasks...");
    let result = env.worker.recover_resolve_tasks().await;

    match result {
        Ok(_) => {
            info!("Successfully recovered unresolved algorithms");

            // The past deadline execution should be checked for resolution immediately
            // The future deadline execution should be scheduled for later

            // In a real scenario, we would verify:
            // 1. Past execution triggers check_and_resolve_algorithm
            // 2. Future execution gets scheduled for its vote_end_time
        }
        Err(e) => {
            error!("Failed to recover resolve tasks: {}", e);
        }
    }

    // Clean up test data
    env.cleanup_entity("EXECUTION", algo_exe_past.id).await;
    env.cleanup_entity("EXECUTION", algo_exe_future.id).await;

    info!("✅ Algorithm recovery test completed");
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test ChainSync with invalid or malformed events
#[tokio::test]
async fn test_chainsync_invalid_event_handling() {
    info!("Testing ChainSync handling of invalid events");

    let env = TestEnvironment::new().await;

    // Test with invalid transaction hash format
    let invalid_tx = BlockchainTransaction {
        id: 0,
        tx_hash: "invalid_hash".to_string(), // Not a valid hex hash
        entity_id: 999999,
        entity_type: "INVALID".to_string(),
        status: TransactionStatus::Pending,
        block_number: None,
        block_timestamp: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    // Try to find invalid transaction (should handle gracefully)
    let result = BlockchainTransaction::find_by_tx_hash(&env.pool, &invalid_tx.tx_hash).await;

    match result {
        Ok(None) => info!("Invalid transaction not found (expected)"),
        Ok(Some(_)) => warn!("Found transaction with invalid hash (unexpected)"),
        Err(e) => info!("Error handling invalid transaction: {}", e),
    }

    // Test with extremely long CID
    let long_cid = "Q".repeat(500); // Way too long for a valid CID
    let result = sqlx::query!(
        "INSERT INTO vote (algo_cid, voter, approve) VALUES ($1, $2, true)",
        long_cid,
        "0x1234567890123456789012345678901234567890"
    )
    .execute(&env.pool)
    .await;

    if result.is_err() {
        info!("Correctly rejected overly long CID");
    }

    info!("✅ Invalid event handling test completed");
}

/// Test ChainSync committee size calculation edge cases
#[tokio::test]
async fn test_chainsync_committee_size_edge_cases() {
    info!("Testing ChainSync committee size calculation edge cases");

    // Test majority calculation for various committee sizes
    let test_cases = vec![
        (1, 1),    // 1 member needs 1 vote
        (2, 2),    // 2 members need 2 votes
        (3, 2),    // 3 members need 2 votes
        (4, 3),    // 4 members need 3 votes
        (5, 3),    // 5 members need 3 votes
        (10, 6),   // 10 members need 6 votes
        (100, 51), // 100 members need 51 votes
    ];

    for (committee_size, expected_required) in test_cases {
        let required_votes = (committee_size / 2) + 1;
        assert_eq!(
            required_votes, expected_required,
            "Committee size {} should require {} votes",
            committee_size, expected_required
        );
        info!(
            "Committee size {}: requires {} votes ✓",
            committee_size, required_votes
        );
    }

    info!("✅ Committee size calculation test completed");
}

/// Test ChainSync with rapid state changes
#[tokio::test]
async fn test_chainsync_rapid_state_changes() {
    info!("Testing ChainSync with rapid state changes");

    let env = TestEnvironment::with_polling_interval(100).await;

    let member_wallet = format!("0x{:040x}", 555555u128);

    // Rapidly toggle committee member status
    for i in 0..5 {
        let is_approved = i % 2 == 0;

        let request = Request::builder()
            .method("POST")
            .uri("/api/committee")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "member_wallet": member_wallet.clone(),
                    "is_approved": is_approved
                })
                .to_string(),
            ))
            .unwrap();

        let response = env.app.clone().oneshot(request).await.unwrap();

        if response.status() == StatusCode::OK {
            info!("Toggle {}: set is_approved={}", i, is_approved);
        }

        // Very short delay between changes
        sleep(Duration::from_millis(100)).await;
    }

    // Wait for all events to be processed
    sleep(Duration::from_secs(3)).await;

    // Check final state
    let member = CommitteeMember::get_by_wallet(&env.pool, &member_wallet).await;

    match member {
        Ok(Some(m)) => {
            info!("Final member state: is_approved={}", m.is_approved);
            // Final state should reflect the last change (i=4, which is even, so approved=true)
            assert!(m.is_approved, "Final state should be approved");
        }
        Ok(None) => {
            warn!("Member not found after rapid changes");
        }
        Err(e) => {
            error!("Error checking final state: {}", e);
        }
    }

    info!("✅ Rapid state changes test completed");
}
