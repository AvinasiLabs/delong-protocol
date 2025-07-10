//! Tests for algorithm execution endpoints

use axum::http::StatusCode;
use common::{
    models::{
        algo_exe::{AlgoExeData, AlgoExeSubmissionRequest, AlgoExeSubmissionResponse},
        PaginatedResponse,
    },
    ApiResponse, ResponseCode,
};

use super::helpers::{generate_user_token, make_auth_request, setup_test_environment};

#[tokio::test]
async fn test_algorithm_execution_submission_validation() {
    let router = setup_test_environment().await;
    let user_token = generate_user_token("test_user");

    // Test with empty github_repo
    let invalid_request = AlgoExeSubmissionRequest {
        github_repo: "".to_string(),
        commit_hash: "some-hash".to_string(),
        scientist_wallet: "0x1234567890123456789012345678901234567890".to_string(), // Valid wallet
        dataset: "1".to_string(), // Valid dataset ID
    };
    let (status, _, body_json) = make_auth_request(
        &router,
        "POST",
        "/api/algo-exes",
        Some(invalid_request),
        &user_token,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let response: ApiResponse<()> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::BadRequest);

    // Test with empty commit_hash
    let invalid_request = AlgoExeSubmissionRequest {
        github_repo: "some-repo".to_string(),
        commit_hash: "".to_string(),
        scientist_wallet: "0x1234567890123456789012345678901234567890".to_string(), // Valid wallet
        dataset: "1".to_string(), // Valid dataset ID
    };
    let (status, _, body_json) = make_auth_request(
        &router,
        "POST",
        "/api/algo-exes",
        Some(invalid_request),
        &user_token,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let response: ApiResponse<()> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::BadRequest);

    println!("✅ Algorithm execution submission validation test passed");
}

#[tokio::test]
async fn test_algorithm_execution_submission_valid() {
    let router = setup_test_environment().await;
    let user_token = generate_user_token("test_user");

    let valid_request = AlgoExeSubmissionRequest {
        github_repo: "https://github.com/delong-const/delong-brain".to_string(),
        commit_hash: "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2".to_string(),
        scientist_wallet: "0x1234567890123456789012345678901234567890".to_string(),
        dataset: "1".to_string(), // Use a numeric dataset ID
    };

    let (status, _, body_json) = make_auth_request(
        &router,
        "POST",
        "/api/algo-exes",
        Some(valid_request),
        &user_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<AlgoExeSubmissionResponse> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
    let data = response.data.unwrap();
    assert!(data.id > 0);
    assert!(!data.tx_hash.is_empty());
    assert_eq!(data.status, "submitted");

    println!("✅ Algorithm execution valid submission test passed");
}

#[tokio::test]
async fn test_algorithm_executions_list() {
    let router = setup_test_environment().await;
    let user_token = generate_user_token("test_user");

    let (status, _, body_json) =
        make_auth_request::<()>(&router, "GET", "/api/algo-exes", None, &user_token).await;
    assert_eq!(status, StatusCode::OK);

    let response: ApiResponse<PaginatedResponse<AlgoExeData>> =
        serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);

    println!("✅ Algorithm executions list test passed");
}

#[tokio::test]
async fn test_algorithm_execution_get_not_found() {
    let router = setup_test_environment().await;
    let user_token = generate_user_token("test_user");

    let (status, _, body_json) = make_auth_request::<()>(
        &router,
        "GET",
        "/api/algo-exes/99999", // Use a numeric ID that is unlikely to exist
        None,
        &user_token,
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let response: ApiResponse<()> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::NotFound);

    println!("✅ Algorithm execution get not found test passed");
} 