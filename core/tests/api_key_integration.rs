//! Integration tests for API key management functionality

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

/// Helper function to register and login a test user, returning the JWT token
async fn setup_test_user(app: &axum::Router) -> (String, String) {
    let username = generate_test_username("apikey_user");
    let email = generate_test_email("apikey");

    // First send verification code
    let send_code_payload = json!({
        "email": email,
        "verification_type": "email",
        "language": "en"
    });
    let request = json_request("POST", "/auth/send-code", send_code_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Register user
    let register_payload = json!({
        "username": username,
        "email": email.clone(),
        "password": "TestPassword123!",
        "verification_code": "1234"
    });

    let request = json_request("POST", "/auth/register", register_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Login to get token
    let login_payload = json!({
        "email": email,
        "password": "TestPassword123!"
    });

    let request = json_request("POST", "/auth/login", login_payload);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    let token = body["data"]["access_token"].as_str().unwrap().to_string();

    (token, email)
}

#[tokio::test]
async fn test_api_key_create_success() {
    let app = setup_clean_test_app().await;
    let (token, _) = setup_test_user(&app).await;

    let create_payload = json!({
        "name": "Test API Key",
        "permissions": ["dataset:read", "dataset:write"],
        "expires_at": null
    });

    let request = authenticated_request("POST", "/api/api-keys", create_payload, &token);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;

    // Debug: Always print response for debugging
    eprintln!(
        "API Key creation response: {}",
        serde_json::to_string_pretty(&body).unwrap()
    );

    assert_eq!(body["code"], "SUCCESS");

    let api_key = &body["data"];
    eprintln!(
        "API Key object: {}",
        serde_json::to_string_pretty(&api_key).unwrap()
    );
    assert_eq!(api_key["name"], "Test API Key");
    assert!(api_key["api_key"].as_str().unwrap().starts_with("dl_"));
    assert_eq!(
        api_key["permissions"],
        json!(["dataset:read", "dataset:write"])
    );
    assert_eq!(api_key["is_active"], true);
}

#[tokio::test]
async fn test_api_key_create_duplicate_name() {
    let app = setup_clean_test_app().await;
    let (token, _) = setup_test_user(&app).await;

    let create_payload = json!({
        "name": "Duplicate Key",
        "permissions": []
    });

    // Create first key
    let request = authenticated_request("POST", "/api/api-keys", create_payload.clone(), &token);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Try to create duplicate
    let request = authenticated_request("POST", "/api/api-keys", create_payload, &token);
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;

    // Debug: Print response if unexpected
    if body["code"] != "VALIDATION_ERROR" {
        eprintln!(
            "Duplicate creation response: {}",
            serde_json::to_string_pretty(&body).unwrap()
        );
    }

    assert_eq!(body["code"], "CONFLICT_ERROR");
    assert!(body["message"].as_str().unwrap().contains("already exists"));
}

#[tokio::test]
async fn test_api_key_list_with_pagination() {
    let app = setup_clean_test_app().await;
    let (token, _) = setup_test_user(&app).await;

    // Create multiple API keys
    for i in 1..=3 {
        let create_payload = json!({
            "name": format!("Key {}", i),
            "permissions": []
        });

        let request = authenticated_request("POST", "/api/api-keys", create_payload, &token);
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // List API keys with pagination
    let request = Request::builder()
        .method("GET")
        .uri("/api/api-keys?page=1&per_page=2")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");

    let data = &body["data"];
    assert_eq!(data["items"].as_array().unwrap().len(), 2);
    assert_eq!(data["total"], 3);
    assert_eq!(data["n_page"], 1);
    assert_eq!(data["per_page"], 2);
}

#[tokio::test]
async fn test_api_key_list_with_filters() {
    let app = setup_clean_test_app().await;
    let (token, _) = setup_test_user(&app).await;

    // Create an active key
    let create_payload = json!({
        "name": "Active Key",
        "permissions": []
    });

    let request = authenticated_request("POST", "/api/api-keys", create_payload, &token);
    let response = app.clone().oneshot(request).await.unwrap();
    let created = extract_json_body(response).await;
    let key_id = created["data"]["id"].as_i64().unwrap();

    // Deactivate the key
    let update_payload = json!({
        "is_active": false
    });

    let request = authenticated_request(
        "PUT",
        &format!("/api/api-keys/{}", key_id),
        update_payload,
        &token,
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Create another active key
    let create_payload = json!({
        "name": "Another Active Key",
        "permissions": []
    });

    let request = authenticated_request("POST", "/api/api-keys", create_payload, &token);
    app.clone().oneshot(request).await.unwrap();

    // Filter by active status
    let request = Request::builder()
        .method("GET")
        .uri("/api/api-keys?is_active=true&page=1&per_page=10")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    println!("Response body for list_with_filters: {:?}", body);

    // Check if the response has the expected structure
    assert_eq!(body["code"], "SUCCESS", "Response code should be SUCCESS");
    assert!(body["data"].is_object(), "Response should have data object");
    assert!(
        body["data"]["items"].is_array(),
        "Data should have items array"
    );

    let items = body["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
}

#[tokio::test]
async fn test_api_key_get_by_id() {
    let app = setup_clean_test_app().await;
    let (token, _) = setup_test_user(&app).await;

    // Create an API key
    let create_payload = json!({
        "name": "Test Key for Get",
        "permissions": ["dataset:read"]
    });

    let request = authenticated_request("POST", "/api/api-keys", create_payload, &token);
    let response = app.clone().oneshot(request).await.unwrap();
    let created = extract_json_body(response).await;
    let key_id = created["data"]["id"].as_i64().unwrap();

    // Get the specific API key
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/api-keys/{}", key_id))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;

    // Debug: Print response if not successful
    if body["code"] != "SUCCESS" {
        eprintln!(
            "Get API key failed: {}",
            serde_json::to_string_pretty(&body).unwrap()
        );
    }

    assert_eq!(body["code"], "SUCCESS");
    assert_eq!(body["data"]["id"], key_id);
    assert_eq!(body["data"]["name"], "Test Key for Get");
}

#[tokio::test]
async fn test_api_key_get_not_found() {
    let app = setup_clean_test_app().await;
    let (token, _) = setup_test_user(&app).await;

    let request = Request::builder()
        .method("GET")
        .uri("/api/api-keys/99999")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "NOT_FOUND_ERROR");
}

#[tokio::test]
async fn test_api_key_update() {
    let app = setup_clean_test_app().await;
    let (token, _) = setup_test_user(&app).await;

    // Create an API key
    let create_payload = json!({
        "name": "Original Name",
        "permissions": ["dataset:read"]
    });

    let request = authenticated_request("POST", "/api/api-keys", create_payload, &token);
    let response = app.clone().oneshot(request).await.unwrap();
    let created = extract_json_body(response).await;
    let key_id = created["data"]["id"].as_i64().unwrap();

    // Update the API key
    let update_payload = json!({
        "name": "Updated Name",
        "permissions": ["dataset:read", "dataset:write"],
        "is_active": false
    });

    let request = authenticated_request(
        "PUT",
        &format!("/api/api-keys/{}", key_id),
        update_payload,
        &token,
    );
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");
    assert_eq!(body["data"]["name"], "Updated Name");
    assert_eq!(
        body["data"]["permissions"],
        json!(["dataset:read", "dataset:write"])
    );
    assert_eq!(body["data"]["is_active"], false);
}

#[tokio::test]
async fn test_api_key_revoke() {
    let app = setup_clean_test_app().await;
    let (token, _) = setup_test_user(&app).await;

    // Create an API key
    let create_payload = json!({
        "name": "Key to Revoke",
        "permissions": []
    });

    let request = authenticated_request("POST", "/api/api-keys", create_payload, &token);
    let response = app.clone().oneshot(request).await.unwrap();
    let created = extract_json_body(response).await;
    let key_id = created["data"]["id"].as_i64().unwrap();

    // Revoke the API key
    let request = Request::builder()
        .method("DELETE")
        .uri(&format!("/api/api-keys/{}", key_id))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "SUCCESS");
    assert_eq!(body["data"]["message"], "API key revoked successfully");

    // Verify the key is deleted
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/api-keys/{}", key_id))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "NOT_FOUND_ERROR");
}

#[tokio::test]
async fn test_api_key_cross_user_isolation() {
    let app = setup_clean_test_app().await;

    // Create first user and their API key
    let (token1, _) = setup_test_user(&app).await;

    let create_payload = json!({
        "name": "User1 Key",
        "permissions": []
    });

    let request = authenticated_request("POST", "/api/api-keys", create_payload, &token1);
    let response = app.clone().oneshot(request).await.unwrap();
    let created = extract_json_body(response).await;
    let key_id = created["data"]["id"].as_i64().unwrap();

    // Create second user
    let (token2, _) = setup_test_user(&app).await;

    // Try to access user1's API key with user2's token
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/api-keys/{}", key_id))
        .header("Authorization", format!("Bearer {}", token2))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "NOT_FOUND_ERROR");

    // Try to update user1's API key with user2's token
    let update_payload = json!({
        "name": "Hacked Name"
    });

    let request = authenticated_request(
        "PUT",
        &format!("/api/api-keys/{}", key_id),
        update_payload,
        &token2,
    );
    let response = app.clone().oneshot(request).await.unwrap();
    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "NOT_FOUND_ERROR");

    // Try to revoke user1's API key with user2's token
    let request = Request::builder()
        .method("DELETE")
        .uri(&format!("/api/api-keys/{}", key_id))
        .header("Authorization", format!("Bearer {}", token2))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    let body = extract_json_body(response).await;
    assert_eq!(body["code"], "NOT_FOUND_ERROR");
}

#[tokio::test]
async fn test_api_key_get_stats() {
    let app = setup_clean_test_app().await;
    let (token, _) = setup_test_user(&app).await;

    // Create multiple API keys with different statuses
    for i in 1..=3 {
        let create_payload = json!({
            "name": format!("Stats Key {}", i),
            "permissions": []
        });

        let request = authenticated_request("POST", "/api/api-keys", create_payload, &token);
        let response = app.clone().oneshot(request).await.unwrap();

        if i == 2 {
            // Deactivate the second key
            let created = extract_json_body(response).await;
            let key_id = created["data"]["id"].as_i64().unwrap();

            let update_payload = json!({
                "is_active": false
            });

            let request = authenticated_request(
                "PUT",
                &format!("/api/api-keys/{}", key_id),
                update_payload,
                &token,
            );
            app.clone().oneshot(request).await.unwrap();
        }
    }

    // Get stats
    let request = Request::builder()
        .method("GET")
        .uri("/api/api-keys/stats")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;

    // Debug: Print the full stats response
    eprintln!(
        "Stats response: {}",
        serde_json::to_string_pretty(&body).unwrap()
    );

    assert_eq!(body["code"], "SUCCESS");

    let stats = &body["data"];

    // Debug: Print individual stat values
    eprintln!("total_keys: {:?}", stats["total_keys"]);
    eprintln!("active_keys: {:?}", stats["active_keys"]);
    eprintln!("inactive_keys: {:?}", stats["inactive_keys"]);

    assert_eq!(stats["total_keys"], 3);
    assert_eq!(stats["active_keys"], 2);
    assert_eq!(stats["inactive_keys"], 1);
}

#[tokio::test]
async fn test_api_key_rate_limit_tier_filter() {
    let app = setup_clean_test_app().await;
    let (token, _) = setup_test_user(&app).await;

    // Create API keys (all will have default "basic" tier)
    for i in 1..=3 {
        let create_payload = json!({
            "name": format!("Tier Key {}", i),
            "permissions": []
        });

        let request = authenticated_request("POST", "/api/api-keys", create_payload, &token);
        app.clone().oneshot(request).await.unwrap();
    }

    // Filter by basic tier
    let request = Request::builder()
        .method("GET")
        .uri("/api/api-keys?rate_limit_tier=basic&page=1&per_page=10")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = extract_json_body(response).await;
    let items = body["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);

    // All should be basic tier
    for item in items {
        assert_eq!(item["rate_limit_tier"], "basic");
    }

    // Filter by non-existent tier
    let request = Request::builder()
        .method("GET")
        .uri("/api/api-keys?rate_limit_tier=Premium&page=1&per_page=10")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    let body = extract_json_body(response).await;
    let items = body["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 0);
}
