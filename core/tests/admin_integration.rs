//! Integration tests for admin functionality

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

/// Helper function to create an admin user and get token
async fn create_admin_user(app: &axum::Router) -> String {
    let username = generate_test_username("admin");
    let email = generate_test_email("admin");
    let password = "AdminPassword123!";

    // First send verification code
    let send_code_payload = json!({
        "email": email,
        "verification_type": "email",
        "language": "en"
    });
    let request = json_request("POST", "/auth/send-code", send_code_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Register the user
    let register_payload = json!({
        "username": username,
        "email": email,
        "password": password,
        "verification_code": "1234"
    });
    let request = json_request("POST", "/auth/register", register_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    let user_id = body["data"]["user"]["id"].as_i64().unwrap() as i32;

    // Update user to admin role directly in database
    // Get database connection from test environment
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:test_password@localhost:11021/db_core_test".to_string()
    });
    let db = sqlx::PgPool::connect(&db_url).await.unwrap();

    sqlx::query!("UPDATE users SET role = $1 WHERE id = $2", "admin", user_id)
        .execute(&db)
        .await
        .unwrap();

    // Login to get fresh token with admin role
    let login_payload = json!({
        "email": email,
        "password": password
    });
    let request = json_request("POST", "/auth/login", login_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    body["data"]["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn test_admin_list_users() {
    let app = setup_clean_test_app().await;

    // Create admin user
    let admin_token = create_admin_user(&app).await;

    // Create some test users
    for i in 0..3 {
        let username = generate_test_username(&format!("user{}", i));
        let email = generate_test_email(&format!("user{}", i));

        // Send verification code
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
            "password": "TestUser123!",
            "verification_code": "1234"
        });
        let request = json_request("POST", "/auth/register", register_payload);
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // List users with admin token
    let request = Request::builder()
        .method("GET")
        .uri("/admin/users")
        .header("Authorization", format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");

    // Should have at least 4 users (1 admin + 3 test users)
    let users = body["data"]["users"].as_array().unwrap();
    assert!(users.len() >= 4);
}

#[tokio::test]
async fn test_admin_get_user_by_id() {
    let app = setup_clean_test_app().await;

    // Create an admin user
    let admin_token = create_admin_user(&app).await;

    // Create a test user to fetch
    let username = generate_test_username("getuser");
    let email = generate_test_email("getuser");

    // Send verification code
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
        "password": "TestUser123!",
        "verification_code": "1234"
    });

    let request = json_request("POST", "/auth/register", register_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    let user_id = body["data"]["user"]["id"].as_i64().unwrap();

    // Now get the user by ID using admin token
    let request = Request::builder()
        .method("GET")
        .uri(format!("/admin/users/{}", user_id))
        .header("Authorization", format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Debug: Print response status
    eprintln!("Get user by ID response status: {:?}", response.status());
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;

    // Debug: Print response body
    eprintln!(
        "Get user by ID response body: {}",
        serde_json::to_string_pretty(&body).unwrap()
    );

    assert_eq!(body["code"], "SUCCESS");
    assert_eq!(body["data"]["id"], user_id);
    assert_eq!(body["data"]["username"], username);
    assert_eq!(body["data"]["email"], email);
}

#[tokio::test]
async fn test_admin_search_users() {
    let app = setup_clean_test_app().await;

    // Create admin user
    let admin_token = create_admin_user(&app).await;

    // Create test users with searchable usernames/emails
    for i in 0..3 {
        let username = generate_test_username(&format!("search{}", i));
        let email = generate_test_email(&format!("search{}", i));

        // Send verification code
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
            "password": "TestUser123!",
            "verification_code": "1234"
        });
        let request = json_request("POST", "/auth/register", register_payload);
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // Search for users with admin token
    let request = Request::builder()
        .method("GET")
        .uri("/admin/users?search=search")
        .header("Authorization", format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");

    let users = body["data"]["users"].as_array().unwrap();
    assert_eq!(
        users.len(),
        3,
        "Should find 3 users with 'search' in username"
    );
}

#[tokio::test]
async fn test_admin_update_user_role() {
    let app = setup_clean_test_app().await;

    // Create admin user
    let admin_token = create_admin_user(&app).await;

    // Create a test user to update
    let username = generate_test_username("roletest");
    let email = generate_test_email("roletest");

    // Send verification code
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
        "password": "TestUser123!",
        "verification_code": "1234"
    });
    let request = json_request("POST", "/auth/register", register_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    let user_id = body["data"]["user"]["id"].as_i64().unwrap();

    // Update user role to committee
    let update_payload = json!({
        "role": "committee"
    });

    let request = authenticated_request(
        "PUT",
        &format!("/admin/users/{}", user_id),
        update_payload,
        &admin_token,
    );

    let response = app.oneshot(request).await.unwrap();

    // Debug: Print response status
    eprintln!("Update user role response status: {:?}", response.status());
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;

    // Debug: Print response body
    eprintln!(
        "Update user role response body: {}",
        serde_json::to_string_pretty(&body).unwrap()
    );

    assert_eq!(body["code"], "SUCCESS");
    assert_eq!(body["data"]["role"], "committee");
}

#[tokio::test]
async fn test_admin_update_user_status() {
    let app = setup_clean_test_app().await;

    // Create admin user
    let admin_token = create_admin_user(&app).await;

    // Create a test user to update
    let username = generate_test_username("statustest");
    let email = generate_test_email("statustest");

    // Send verification code
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
        "password": "TestUser123!",
        "verification_code": "1234"
    });
    let request = json_request("POST", "/auth/register", register_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    let user_id = body["data"]["user"]["id"].as_i64().unwrap();

    // Update user status to suspended
    let update_payload = json!({
        "status": "suspended"
    });

    let request = authenticated_request(
        "PUT",
        &format!("/admin/users/{}", user_id),
        update_payload,
        &admin_token,
    );

    let response = app.oneshot(request).await.unwrap();

    // Debug: Print response status
    eprintln!(
        "Update user status response status: {:?}",
        response.status()
    );
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;

    // Debug: Print response body
    eprintln!(
        "Update user status response body: {}",
        serde_json::to_string_pretty(&body).unwrap()
    );

    assert_eq!(body["code"], "SUCCESS");
    assert_eq!(body["data"]["status"], "suspended");
}

#[tokio::test]
async fn test_admin_permissions() {
    let app = setup_clean_test_app().await;

    // Create a regular user (not admin)
    let username = generate_test_username("regular");
    let email = generate_test_email("regular");

    // Send verification code
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
        "password": "RegularUser123!",
        "verification_code": "1234"
    });

    let request = json_request("POST", "/auth/register", register_payload);
    let response = app.clone().oneshot(request).await.unwrap();

    let body = extract_json_body(response).await;
    let regular_token = body["data"]["access_token"].as_str().unwrap();

    // Try to access admin roles endpoint with regular user token
    let request = Request::builder()
        .method("GET")
        .uri("/admin/roles")
        .header("Authorization", format!("Bearer {}", regular_token))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Regular user should be denied access to admin routes
    assert_eq!(response.status(), StatusCode::OK); // All responses return 200 according to STANDARD

    let body = extract_json_body(response).await;

    // Should return authorization error
    assert!(
        body["code"] == "AUTHORIZATION_ERROR" || body["code"] == "AUTHENTICATION_ERROR",
        "Expected authorization/authentication error for regular user accessing admin route"
    );

    // Now test with an admin user
    let admin_token = create_admin_user(&app).await;

    // Try to access admin roles endpoint with admin token
    let request = Request::builder()
        .method("GET")
        .uri("/admin/roles")
        .header("Authorization", format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Admin should have access
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");

    // Check that roles data is returned
    assert!(body["data"]["roles"].is_array());
    let roles = body["data"]["roles"].as_array().unwrap();
    assert!(roles.len() > 0);
    // Should contain at least "scientist" role
    assert!(
        roles
            .iter()
            .any(|r| r["name"].as_str() == Some("scientist")),
        "Should contain scientist role"
    );
}
