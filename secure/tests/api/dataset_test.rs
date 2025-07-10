//! Tests for static dataset endpoints

use axum::http::StatusCode;
use common::{
    models::{PaginatedResponse, StaticDatasetInfo},
    ApiResponse, ResponseCode,
};

use super::helpers::{generate_user_token, make_auth_request, setup_test_environment};

#[tokio::test]
async fn test_static_datasets_list_empty() {
    let router = setup_test_environment().await;
    let user_token = generate_user_token("test_user");

    let (status, _, body_json) = make_auth_request::<()>(
        &router,
        "GET",
        "/api/static-datasets",
        None,
        &user_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);

    let response: ApiResponse<PaginatedResponse<StaticDatasetInfo>> =
        serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
    let paginated_data = response.data.unwrap();
    assert_eq!(paginated_data.total_items, 0);
    assert_eq!(paginated_data.items.len(), 0);

    println!("✅ Static datasets empty list test passed");
}

#[tokio::test]
async fn test_static_datasets_list_with_pagination() {
    let router = setup_test_environment().await;
    let user_token = generate_user_token("test_user");

    // This test would require pre-populating the database with datasets
    // For now, we just test the endpoint with pagination params
    let (status, _, body_json) = make_auth_request(
        &router,
        "GET",
        "/api/static-datasets?page=2&per_page=10",
        None::<()>,
        &user_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<PaginatedResponse<StaticDatasetInfo>> =
        serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);

    println!("✅ Static datasets pagination test passed");
}

#[tokio::test]
async fn test_static_dataset_get_not_found() {
    let router = setup_test_environment().await;
    let user_token = generate_user_token("test_user");

    // Use a valid but non-existent ID format (e.g., a number)
    let (status, _, body_json) = make_auth_request::<()>(
        &router,
        "GET",
        "/api/static-datasets/99999",
        None,
        &user_token,
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let response: ApiResponse<()> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::NotFound);

    println!("✅ Static dataset not found test passed");
} 