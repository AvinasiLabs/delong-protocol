//! Tests for JWT authentication middleware

use axum::http::StatusCode;
use common::{
    models::{
        auth::{Claims, JwtUtils},
        dataset::{DatasetPaginatedResponse, StaticDatasetInfo},
    },
    ApiResponse, ResponseCode,
};

use super::helpers::{
    generate_admin_token, generate_custom_token, generate_user_token, make_auth_request,
    make_json_request, setup_test_environment,
};

#[tokio::test]
async fn test_jwt_authentication_missing_token() {
    let router = setup_test_environment().await;

    // A request without token to an authenticated endpoint should fail
    let (status, _, body_json) =
        make_json_request::<()>(&router, "GET", "/api/static-datasets", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let response: ApiResponse<()> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Unauthorized);

    println!("✅ JWT missing token test passed");
}

#[tokio::test]
async fn test_jwt_authentication_invalid_token() {
    let router = setup_test_environment().await;

    // A request with an invalid token should fail
    let (status, _, body_json) =
        make_auth_request::<()>(&router, "GET", "/api/static-datasets", None, "invalid.jwt.token")
            .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let response: ApiResponse<()> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Unauthorized);

    println!("✅ JWT invalid token test passed");
}

#[tokio::test]
async fn test_jwt_authentication_expired_token() {
    let router = setup_test_environment().await;
    let expired_token = generate_custom_token("user", "test_user", -3600); // Expired 1 hour ago

    let (status, _, body_json) =
        make_auth_request::<()>(&router, "GET", "/api/static-datasets", None, &expired_token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let response: ApiResponse<()> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Unauthorized);

    println!("✅ JWT expired token test passed");
}

#[tokio::test]
async fn test_jwt_authentication_valid_admin_token() {
    let router = setup_test_environment().await;
    let token = generate_admin_token("admin_user");

    // This endpoint requires auth but should succeed with a valid token
    let (status, _, _) = make_auth_request::<DatasetPaginatedResponse<StaticDatasetInfo>>(
        &router,
        "GET",
        "/api/static-datasets",
        None,
        &token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);

    println!("✅ JWT valid admin token test passed");
}

#[tokio::test]
async fn test_jwt_authentication_valid_user_token() {
    let router = setup_test_environment().await;
    let token = generate_user_token("regular_user");

    let (status, _, _) = make_auth_request::<DatasetPaginatedResponse<StaticDatasetInfo>>(
        &router,
        "GET",
        "/api/static-datasets",
        None,
        &token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);

    println!("✅ JWT valid user token test passed");
} 