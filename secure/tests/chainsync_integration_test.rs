//! Integration tests for ChainSyncWorker
//!
//! These tests verify the chain synchronization functionality with a real database
//! but mock external dependencies like blockchain interactions.

use secure::{config::Config, infra::db::Database, models::AlgoReviewStatus};
use sqlx::PgPool;
use std::sync::Arc;

/// Helper to setup test database with proper cleanup
async fn setup_test_environment() -> (Arc<Database>, PgPool) {
    // Initialize test environment
    if let Err(e) = dotenvy::from_filename(".env") {
        eprintln!("Failed to load .env: {}. Using environment variables.", e);
    }

    // Initialize configuration
    let config = Config::from_env().expect("Failed to load config");

    // Initialize database
    let db = Database::new(&config.database)
        .await
        .expect("Failed to connect to database");

    // Run migrations (ignore errors if schema already exists)
    if let Err(e) = db.run_migrations().await {
        eprintln!("Migration warning: {}. Continuing with existing schema.", e);
    }

    let pool = db.pool().clone();
    (Arc::new(db), pool)
}

/// Clean up test data after each test
async fn cleanup_test_data(pool: &PgPool) {
    // Clean up in reverse order of foreign key dependencies
    let _ = sqlx::query!("DELETE FROM vote").execute(pool).await;
    let _ = sqlx::query!("DELETE FROM data_usage").execute(pool).await;
    let _ = sqlx::query!("DELETE FROM committee_member")
        .execute(pool)
        .await;
    let _ = sqlx::query!("DELETE FROM algo_exe").execute(pool).await;
    let _ = sqlx::query!("DELETE FROM algo").execute(pool).await;
}

#[tokio::test]
async fn test_data_usage_tracking() {
    let (_db, pool) = setup_test_environment().await;

    // Insert test data usage record
    let scientist_wallet = "0x1234567890123456789012345678901234567890";
    let cid = "QmTestCID123";
    let dataset = "test_dataset";

    let result = sqlx::query!(
        r#"
        INSERT INTO data_usage (scientist_wallet, cid, dataset, used_at)
        VALUES ($1, $2, $3, NOW())
        RETURNING id
        "#,
        scientist_wallet,
        cid,
        dataset
    )
    .fetch_one(&pool)
    .await;

    assert!(result.is_ok());
    let record = result.unwrap();
    assert!(record.id > 0);

    // Verify the record was inserted
    let usage = sqlx::query!(
        "SELECT * FROM data_usage WHERE scientist_wallet = $1 AND cid = $2",
        scientist_wallet,
        cid
    )
    .fetch_optional(&pool)
    .await
    .unwrap();

    assert!(usage.is_some());
    let usage_record = usage.unwrap();
    assert_eq!(usage_record.dataset, dataset);
    assert_eq!(usage_record.cid, cid);

    cleanup_test_data(&pool).await;
}

#[tokio::test]
async fn test_committee_member_management() {
    let (_db, pool) = setup_test_environment().await;

    let member_wallet = "0x2345678901234567890123456789012345678901";

    // Add committee member
    let result = sqlx::query!(
        r#"
        INSERT INTO committee_member (member_wallet, is_approved)
        VALUES ($1, $2)
        ON CONFLICT (member_wallet)
        DO UPDATE SET is_approved = $2
        RETURNING id
        "#,
        member_wallet,
        true
    )
    .fetch_one(&pool)
    .await;

    assert!(result.is_ok());

    // Verify member was added
    let member = sqlx::query!(
        "SELECT * FROM committee_member WHERE member_wallet = $1",
        member_wallet
    )
    .fetch_optional(&pool)
    .await
    .unwrap();

    assert!(member.is_some());
    assert!(member.unwrap().is_approved);

    // Update member status
    let update_result = sqlx::query!(
        r#"
        UPDATE committee_member
        SET is_approved = false
        WHERE member_wallet = $1
        "#,
        member_wallet
    )
    .execute(&pool)
    .await;

    assert!(update_result.is_ok());

    // Verify member status was updated
    let updated_member = sqlx::query!(
        "SELECT * FROM committee_member WHERE member_wallet = $1",
        member_wallet
    )
    .fetch_optional(&pool)
    .await
    .unwrap();

    assert!(updated_member.is_some());
    assert!(!updated_member.unwrap().is_approved);

    cleanup_test_data(&pool).await;
}

#[tokio::test]
async fn test_vote_tracking_and_resolution() {
    let (_db, pool) = setup_test_environment().await;

    let cid = "QmTestAlgo123";
    let algo_name = "Test Algorithm";
    let algo_link = "http://example.com/test-algo";

    // Create algorithm
    let algo_result = sqlx::query!(
        r#"
        INSERT INTO algo (cid, name, algo_link)
        VALUES ($1, $2, $3)
        RETURNING id
        "#,
        cid,
        algo_name,
        algo_link
    )
    .fetch_one(&pool)
    .await;

    assert!(algo_result.is_ok());
    let algo_id = algo_result.unwrap().id;

    // Create algorithm execution
    let exe_result = sqlx::query!(
        r#"
        INSERT INTO algo_exe (algo_id, review_status)
        VALUES ($1, $2)
        RETURNING id
        "#,
        algo_id,
        AlgoReviewStatus::Reviewing as AlgoReviewStatus
    )
    .fetch_one(&pool)
    .await;

    assert!(exe_result.is_ok());

    // Add committee members
    let members = vec![
        "0x3456789012345678901234567890123456789012",
        "0x4567890123456789012345678901234567890123",
        "0x5678901234567890123456789012345678901234",
        "0x6789012345678901234567890123456789012345",
        "0x7890123456789012345678901234567890123456",
    ];

    for member in &members {
        sqlx::query!(
            r#"
            INSERT INTO committee_member (member_wallet, is_approved)
            VALUES ($1, $2)
            "#,
            member,
            true
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    // Cast votes (3 approve, 2 reject)
    for (i, member) in members.iter().enumerate() {
        let approve = i < 3; // First 3 approve, last 2 reject

        sqlx::query!(
            r#"
            INSERT INTO vote (algo_cid, voter, approve, voted_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (algo_cid, voter)
            DO UPDATE SET approve = $3, voted_at = NOW()
            "#,
            cid,
            member,
            approve
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    // Verify vote counts
    let vote_stats = sqlx::query!(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE approve = true) as approve_count,
            COUNT(*) FILTER (WHERE approve = false) as reject_count,
            COUNT(*) as total_count
        FROM vote
        WHERE algo_cid = $1
        "#,
        cid
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(vote_stats.approve_count.unwrap_or(0), 3);
    assert_eq!(vote_stats.reject_count.unwrap_or(0), 2);
    assert_eq!(vote_stats.total_count.unwrap_or(0), 5);

    // Check if we have majority (3/5 is majority)
    let committee_size =
        sqlx::query!("SELECT COUNT(*) as count FROM committee_member WHERE is_approved = true")
            .fetch_one(&pool)
            .await
            .unwrap()
            .count
            .unwrap_or(0);

    assert_eq!(committee_size, 5);

    let required_votes = (committee_size / 2) + 1;
    assert_eq!(required_votes, 3);

    // Verify approval has majority
    assert!(vote_stats.approve_count.unwrap_or(0) >= required_votes);

    // Simulate algorithm resolution
    let update_result = sqlx::query!(
        r#"
        UPDATE algo_exe
        SET review_status = $2, updated_at = NOW()
        WHERE algo_id = $1
        "#,
        algo_id,
        AlgoReviewStatus::Approved as AlgoReviewStatus
    )
    .execute(&pool)
    .await;

    assert!(update_result.is_ok());

    // Clean up votes
    let cleanup_result = sqlx::query!(
        r#"
        DELETE FROM vote WHERE algo_cid = $1
        "#,
        cid
    )
    .execute(&pool)
    .await;

    assert!(cleanup_result.is_ok());

    // Verify votes were cleaned up
    let remaining_votes = sqlx::query!(
        "SELECT COUNT(*) as count FROM vote WHERE algo_cid = $1",
        cid
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .count
    .unwrap_or(0);

    assert_eq!(remaining_votes, 0);

    cleanup_test_data(&pool).await;
}

#[tokio::test]
async fn test_concurrent_vote_handling() {
    let (_db, pool) = setup_test_environment().await;

    let cid = "QmConcurrentTest";
    let algo_name = "Concurrent Test Algorithm";
    let algo_link = "http://example.com/concurrent-test";

    // Create algorithm
    sqlx::query!(
        r#"
        INSERT INTO algo (cid, name, algo_link)
        VALUES ($1, $2, $3)
        "#,
        cid,
        algo_name,
        algo_link
    )
    .execute(&pool)
    .await
    .unwrap();

    // Add committee members
    for i in 0..10 {
        sqlx::query!(
            r#"
            INSERT INTO committee_member (member_wallet, is_approved)
            VALUES ($1, $2)
            "#,
            format!("0xmember{:02}", i),
            true
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    // Simulate concurrent votes
    let mut handles = vec![];

    for i in 0..10 {
        let pool_clone = pool.clone();
        let cid_clone = cid.to_string();

        let handle = tokio::spawn(async move {
            sqlx::query!(
                r#"
                INSERT INTO vote (algo_cid, voter, approve, voted_at)
                VALUES ($1, $2, $3, NOW())
                ON CONFLICT (algo_cid, voter)
                DO UPDATE SET approve = $3, voted_at = NOW()
                "#,
                cid_clone,
                format!("0xmember{:02}", i),
                i % 2 == 0 // Even indices approve, odd reject
            )
            .execute(&pool_clone)
            .await
        });

        handles.push(handle);
    }

    // Wait for all votes to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    // Verify final vote count
    let vote_count = sqlx::query!(
        "SELECT COUNT(*) as count FROM vote WHERE algo_cid = $1",
        cid
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .count
    .unwrap_or(0);

    assert_eq!(vote_count, 10);

    cleanup_test_data(&pool).await;
}

#[tokio::test]
async fn test_data_validation_constraints() {
    let (_db, pool) = setup_test_environment().await;

    // Test CID that's too long (should fail)
    let long_cid = "Q".repeat(300);
    let result = sqlx::query!(
        r#"
        INSERT INTO data_usage (scientist_wallet, cid, dataset, used_at)
        VALUES ($1, $2, $3, NOW())
        "#,
        "0xtest",
        &long_cid[..],
        "dataset"
    )
    .execute(&pool)
    .await;

    assert!(result.is_err());

    // Test valid CID length (should succeed)
    let valid_cid = "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG";
    let result = sqlx::query!(
        r#"
        INSERT INTO data_usage (scientist_wallet, cid, dataset, used_at)
        VALUES ($1, $2, $3, NOW())
        "#,
        "0xtest",
        valid_cid,
        "dataset"
    )
    .execute(&pool)
    .await;

    assert!(result.is_ok());

    cleanup_test_data(&pool).await;
}

/// Mock scheduler handler for testing
// Mock implementations removed as scheduler is no longer part of the architecture

// TODO: Re-enable this test after updating to use AlgoExecutor instead of Scheduler
// #[tokio::test]
// async fn test_chainsync_scheduler_integration() {
//     let (db, pool) = setup_test_environment().await;

//     // Create mock scheduler handler
//     let mock_handler = Arc::new(MockSchedulerHandler::default());

//     // Create scheduler with mock handler
//     let scheduler_config = SchedulerConfig::default();
//     let algo_scheduler = Arc::new(
//         secure::workers::scheduler::AlgoScheduler::new(mock_handler.clone(), scheduler_config)
//             .expect("Failed to create algo scheduler"),
//     );
//     let scheduler = Arc::new(Scheduler::new(algo_scheduler));

//     // Create mock notification service
//     let notifier = Arc::new(Notifier::new());

//     // Create contract caller (mock for testing)
//     let config = Config::from_env().expect("Failed to load config");
//     let contract_caller = Arc::new(
//         ContractCaller::new(config.chain.clone())
//             .await
//             .expect("Failed to create contract caller"),
//     );

//     // Create ChainSyncWorker with scheduler
//     let chain_sync = Arc::new(ChainSyncWorker::new(
//         db.clone(),
//         contract_caller,
//         notifier,
//         scheduler.clone(),
//     ));

//     // Test 1: Algorithm approval should trigger schedule_run
//     let algo_cid = "QmTestAlgoForScheduler".to_string();
//     let algo_id = sqlx::query!(
//         r#"
//         INSERT INTO algo (cid, name, algo_link)
//         VALUES ($1, $2, $3)
//         RETURNING id
//         "#,
//         algo_cid,
//         "Test Algorithm",
//         "https://github.com/test/algo"
//     )
//     .fetch_one(&pool)
//     .await
//     .unwrap()
//     .id;

//     let exe_id = sqlx::query!(
//         r#"
//         INSERT INTO algo_exe (algo_id, review_status)
//         VALUES ($1, $2)
//         RETURNING id
//         "#,
//         algo_id,
//         AlgoReviewStatus::Reviewing as AlgoReviewStatus
//     )
//     .fetch_one(&pool)
//     .await
//     .unwrap()
//     .id;

//     // TODO: handle_log is private, need to refactor test approach
//     // Options:
//     // 1. Make handle_log public with #[cfg(test)]
//     // 2. Test through the public interface (start() method with mocked blockchain)
//     // 3. Create a test-specific trait for ChainSyncWorker

//     // For now, we'll simulate the expected behavior without calling handle_log directly
//     // This test verifies that the scheduler integration works correctly

//     // Manually trigger scheduler as if AlgorithmResolved event was processed
//     mock_handler.on_run(exe_id as u64).await;

//     // Wait a bit for async operations
//     tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

//     // Verify scheduler received the event
//     let run_calls = mock_handler.run_calls.lock().await;
//     assert_eq!(run_calls.len(), 1);
//     assert_eq!(run_calls[0], exe_id as u64);

//     // Test 2: Resolution scheduling should work
//     // TODO: Same issue as above - handle_log is private
//     // Manually trigger scheduler as if ExecutionSubmitted event was processed
//     mock_handler
//         .on_resolve(exe_id as u64, algo_cid.clone(), chrono::Utc::now())
//         .await;

//     // Wait a bit for async operations
//     tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

//     // Verify scheduler received the resolution event
//     let resolve_calls = mock_handler.resolve_calls.lock().await;
//     assert_eq!(resolve_calls.len(), 1);
//     assert_eq!(resolve_calls[0].0, exe_id as u64);

//     // TODO: Add more comprehensive tests once handle_log is accessible
// }

// TODO: Re-enable this test after updating to use AlgoExecutor instead of Scheduler
// #[tokio::test]
// async fn test_chainsync_recover_unresolved_algorithms() {
//     let (db, pool) = setup_test_environment().await;

//     // Create mock scheduler handler
//     let mock_handler = Arc::new(MockSchedulerHandler::default());

//     // Create scheduler with mock handler
//     let scheduler_config = SchedulerConfig::default();
//     let algo_scheduler = Arc::new(
//         secure::workers::scheduler::AlgoScheduler::new(mock_handler.clone(), scheduler_config)
//             .expect("Failed to create algo scheduler"),
//     );
//     let scheduler = Arc::new(Scheduler::new(algo_scheduler));

//     // Create mock services
//     let notifier = Arc::new(Notifier::new());
//     let config = Config::from_env().expect("Failed to load config");
//     let contract_caller = Arc::new(
//         ContractCaller::new(config.chain.clone())
//             .await
//             .expect("Failed to create contract caller"),
//     );

//     // Create ChainSyncWorker
//     let chain_sync = Arc::new(ChainSyncWorker::new(
//         db.clone(),
//         contract_caller,
//         notifier,
//         scheduler.clone(),
//     ));

//     // Create test data with future vote end time
//     let future_time = chrono::Utc::now() + chrono::Duration::hours(2);
//     let algo_cid = "QmUnresolvedAlgo".to_string();

//     let algo_id = sqlx::query!(
//         r#"
//         INSERT INTO algo (cid, name, algo_link)
//         VALUES ($1, $2, $3)
//         RETURNING id
//         "#,
//         algo_cid,
//         "Unresolved Algorithm",
//         "http://example.com/unresolved"
//     )
//     .fetch_one(&pool)
//     .await
//     .unwrap()
//     .id;

//     // Create execution with REVIEWING status and future vote end time
//     let exe_id = sqlx::query!(
//         r#"
//         INSERT INTO algo_exe (algo_id, review_status, vote_start_time, vote_end_time)
//         VALUES ($1, $2, NOW(), $3)
//         RETURNING id
//         "#,
//         algo_id,
//         AlgoReviewStatus::Reviewing as AlgoReviewStatus,
//         future_time
//     )
//     .fetch_one(&pool)
//     .await
//     .unwrap()
//     .id;

//     // Create confirmed transaction for the execution
//     let tx_hash = format!("{:?}", alloy::primitives::B256::from([3u8; 32]));
//     sqlx::query!(
//         r#"
//         INSERT INTO blockchain_transaction (tx_hash, entity_id, entity_type, status, block_number, block_timestamp)
//         VALUES ($1, $2, $3, $4, $5, NOW())
//         "#,
//         tx_hash,
//         exe_id,
//         "EXECUTION",
//         secure::models::TransactionStatus::Confirmed as secure::models::TransactionStatus,
//         200i64
//     )
//     .execute(&pool)
//     .await
//     .unwrap();

//     // Run recovery
//     chain_sync.recover_resolve_tasks().await.unwrap();

//     // Wait a bit for async operations
//     tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

//     // Verify schedule_resolve was called for the unresolved algorithm
//     let resolve_calls = mock_handler.resolve_calls.lock().await;
//     assert_eq!(resolve_calls.len(), 1);
//     assert_eq!(resolve_calls[0].0, exe_id as u64);
//     assert_eq!(resolve_calls[0].1, algo_cid);

//     cleanup_test_data(&pool).await;
// }

#[tokio::test]
async fn test_algorithm_execution_status_flow() {
    let (_db, pool) = setup_test_environment().await;

    // Create algorithm
    let algo_id = sqlx::query!(
        r#"
        INSERT INTO algo (cid, name, algo_link)
        VALUES ($1, $2, $3)
        RETURNING id
        "#,
        "QmStatusFlowTest",
        "Status Flow Test",
        "http://example.com/status-flow"
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .id;

    // Create execution with initial status
    let exe_id = sqlx::query!(
        r#"
        INSERT INTO algo_exe (algo_id, review_status)
        VALUES ($1, $2)
        RETURNING id
        "#,
        algo_id,
        AlgoReviewStatus::Reviewing as AlgoReviewStatus
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .id;

    // Verify initial status
    let exe_status = sqlx::query!(
        r#"
        SELECT review_status as "review_status: AlgoReviewStatus"
        FROM algo_exe
        WHERE id = $1
        "#,
        exe_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(exe_status.review_status, AlgoReviewStatus::Reviewing);

    // Update to approved
    sqlx::query!(
        r#"
        UPDATE algo_exe
        SET review_status = $2, updated_at = NOW()
        WHERE id = $1
        "#,
        exe_id,
        AlgoReviewStatus::Approved as AlgoReviewStatus
    )
    .execute(&pool)
    .await
    .unwrap();

    // Verify updated status
    let updated_status = sqlx::query!(
        r#"
        SELECT review_status as "review_status: AlgoReviewStatus"
        FROM algo_exe
        WHERE id = $1
        "#,
        exe_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(updated_status.review_status, AlgoReviewStatus::Approved);

    cleanup_test_data(&pool).await;
}
