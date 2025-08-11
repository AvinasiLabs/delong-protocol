use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

mod common;
use common::{generate_test_wallet_address, setup_test_app};

#[tokio::test]
async fn test_set_committee_member_add() {
    let app = setup_test_app().await;

    let request_body = json!({
        "member_wallet": generate_test_wallet_address(1),
        "is_approved": true
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/committee")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Debug output to see what's in the response
    println!(
        "Response JSON: {}",
        serde_json::to_string_pretty(&json).unwrap()
    );

    assert_eq!(json["code"], "SUCCESS");
    assert!(json["data"]["tx_hash"].is_string());
}

#[tokio::test]
async fn test_set_committee_member_remove() {
    let app = setup_test_app().await;

    let request_body = json!({
        "member_wallet": generate_test_wallet_address(2),
        "is_approved": false
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/committee")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["code"], "SUCCESS");
    assert!(json["data"]["tx_hash"].is_string());
}

#[tokio::test]
async fn test_set_committee_member_invalid_wallet() {
    let app = setup_test_app().await;

    let request_body = json!({
        "member_wallet": "invalid-wallet-address",
        "is_approved": true
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/committee")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["code"], "VALIDATION_ERROR");
    assert!(json["message"]
        .as_str()
        .unwrap()
        .contains("Invalid Ethereum address"));
}

#[tokio::test]
async fn test_list_committee_members() {
    let app = setup_test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/api/committee?page=1&per_page=10")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Debug output to see what's in the response
    println!(
        "Response JSON: {}",
        serde_json::to_string_pretty(&json).unwrap()
    );

    assert_eq!(json["code"], "SUCCESS");
    assert!(json["data"]["items"].is_array());
    assert!(json["data"]["total"].is_number());
    assert_eq!(json["data"]["n_page"], 1);
    assert_eq!(json["data"]["per_page"], 10);
}

#[tokio::test]
async fn test_get_committee_member() {
    let app = setup_test_app().await;

    // Test with non-existent member
    let request = Request::builder()
        .method("GET")
        .uri("/api/committee/999999")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["code"], "NOT_FOUND_ERROR");
    assert!(json["message"]
        .as_str()
        .unwrap()
        .contains("Committee member not found"));
}

#[tokio::test]
async fn test_is_committee_member() {
    let app = setup_test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/api/committee/is-member?member_wallet=0x70997970C51812dc3A010C7d01b50e0d17dc79C8")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["code"], "SUCCESS");
    assert!(json["data"].is_boolean());
}

#[tokio::test]
async fn test_is_committee_member_missing_wallet() {
    let app = setup_test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/api/committee/is-member")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Check the actual status code
    eprintln!("Actual status code: {}", response.status());

    if response.status() == StatusCode::OK {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        eprintln!(
            "Response body: {}",
            serde_json::to_string_pretty(&json).unwrap()
        );

        // Check if it's a validation/parsing error
        assert!(json["code"] == "VALIDATION_ERROR" || json["code"] == "PARSING_ERROR");
        assert!(json["message"].as_str().unwrap().contains("member_wallet"));
    } else {
        // Original assertion
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
