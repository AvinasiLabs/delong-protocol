//! Tests for blockchain transaction endpoints

use axum::http::StatusCode;
use common::{
    models::{
        blockchain::{
            BlockchainTransaction, SubmitTransactionRequest, TransactionResponse,
        },
        PaginatedResponse,
    },
    ApiResponse, ResponseCode,
};

use super::helpers::{create_auth_test_router, generate_admin_token, make_auth_request, setup_test_environment};

#[tokio::test]
async fn test_submit_transaction() {
    let router = setup_test_environment().await;
    let admin_token = generate_admin_token("admin");

    let tx_request = SubmitTransactionRequest {
        transaction_type: "CONTRACT_CALL".to_string(),
        data: serde_json::json!({
            "contract_address": "0x5FbDB2315678afecb367f032d93F642f64180aa3",
            "function_name": "set",
            "args": ["hello", "world"]
        }),
        from_address: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".to_string(), // Anvil default address
    };

    let (status, _, body_json) = make_auth_request(
        &router,
        "POST",
        "/api/contracts/transactions",
        Some(tx_request),
        &admin_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<String> =
        serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
    let tx_hash = response.data.unwrap();
    assert!(tx_hash.starts_with("0x"));
}

#[tokio::test]
async fn test_list_transactions() {
    let router = setup_test_environment().await;
    let admin_token = generate_admin_token("admin");

    let (status, _, body_json) = make_auth_request::<()>(
        &router,
        "GET",
        "/api/contracts/transactions",
        None,
        &admin_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<Vec<TransactionResponse>> =
        serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
}
