//! Integration tests for /api/user/me endpoint

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
async fn test_get_current_user_unauthenticated() {
    let app = setup_clean_test_app().await;

    // Make request without authentication
    let request = Request::builder()
        .method("GET")
        .uri("/api/user/me")
        .header("Content-Type", "application/json")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 200 with AUTHENTICATION_ERROR code
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "AUTHENTICATION_ERROR");
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("auth")
    );
}

#[tokio::test]
async fn test_get_current_user_authenticated() {
    let app = setup_clean_test_app().await;

    // Generate test credentials
    let username = generate_test_username("authme");
    let email = generate_test_email("authme");
    let password = "TestPassword123!";

    // First send verification code
    let send_code_payload = json!({
        "email": email,
        "verification_type": "email",
        "language": "en"
    });

    let send_code_request = json_request("POST", "/auth/send-code", send_code_payload);
    let send_code_response = app.clone().oneshot(send_code_request).await.unwrap();
    assert_eq!(send_code_response.status(), StatusCode::OK);

    // Register the user with test verification code
    let register_payload = json!({
        "username": username,
        "email": email,
        "password": password,
        "verification_code": "123456"  // Test mode verification code
    });

    let register_request = json_request("POST", "/auth/register", register_payload);
    let register_response = app.clone().oneshot(register_request).await.unwrap();
    let register_body = extract_json_body(register_response).await;

    // Get access token from registration or login
    let access_token = if register_body["code"] == "SUCCESS" {
        register_body["data"]["access_token"]
            .as_str()
            .map(String::from)
    } else {
        // Try login if registration failed
        let login_payload = json!({
            "email": email,
            "password": password
        });

        let login_request = json_request("POST", "/auth/login", login_payload);
        let login_response = app.clone().oneshot(login_request).await.unwrap();
        let login_body = extract_json_body(login_response).await;

        if login_body["code"] == "SUCCESS" {
            login_body["data"]["access_token"]
                .as_str()
                .map(String::from)
        } else {
            None
        }
    };

    // If we have a token, test /api/user/me
    if let Some(token) = access_token {
        // Make authenticated request to /api/user/me
        let me_request = authenticated_request("GET", "/api/user/me", json!({}), &token);
        let me_response = app.oneshot(me_request).await.unwrap();

        assert_eq!(me_response.status(), StatusCode::OK);

        let body = extract_json_body(me_response).await;

        // Verify response structure
        assert_eq!(body["code"], "SUCCESS");
        assert!(body["data"].is_object());

        // Verify user data
        let user_data = &body["data"];
        assert_eq!(user_data["email"], email);
        assert_eq!(user_data["username"], username);
        assert!(user_data["id"].as_i64().is_some());
        assert!(user_data["role"].as_str().is_some());
        assert!(user_data["status"].as_str().is_some());
        assert!(user_data["created_at"].as_str().is_some());
    } else {
        panic!("Failed to get authentication token for testing");
    }
}

#[tokio::test]
async fn test_get_current_user_invalid_token() {
    let app = setup_clean_test_app().await;

    // Make request with invalid token
    let request = Request::builder()
        .method("GET")
        .uri("/api/user/me")
        .header("Content-Type", "application/json")
        .header("Authorization", "Bearer invalid_token_here")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 200 with AUTHENTICATION_ERROR code
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "AUTHENTICATION_ERROR");
}

#[tokio::test]
async fn test_get_current_user_malformed_token() {
    let app = setup_clean_test_app().await;

    // Use a malformed JWT token
    let malformed_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiZXhwIjoxNTE2MjM5MDIyfQ.invalid_signature";

    let request = Request::builder()
        .method("GET")
        .uri("/api/user/me")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", malformed_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 200 with AUTHENTICATION_ERROR code
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "AUTHENTICATION_ERROR");
}

#[tokio::test]
async fn test_get_current_user_data_consistency() {
    let app = setup_clean_test_app().await;

    // Generate unique test credentials
    let username = generate_test_username("consistency");
    let email = generate_test_email("consistency");
    let password = "ConsistentPass123!";

    // Send verification code
    let send_code_payload = json!({
        "email": email,
        "verification_type": "email",
        "language": "en"
    });

    let send_code_request = json_request("POST", "/auth/send-code", send_code_payload);
    app.clone().oneshot(send_code_request).await.unwrap();

    // Register the user
    let register_payload = json!({
        "username": username,
        "email": email,
        "password": password,
        "verification_code": "123456"
    });

    let register_request = json_request("POST", "/auth/register", register_payload);
    let register_response = app.clone().oneshot(register_request).await.unwrap();
    let register_body = extract_json_body(register_response).await;

    // Get token
    let token = if register_body["code"] == "SUCCESS" {
        register_body["data"]["access_token"]
            .as_str()
            .map(String::from)
    } else {
        // Try login
        let login_payload = json!({
            "email": email,
            "password": password
        });

        let login_request = json_request("POST", "/auth/login", login_payload);
        let login_response = app.clone().oneshot(login_request).await.unwrap();
        let login_body = extract_json_body(login_response).await;

        if login_body["code"] == "SUCCESS" {
            login_body["data"]["access_token"]
                .as_str()
                .map(String::from)
        } else {
            None
        }
    };

    if let Some(access_token) = token {
        // Call /api/user/me multiple times and verify consistency
        let mut previous_user_id: Option<i64> = None;

        for i in 0..3 {
            let me_request = authenticated_request("GET", "/api/user/me", json!({}), &access_token);
            let me_response = app.clone().oneshot(me_request).await.unwrap();

            assert_eq!(me_response.status(), StatusCode::OK);

            let body = extract_json_body(me_response).await;

            // Verify response structure
            assert_eq!(body["code"], "SUCCESS");

            let user_data = &body["data"];

            // Verify consistent data
            assert_eq!(user_data["email"], email);
            assert_eq!(user_data["username"], username);

            // Verify ID consistency across calls
            let current_id = user_data["id"].as_i64().unwrap();
            if let Some(prev_id) = previous_user_id {
                assert_eq!(
                    current_id, prev_id,
                    "User ID should be consistent across calls"
                );
            }
            previous_user_id = Some(current_id);

            // Verify required fields are present
            assert!(user_data["role"].as_str().is_some());
            assert!(user_data["status"].as_str().is_some());
            assert!(user_data["created_at"].as_str().is_some());

            println!("Call {} - User data verified successfully", i + 1);
        }
    } else {
        panic!("Failed to get authentication token for consistency test");
    }
}

#[tokio::test]
async fn test_get_current_user_with_different_roles() {
    let app = setup_clean_test_app().await;

    // Test with different user roles
    let test_cases = vec![("scientist_me", "scientist"), ("committee_me", "committee")];

    for (username_prefix, expected_role) in test_cases {
        let username = generate_test_username(username_prefix);
        let email = generate_test_email(username_prefix);
        let password = "RoleTest123!";

        // Send verification code
        let send_code_request = json_request(
            "POST",
            "/auth/send-code",
            json!({
                "email": email,
                "verification_type": "email",
                "language": "en"
            }),
        );
        app.clone().oneshot(send_code_request).await.unwrap();

        // Register with specific role
        let register_payload = json!({
            "username": username,
            "email": email,
            "password": password,
            "role": expected_role,
            "verification_code": "123456"
        });

        let register_request = json_request("POST", "/auth/register", register_payload);
        let register_response = app.clone().oneshot(register_request).await.unwrap();
        let register_body = extract_json_body(register_response).await;

        if register_body["code"] == "SUCCESS" {
            let token = register_body["data"]["access_token"].as_str().unwrap();

            // Test /api/user/me with this user
            let me_request = authenticated_request("GET", "/api/user/me", json!({}), token);
            let me_response = app.clone().oneshot(me_request).await.unwrap();

            assert_eq!(me_response.status(), StatusCode::OK);

            let me_body = extract_json_body(me_response).await;
            assert_eq!(me_body["code"], "SUCCESS");

            let user_data = &me_body["data"];
            assert_eq!(user_data["email"], email);
            assert_eq!(user_data["username"], username);
            assert_eq!(user_data["role"], expected_role);

            println!("Successfully tested /api/user/me for role: {}", expected_role);
        }
    }
}

#[tokio::test]
async fn test_get_current_user_without_bearer_prefix() {
    let app = setup_clean_test_app().await;

    // Make request with token but without "Bearer" prefix
    let request = Request::builder()
        .method("GET")
        .uri("/api/user/me")
        .header("Content-Type", "application/json")
        .header("Authorization", "some_token_without_bearer")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 200 with AUTHENTICATION_ERROR code
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "AUTHENTICATION_ERROR");
}

#[tokio::test]
async fn test_get_current_user_response_fields() {
    let app = setup_clean_test_app().await;

    // Generate test credentials
    let username = generate_test_username("fields");
    let email = generate_test_email("fields");
    let password = "FieldsTest123!";

    // Send verification code
    let send_code_request = json_request(
        "POST",
        "/auth/send-code",
        json!({
            "email": email,
            "verification_type": "email",
            "language": "en"
        }),
    );
    app.clone().oneshot(send_code_request).await.unwrap();

    // Register the user
    let register_request = json_request(
        "POST",
        "/auth/register",
        json!({
            "username": username,
            "email": email,
            "password": password,
            "verification_code": "123456"
        }),
    );
    let register_response = app.clone().oneshot(register_request).await.unwrap();
    let register_body = extract_json_body(register_response).await;

    if register_body["code"] == "SUCCESS" {
        let token = register_body["data"]["access_token"].as_str().unwrap();

        // Test /api/user/me
        let me_request = authenticated_request("GET", "/api/user/me", json!({}), token);
        let me_response = app.oneshot(me_request).await.unwrap();

        assert_eq!(me_response.status(), StatusCode::OK);

        let me_body = extract_json_body(me_response).await;
        assert_eq!(me_body["code"], "SUCCESS");

        // Verify all expected fields are present
        let user_data = &me_body["data"];
        let required_fields = vec!["id", "email", "username", "role", "status", "created_at"];

        for field in required_fields {
            assert!(
                user_data[field] != serde_json::Value::Null,
                "Field '{}' should be present and not null",
                field
            );
        }

        // Verify field types
        assert!(user_data["id"].is_i64(), "id should be a number");
        assert!(user_data["email"].is_string(), "email should be a string");
        assert!(
            user_data["username"].is_string(),
            "username should be a string"
        );
        assert!(user_data["role"].is_string(), "role should be a string");
        assert!(user_data["status"].is_string(), "status should be a string");
        assert!(
            user_data["created_at"].is_string(),
            "created_at should be a string"
        );
    }
}
