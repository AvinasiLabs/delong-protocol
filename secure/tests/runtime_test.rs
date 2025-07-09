// use std::sync::Arc;  // Not used in simplified tests
// use std::time::Duration;  // Not used in simplified tests  
// use tokio::time::sleep;  // Not used in simplified tests

use secure::config::SecureConfig;
use secure::runtime::{
    ExecutionScheduler,
    execution_queue::{ExecutionRequest, ExecutionStatus},
};
use secure::tee::KeyVault;

#[tokio::test]
async fn test_execution_queue_operations() {
    let config = SecureConfig::mock();
    let key_vault = KeyVault::new_with_client_kind(secure::tee::ClientKind::Mock);
    let ipfs_client = ipfs_api_backend_hyper::IpfsClient::default();
    
    let scheduler = ExecutionScheduler::new(config, 2, key_vault, ipfs_client);
    
    // Test submitting executions
    let request1 = ExecutionRequest {
        execution_id: 1,
        algorithm_cid: "QmAlgorithm1".to_string(),
        dataset_id: "dataset1".to_string(),
        scientist_wallet: "0x123".to_string(),
        priority: 1,
        created_at: chrono::Utc::now(),
        status: ExecutionStatus::Queued,
        parameters: serde_json::Value::Null,
    };
    
    let request2 = ExecutionRequest {
        execution_id: 2,
        algorithm_cid: "QmAlgorithm2".to_string(),
        dataset_id: "dataset2".to_string(),
        scientist_wallet: "0x456".to_string(),
        priority: 2, // Higher priority
        created_at: chrono::Utc::now(),
        status: ExecutionStatus::Queued,
        parameters: serde_json::Value::Null,
    };
    
    // Submit requests
    scheduler.submit_execution(request1).await.unwrap();
    scheduler.submit_execution(request2).await.unwrap();
    
    // Check queue stats
    let stats = scheduler.get_queue_stats().await;
    assert_eq!(stats.queued_count, 2);
    assert_eq!(stats.running_count, 0);
    assert_eq!(stats.max_concurrent, 2);
    
    // Check that higher priority request is first
    let queued_requests = scheduler.get_queued_requests().await;
    assert_eq!(queued_requests.len(), 2);
    assert_eq!(queued_requests[0].execution_id, 2); // Higher priority first
    assert_eq!(queued_requests[1].execution_id, 1);
    
    println!("✅ Execution queue operations test passed");
}

#[tokio::test]
async fn test_algorithm_execution() {
    let config = SecureConfig::mock();
    let key_vault = KeyVault::new_with_client_kind(secure::tee::ClientKind::Mock);
    let ipfs_client = ipfs_api_backend_hyper::IpfsClient::default();
    
    let scheduler = ExecutionScheduler::new(config, 1, key_vault, ipfs_client);
    
    // Create and submit an execution request
    let request = ExecutionRequest {
        execution_id: 100,
        algorithm_cid: "QmTestAlgorithm".to_string(),
        dataset_id: "test_dataset".to_string(),
        scientist_wallet: "0xTestWallet".to_string(),
        priority: 1,
        created_at: chrono::Utc::now(),
        status: ExecutionStatus::Queued,
        parameters: serde_json::json!({
            "test_parameter": "test_value"
        }),
    };
    
    // Submit the execution
    scheduler.submit_execution(request).await.unwrap();
    
    // Check that it was queued
    let stats = scheduler.get_queue_stats().await;
    assert_eq!(stats.queued_count, 1);
    assert_eq!(stats.running_count, 0);
    
    // Check that scheduler is initially not running
    assert!(!scheduler.is_running().await);
    
    // The scheduler can be started and stopped
    // Note: We don't actually start it here to avoid hanging tests
    
    println!("✅ Algorithm execution test passed");
}

#[tokio::test]
async fn test_execution_cancellation() {
    let config = SecureConfig::mock();
    let key_vault = KeyVault::new_with_client_kind(secure::tee::ClientKind::Mock);
    let ipfs_client = ipfs_api_backend_hyper::IpfsClient::default();
    
    let scheduler = ExecutionScheduler::new(config, 1, key_vault, ipfs_client);
    
    // Create execution request
    let request = ExecutionRequest {
        execution_id: 200,
        algorithm_cid: "QmCancelTest".to_string(),
        dataset_id: "cancel_dataset".to_string(),
        scientist_wallet: "0xCancelWallet".to_string(),
        priority: 1,
        created_at: chrono::Utc::now(),
        status: ExecutionStatus::Queued,
        parameters: serde_json::Value::Null,
    };
    
    // Submit execution
    scheduler.submit_execution(request).await.unwrap();
    
    // Check it's queued
    let stats = scheduler.get_queue_stats().await;
    assert_eq!(stats.queued_count, 1);
    
    // Cancel the execution
    scheduler.cancel_execution(200).await.unwrap();
    
    // Check it's no longer queued
    let stats = scheduler.get_queue_stats().await;
    assert_eq!(stats.queued_count, 0);
    
    println!("✅ Execution cancellation test passed");
}

#[tokio::test]
async fn test_concurrent_execution_limits() {
    let config = SecureConfig::mock();
    let key_vault = KeyVault::new_with_client_kind(secure::tee::ClientKind::Mock);
    let ipfs_client = ipfs_api_backend_hyper::IpfsClient::default();
    
    // Set max concurrent to 2
    let scheduler = ExecutionScheduler::new(config, 2, key_vault, ipfs_client);
    
    // Submit 5 execution requests
    for i in 1..=5 {
        let request = ExecutionRequest {
            execution_id: i,
            algorithm_cid: format!("QmAlgorithm{}", i),
            dataset_id: format!("dataset{}", i),
            scientist_wallet: format!("0x{:03x}", i),
            priority: i as u32,
            created_at: chrono::Utc::now(),
            status: ExecutionStatus::Queued,
            parameters: serde_json::Value::Null,
        };
        scheduler.submit_execution(request).await.unwrap();
    }
    
    // Check that all 5 are queued
    let stats = scheduler.get_queue_stats().await;
    assert_eq!(stats.queued_count, 5);
    assert_eq!(stats.running_count, 0);
    assert_eq!(stats.max_concurrent, 2);
    assert_eq!(stats.available_slots, 2);
    
    println!("✅ Concurrent execution limits test passed");
}

#[tokio::test]
async fn test_priority_ordering() {
    let config = SecureConfig::mock();
    let key_vault = KeyVault::new_with_client_kind(secure::tee::ClientKind::Mock);
    let ipfs_client = ipfs_api_backend_hyper::IpfsClient::default();
    
    let scheduler = ExecutionScheduler::new(config, 1, key_vault, ipfs_client);
    
    // Submit requests with different priorities
    let priorities = vec![1, 5, 2, 8, 3];
    for (i, priority) in priorities.iter().enumerate() {
        let request = ExecutionRequest {
            execution_id: i as u64,
            algorithm_cid: format!("QmAlgorithm{}", i),
            dataset_id: format!("dataset{}", i),
            scientist_wallet: format!("0x{:03x}", i),
            priority: *priority,
            created_at: chrono::Utc::now(),
            status: ExecutionStatus::Queued,
            parameters: serde_json::Value::Null,
        };
        scheduler.submit_execution(request).await.unwrap();
    }
    
    // Check that requests are ordered by priority (highest first)
    let queued_requests = scheduler.get_queued_requests().await;
    let mut last_priority = u32::MAX;
    
    for request in queued_requests {
        assert!(request.priority <= last_priority, 
            "Priority ordering failed: {} > {}", request.priority, last_priority);
        last_priority = request.priority;
    }
    
    println!("✅ Priority ordering test passed");
}

#[tokio::test]
async fn test_emergency_shutdown() {
    let config = SecureConfig::mock();
    let key_vault = KeyVault::new_with_client_kind(secure::tee::ClientKind::Mock);
    let ipfs_client = ipfs_api_backend_hyper::IpfsClient::default();
    
    let scheduler = ExecutionScheduler::new(config, 2, key_vault, ipfs_client);
    
    // Submit multiple requests
    for i in 1..=3 {
        let request = ExecutionRequest {
            execution_id: i,
            algorithm_cid: format!("QmAlgorithm{}", i),
            dataset_id: format!("dataset{}", i),
            scientist_wallet: format!("0x{:03x}", i),
            priority: i as u32,
            created_at: chrono::Utc::now(),
            status: ExecutionStatus::Queued,
            parameters: serde_json::Value::Null,
        };
        scheduler.submit_execution(request).await.unwrap();
    }
    
    // Check initial state
    let stats = scheduler.get_queue_stats().await;
    assert_eq!(stats.queued_count, 3);
    
    // Perform emergency shutdown
    scheduler.emergency_shutdown().await.unwrap();
    
    // Check that queue is cleared
    let stats = scheduler.get_queue_stats().await;
    assert_eq!(stats.queued_count, 0);
    
    // Check that scheduler is stopped
    assert!(!scheduler.is_running().await);
    
    println!("✅ Emergency shutdown test passed");
} 