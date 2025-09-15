//! Integration tests for wallet connection functionality

mod common;

use axum::http::StatusCode;
use common::{
    cookie_authenticated_request, extract_cookies_from_response, extract_json_body,
    generate_test_email, generate_test_username, json_request, setup_clean_test_app,
};
use serde_json::json;
use tower::ServiceExt;

/// Test updating wallet address for authenticated user
#[tokio::test]
async fn test_update_wallet_address() {
    let app = setup_clean_test_app().await;

    // Create test user credentials
    let email = generate_test_email("wallet_test");
    let username = generate_test_username("wallet_test");
    let password = "TestPassword123!";

    // Send verification code
    let request = json_request(
        "POST",
        "/auth/send-code",
        json!({
            "email": email,
            "verification_type": "email",
            "language": "en"
        }),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Register user with fixed test code
    let request = json_request(
        "POST",
        "/auth/register",
        json!({
            "email": email,
            "username": username,
            "password": password,
            "verification_code": "1234"  // Fixed code in test mode
        }),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Login to get authentication cookies
    let request = json_request(
        "POST",
        "/auth/login",
        json!({
            "email": email,
            "password": password
        }),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Extract cookies for authentication
    let (access_token, refresh_token) = extract_cookies_from_response(&response);
    assert!(access_token.is_some(), "Access token should be present");
    assert!(refresh_token.is_some(), "Refresh token should be present");

    // Update wallet address
    let wallet_address = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb7";
    let request = cookie_authenticated_request(
        "PUT",
        "/api/auth/wallet",
        json!({
            "wallet_address": wallet_address
        }),
        access_token.as_ref().unwrap(),
        refresh_token.as_ref().unwrap(),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");
    assert_eq!(
        body["payload"]["wallet_address"].as_str(),
        Some(wallet_address)
    );

    // Verify by fetching user info
    let request = cookie_authenticated_request(
        "GET",
        "/api/user/me",
        json!({}),
        access_token.as_ref().unwrap(),
        refresh_token.as_ref().unwrap(),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(
        body["payload"]["wallet_address"].as_str(),
        Some(wallet_address)
    );
}

/// Test wallet address format validation
#[tokio::test]
async fn test_invalid_wallet_address_format() {
    let app = setup_clean_test_app().await;

    // Create test user credentials
    let email = generate_test_email("wallet_invalid");
    let username = generate_test_username("wallet_invalid");
    let password = "TestPassword123!";

    // Send verification code
    let request = json_request(
        "POST",
        "/auth/send-code",
        json!({
            "email": email,
            "verification_type": "email",
            "language": "en"
        }),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Register user
    let request = json_request(
        "POST",
        "/auth/register",
        json!({
            "email": email,
            "username": username,
            "password": password,
            "verification_code": "1234"
        }),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Login to get authentication cookies
    let request = json_request(
        "POST",
        "/auth/login",
        json!({
            "email": email,
            "password": password
        }),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let (access_token, refresh_token) = extract_cookies_from_response(&response);
    assert!(access_token.is_some());
    assert!(refresh_token.is_some());

    // Test various invalid wallet addresses
    let invalid_addresses = vec![
        "invalid_address",                             // No 0x prefix
        "0xinvalid",                                   // Invalid characters
        "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",   // Too short (39 chars)
        "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb77", // Too long (41 chars)
        "742d35Cc6634C0532925a3b844Bc9e7595f0bEb7",    // Missing 0x prefix
        "0x742d35cc6634c0532925a3b844bc9e7595f0beb7",  // Valid but lowercase
    ];

    for invalid_address in invalid_addresses {
        let request = cookie_authenticated_request(
            "PUT",
            "/api/auth/wallet",
            json!({
                "wallet_address": invalid_address
            }),
            access_token.as_ref().unwrap(),
            refresh_token.as_ref().unwrap(),
        );
        let response = app.clone().oneshot(request).await.unwrap();

        // Should return validation error for invalid formats
        if invalid_address == "0x742d35cc6634c0532925a3b844bc9e7595f0beb7" {
            // Lowercase addresses should be accepted
            assert_eq!(response.status(), StatusCode::OK);
        } else {
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let body = extract_json_body(response).await;
            assert_eq!(body["code"], "VALIDATION_ERROR");
        }
    }
}

/// Test that wallet address is included in auth response after update
#[tokio::test]
async fn test_wallet_in_auth_response() {
    let app = setup_clean_test_app().await;

    // Create test user credentials
    let email = generate_test_email("wallet_auth");
    let username = generate_test_username("wallet_auth");
    let password = "TestPassword123!";

    // Send verification code
    let request = json_request(
        "POST",
        "/auth/send-code",
        json!({
            "email": email,
            "verification_type": "email",
            "language": "en"
        }),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Register user
    let request = json_request(
        "POST",
        "/auth/register",
        json!({
            "email": email,
            "username": username,
            "password": password,
            "verification_code": "1234"
        }),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // First login - should have no wallet
    let request = json_request(
        "POST",
        "/auth/login",
        json!({
            "email": email,
            "password": password
        }),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert!(body["payload"]["user"]["wallet_address"].is_null());

    // Extract cookies for wallet update
    let request = json_request(
        "POST",
        "/auth/login",
        json!({
            "email": email,
            "password": password
        }),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    let (access_token, refresh_token) = extract_cookies_from_response(&response);

    // Update wallet address
    let wallet_address = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb7";
    let request = cookie_authenticated_request(
        "PUT",
        "/api/auth/wallet",
        json!({
            "wallet_address": wallet_address
        }),
        access_token.as_ref().unwrap(),
        refresh_token.as_ref().unwrap(),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Login again - should have wallet
    let request = json_request(
        "POST",
        "/auth/login",
        json!({
            "email": email,
            "password": password
        }),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(
        body["payload"]["user"]["wallet_address"].as_str(),
        Some(wallet_address)
    );
}

/// Test wallet update requires authentication
#[tokio::test]
async fn test_wallet_update_requires_auth() {
    let app = setup_clean_test_app().await;

    // Try to update wallet without authentication
    let wallet_address = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb7";
    let request = json_request(
        "PUT",
        "/api/auth/wallet",
        json!({
            "wallet_address": wallet_address
        }),
    );
    let response = app.clone().oneshot(request).await.unwrap();

    // Should return unauthorized
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "AUTHENTICATION_ERROR");
}

/// Test multiple wallet updates
#[tokio::test]
async fn test_multiple_wallet_updates() {
    let app = setup_clean_test_app().await;

    // Create test user
    let email = generate_test_email("wallet_multiple");
    let username = generate_test_username("wallet_multiple");
    let password = "TestPassword123!";

    // Send verification code
    let request = json_request(
        "POST",
        "/auth/send-code",
        json!({
            "email": email,
            "verification_type": "email",
            "language": "en"
        }),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Register user
    let request = json_request(
        "POST",
        "/auth/register",
        json!({
            "email": email,
            "username": username,
            "password": password,
            "verification_code": "1234"
        }),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Login to get authentication cookies
    let request = json_request(
        "POST",
        "/auth/login",
        json!({
            "email": email,
            "password": password
        }),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let (access_token, refresh_token) = extract_cookies_from_response(&response);
    assert!(access_token.is_some());
    assert!(refresh_token.is_some());

    // Update wallet multiple times
    let wallets = vec![
        "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb7",
        "0x5aAeb6053f3E94C9b9A09f33669435E7Ef1BeAed",
        "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359",
    ];

    for wallet_address in wallets.iter() {
        let request = cookie_authenticated_request(
            "PUT",
            "/api/auth/wallet",
            json!({
                "wallet_address": wallet_address
            }),
            access_token.as_ref().unwrap(),
            refresh_token.as_ref().unwrap(),
        );
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = extract_json_body(response).await;
        assert_eq!(
            body["payload"]["wallet_address"].as_str(),
            Some(*wallet_address)
        );
    }

    // Verify final wallet address
    let request = cookie_authenticated_request(
        "GET",
        "/api/user/me",
        json!({}),
        access_token.as_ref().unwrap(),
        refresh_token.as_ref().unwrap(),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(
        body["payload"]["wallet_address"].as_str(),
        Some(*wallets.last().unwrap())
    );
}
