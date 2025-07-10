//! Tests for main API endpoints like health check, CORS, etc.

use axum::http::StatusCode;
use common::{ApiResponse, ResponseCode};
use crate::api::helpers::{create_test_router, make_json_request};


#[tokio::test]
async fn test_health_endpoint() {
    let router = create_test_router().await;

    let (status, _, body_json) = make_json_request::<()>(&router, "GET", "/health", None).await;

    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<String> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
    assert!(response.data.unwrap().contains("Secure service is healthy"));
    println!("✅ Health endpoint test passed");
}

#[tokio::test]
async fn test_invalid_endpoints() {
    let router = create_test_router().await;

    let (status, _, _) = make_json_request::<()>(&router, "GET", "/invalid-endpoint", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _, _) = make_json_request::<()>(&router, "POST", "/api/invalid", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    println!("✅ Invalid endpoints test passed");
} 