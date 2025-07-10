use std::time::Duration;
use tokio::time::sleep;
use secure::services::blockchain_sync::BlockchainSyncService;

#[tokio::test]
async fn test_blockchain_sync_service_starts_and_stops() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let service = BlockchainSyncService::new_for_test().await.unwrap();

    // Health check should fail before starting
    assert!(service.health_check().await.is_err());

    // Start the service
    service.start().await.unwrap();

    // Health check should pass after starting
    assert!(service.health_check().await.is_ok());

    // Let it run for a bit
    sleep(Duration::from_secs(1)).await;

    // Stop the service
    service.stop().await.unwrap();

    // Health check should fail after stopping
    assert!(service.health_check().await.is_err());
}

#[tokio::test]
async fn test_transaction_submission_and_retrieval() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let service = BlockchainSyncService::new_for_test().await.unwrap();
    service.start().await.unwrap();

    let tx_type = "test_transaction";
    let tx_data = serde_json::json!({ "key": "value" });
    let from_addr = "0x123";

    let tx_hash = service
        .submit_transaction(tx_type, tx_data, from_addr)
        .await
        .unwrap();

    assert!(!tx_hash.is_empty());

    // Retrieve transaction status
    let tx_status = service.get_transaction_status(&tx_hash).await.unwrap();

    assert_eq!(tx_status.tx_hash, tx_hash);
    assert_eq!(tx_status.entity_type.unwrap(), tx_type);
    assert_eq!(tx_status.status.unwrap(), "PENDING");

    service.stop().await.unwrap();
} 