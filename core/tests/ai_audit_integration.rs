//! AI Audit integration tests for the core service
//!
//! These tests verify the AI audit functionality including:
//! - Creating AI audit requests
//! - Querying AI audit reports
//! - Authorization and authentication
//! - Pagination and filtering

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::{
    authenticated_request, extract_json_body, generate_test_email, generate_test_username,
    json_request, setup_clean_test_app,
};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
#[ignore = "AI audit service URL not configured for test environment"]
async fn test_create_ai_audit() {
    let app = setup_clean_test_app().await;

    // First register a user to get a token
    let username = generate_test_username("aiaudit");
    let email = generate_test_email("aiaudit");

    let register_payload = json!({
        "username": username,
        "email": email,
        "password": "TestPassword123!",
        "verification_code": "1234"
    });

    let request = json_request("POST", "/auth/register", register_payload);
    let response = app.clone().oneshot(request).await.unwrap();

    let body = extract_json_body(response).await;
    let token = body["data"]["access_token"].as_str().unwrap();

    // Create AI audit request
    let audit_payload = json!({
        "algorithm_id": "test-algo-123",
        "execution_id": "test-exec-456",
        "github_url": "https://github.com/test/repo",
        "commit_hash": "abc123def456",
        "dataset_cid": "QmTest123456789",
        "author_wallet": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb7"
    });

    let request = authenticated_request("POST", "/api/ai-audit", audit_payload, token);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");
    assert!(body["data"]["id"].is_string());
    assert_eq!(body["data"]["status"], "pending");
}

#[tokio::test]
#[ignore = "AI audit service URL not configured for test environment"]
async fn test_get_ai_audit_reports() {
    let app = setup_clean_test_app().await;

    // First register a user to get a token
    let username = generate_test_username("aiaudit");
    let email = generate_test_email("aiaudit");

    let register_payload = json!({
        "username": username,
        "email": email,
        "password": "TestPassword123!",
        "verification_code": "1234"
    });

    let request = json_request("POST", "/auth/register", register_payload);
    let response = app.clone().oneshot(request).await.unwrap();

    let body = extract_json_body(response).await;
    let token = body["data"]["access_token"].as_str().unwrap();

    // Query AI audit reports
    let request = Request::builder()
        .method("GET")
        .uri("/api/ai-audit?algorithm_id=test-algo-123")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");
    assert!(body["data"].is_array());
}

#[tokio::test]
#[ignore = "AI audit service URL not configured for test environment"]
async fn test_ai_audit_without_auth() {
    let app = setup_clean_test_app().await;

    // Try to create AI audit without authentication
    let audit_payload = json!({
        "algorithm_id": "test-algo-123",
        "execution_id": "test-exec-456",
        "github_url": "https://github.com/test/repo",
        "commit_hash": "abc123def456"
    });

    let request = json_request("POST", "/api/ai-audit", audit_payload);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "AUTHENTICATION_ERROR");
    assert!(body["message"].as_str().unwrap().contains("Authorization"));
}

#[tokio::test]
#[ignore = "AI audit service URL not configured for test environment"]
async fn test_ai_audit_pagination() {
    let app = setup_clean_test_app().await;

    // First register a user to get a token
    let username = generate_test_username("aiaudit");
    let email = generate_test_email("aiaudit");

    let register_payload = json!({
        "username": username,
        "email": email,
        "password": "TestPassword123!",
        "verification_code": "1234"
    });

    let request = json_request("POST", "/auth/register", register_payload);
    let response = app.clone().oneshot(request).await.unwrap();

    let body = extract_json_body(response).await;
    let token = body["data"]["access_token"].as_str().unwrap();

    // Query with pagination
    let request = Request::builder()
        .method("GET")
        .uri("/api/ai-audit?algorithm_id=test-algo-123&page=1&limit=10")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");
    assert!(body["data"].is_array());
}

#[tokio::test]
#[ignore = "AI audit service URL not configured for test environment"]
async fn test_ai_audit_filtering() {
    let app = setup_clean_test_app().await;

    // First register a user to get a token
    let username = generate_test_username("aiaudit");
    let email = generate_test_email("aiaudit");

    let register_payload = json!({
        "username": username,
        "email": email,
        "password": "TestPassword123!",
        "verification_code": "1234"
    });

    let request = json_request("POST", "/auth/register", register_payload);
    let response = app.clone().oneshot(request).await.unwrap();

    let body = extract_json_body(response).await;
    let token = body["data"]["access_token"].as_str().unwrap();

    // Query with multiple filters
    let request = Request::builder()
        .method("GET")
        .uri("/api/ai-audit?github_url=https://github.com/test/repo&commit_hash=abc123")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");
    assert!(body["data"].is_array());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_test_username() {
        let username = generate_test_username("test");
        assert!(username.starts_with("test_"));
    }

    #[test]
    fn test_generate_test_email() {
        let email = generate_test_email("test");
        assert!(email.starts_with("test+"));
        assert!(email.ends_with("@test.com"));
    }
}
