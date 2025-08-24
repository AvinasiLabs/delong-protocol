//! Integration tests for authentication flow

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
async fn test_health_check() {
    let app = setup_clean_test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");
    assert_eq!(body["data"]["status"], "healthy");
}

#[tokio::test]
async fn test_user_registration_flow() {
    println!("Starting test_user_registration_flow");
    let app = setup_clean_test_app().await;
    println!("App created successfully");

    let username = generate_test_username("testuser");
    let email = generate_test_email("test");
    println!(
        "Generated test credentials - username: {}, email: {}",
        username, email
    );

    // First send verification code
    println!("Sending verification code...");
    let send_code_payload = json!({
        "email": email,
        "verification_type": "email",
        "language": "en"
    });
    let request = json_request("POST", "/auth/send-code", send_code_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let send_body = extract_json_body(response).await;
    println!("Verification code sent successfully: {:?}", send_body);

    // Wait a bit to ensure Redis write is complete
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let register_payload = json!({
        "username": username,
        "email": email,
        "password": "TestPassword123!",
        "verification_code": "1234"
    });
    println!("Created register payload: {:?}", register_payload);

    println!("Creating request...");
    let request = json_request("POST", "/auth/register", register_payload);
    println!("Request created, sending to app.oneshot()...");

    let response = app.oneshot(request).await.unwrap();
    println!("Received response with status: {}", response.status());

    assert_eq!(response.status(), StatusCode::OK);

    println!("Extracting JSON body...");
    let body = extract_json_body(response).await;
    println!("Response body: {:?}", body);

    assert_eq!(body["code"], "SUCCESS");
    assert!(body["data"]["access_token"].is_string());
    assert!(body["data"]["refresh_token"].is_string());
    assert_eq!(body["data"]["user"]["username"], username);
    assert_eq!(body["data"]["user"]["email"], email);
    println!("Test completed successfully");
}

#[tokio::test]
async fn test_user_login_flow() {
    let app = setup_clean_test_app().await;

    let username = generate_test_username("logintest");
    let email = generate_test_email("login");
    let password = "TestPassword123!";

    // First send verification code
    let send_code_payload = json!({
        "email": email,
        "verification_type": "email",
        "language": "en"
    });
    let request = json_request("POST", "/auth/send-code", send_code_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Then register a user
    let register_payload = json!({
        "username": username,
        "email": email,
        "password": password,
        "verification_code": "1234"
    });

    let request = json_request("POST", "/auth/register", register_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Now test login
    let login_payload = json!({
        "email": email,
        "password": password
    });

    let request = json_request("POST", "/auth/login", login_payload);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");
    assert!(body["data"]["access_token"].is_string());
    assert!(body["data"]["refresh_token"].is_string());
    assert_eq!(body["data"]["user"]["email"], email);
}

#[tokio::test]
async fn test_duplicate_registration() {
    let app = setup_clean_test_app().await;

    let username = generate_test_username("duplicate");
    let email = generate_test_email("duplicate");

    // First send verification code
    let send_code_payload = json!({
        "email": email,
        "verification_type": "email",
        "language": "en"
    });
    let request = json_request("POST", "/auth/send-code", send_code_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let register_payload = json!({
        "username": username,
        "email": email,
        "password": "TestPassword123!",
        "verification_code": "1234"
    });

    // First registration should succeed
    let request = json_request("POST", "/auth/register", register_payload.clone());
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Second registration with same email should fail
    // First need to send a new verification code since the first one was consumed
    let send_code_payload2 = json!({
        "email": email,
        "verification_type": "email",
        "language": "en"
    });
    let request = json_request("POST", "/auth/send-code", send_code_payload2);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let duplicate_username = generate_test_username("another");
    let duplicate_payload = json!({
        "username": duplicate_username,
        "email": email,
        "password": "AnotherPassword123!",
        "verification_code": "1234"
    });

    let request = json_request("POST", "/auth/register", duplicate_payload);
    let response = app.clone().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "CONFLICT_ERROR");
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("already registered")
    );
}

#[tokio::test]
async fn test_invalid_login() {
    let app = setup_clean_test_app().await;

    // Try to login with non-existent user
    let login_payload = json!({
        "email": "nonexistent@test.com",
        "password": "WrongPassword123!"
    });

    let request = json_request("POST", "/auth/login", login_payload);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "AUTHENTICATION_ERROR");
    assert!(body["message"].as_str().unwrap().contains("Invalid"));
}

#[tokio::test]
async fn test_protected_endpoint_without_auth() {
    let app = setup_clean_test_app().await;

    // Try to access protected endpoint without token (using AI audit endpoint)
    let request = Request::builder()
        .method("GET")
        .uri("/api/ai-audit")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;

    // Debug: Print the actual response body
    eprintln!(
        "Protected endpoint without auth response: {}",
        serde_json::to_string_pretty(&body).unwrap()
    );

    assert_eq!(body["code"], "AUTHENTICATION_ERROR");
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("JWT token required")
    );
}

#[tokio::test]
async fn test_protected_endpoint_with_auth() {
    let app = setup_clean_test_app().await;

    let username = generate_test_username("authtest");
    let email = generate_test_email("auth");

    // First send verification code
    let send_code_payload = json!({
        "email": email,
        "verification_type": "email",
        "language": "en"
    });
    let request = json_request("POST", "/auth/send-code", send_code_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Register and login
    let register_payload = json!({
        "username": username,
        "email": email,
        "password": "TestPassword123!",
        "verification_code": "1234"
    });

    let request = json_request("POST", "/auth/register", register_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    let token = body["data"]["access_token"].as_str().unwrap();

    // Now access protected endpoint with token (using update wallet endpoint as a simple test)
    let wallet_payload = json!({
        "wallet_address": "0x1234567890123456789012345678901234567890"
    });

    let request = authenticated_request("POST", "/api/user/update-wallet", wallet_payload, token);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    // Update wallet should succeed
    assert_eq!(body["code"], "SUCCESS");
    assert_eq!(
        body["data"]["wallet_address"],
        "0x1234567890123456789012345678901234567890"
    );
}

#[tokio::test]
#[ignore = "Refresh token endpoint not implemented yet"]
async fn test_refresh_token() {
    let app = setup_clean_test_app().await;

    let username = generate_test_username("refresh");
    let email = generate_test_email("refresh");

    // Register a user to get tokens
    let register_payload = json!({
        "username": username,
        "email": email,
        "password": "TestPassword123!",
        "verification_code": "1234"
    });

    let request = json_request("POST", "/auth/register", register_payload);
    let response = app.clone().oneshot(request).await.unwrap();

    let body = extract_json_body(response).await;
    let refresh_token = body["data"]["refresh_token"].as_str().unwrap();

    // Use refresh token to get new access token
    let refresh_payload = json!({
        "refresh_token": refresh_token
    });

    let request = json_request("POST", "/auth/refresh", refresh_payload);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");
    assert!(body["data"]["access_token"].is_string());
    assert!(body["data"]["refresh_token"].is_string());
}

#[tokio::test]
async fn test_update_wallet_address() {
    let app = setup_clean_test_app().await;

    let username = generate_test_username("wallettest");
    let email = generate_test_email("wallet");

    // First send verification code
    let send_code_payload = json!({
        "email": email,
        "verification_type": "email",
        "language": "en"
    });
    let request = json_request("POST", "/auth/send-code", send_code_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Register a user
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

    // Update wallet address
    let wallet_address = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb7";
    let update_payload = json!({
        "wallet_address": wallet_address
    });

    let request = authenticated_request("POST", "/api/user/update-wallet", update_payload, token);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");
    assert_eq!(body["data"]["wallet_address"], wallet_address);
}

#[tokio::test]
async fn test_send_verification_code() {
    let app = setup_clean_test_app().await;

    let email = generate_test_email("verify");

    let send_code_payload = json!({
        "email": email,
        "verification_type": "email"
    });

    let request = json_request("POST", "/auth/send-code", send_code_payload);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");
    assert_eq!(
        body["data"]["message"],
        "Verification code sent successfully"
    );
}

#[tokio::test]
async fn test_google_login() {
    let app = setup_clean_test_app().await;

    // Test with invalid token (should fail)
    let google_login_payload = json!({
        "access_token": "invalid_test_token",
        "id_token": "invalid_id_token"
    });

    let request = json_request("POST", "/auth/google/login", google_login_payload);
    let response = app.oneshot(request).await.unwrap();

    // Should fail with invalid token
    assert_ne!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_invalid_registration_data() {
    let app = setup_clean_test_app().await;

    // Test with invalid email
    let invalid_email_payload = json!({
        "username": "testuser",
        "email": "invalid-email",
        "password": "TestPassword123!",
        "verification_code": "1234"
    });

    let request = json_request("POST", "/auth/register", invalid_email_payload);
    let response = app.clone().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "VALIDATION_ERROR");

    // Test with short password
    let short_password_payload = json!({
        "username": "testuser2",
        "email": "test@example.com",
        "password": "123",
        "verification_code": "1234"
    });

    let request = json_request("POST", "/auth/register", short_password_payload);
    let response = app.clone().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "VALIDATION_ERROR");

    // Test with invalid username
    let invalid_username_payload = json!({
        "username": "ab",  // Too short
        "email": "test2@example.com",
        "password": "TestPassword123!",
        "verification_code": "1234"
    });

    let request = json_request("POST", "/auth/register", invalid_username_payload);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "VALIDATION_ERROR");
}
