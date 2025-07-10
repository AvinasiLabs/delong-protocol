//! Tests for committee management endpoints

use axum::http::StatusCode;
use common::{
    models::{
        committee::{CommitteeMemberData, SetCommitteeMemberRequest},
        PaginatedResponse,
    },
    ApiResponse, ResponseCode,
};

use super::helpers::{generate_admin_token, make_auth_request, setup_test_environment};

const TEST_WALLET: &str = "0x1234567890123456789012345678901234567890";

#[tokio::test]
async fn test_committee_lifecycle() {
    let router = setup_test_environment().await;
    let admin_token = generate_admin_token("admin");

    // 1. List initial members (should be empty)
    let (status, _, body_json) =
        make_auth_request::<()>(&router, "GET", "/api/committee", None, &admin_token).await;
    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<PaginatedResponse<CommitteeMemberData>> =
        serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
    let paginated_data = response.data.unwrap();
    assert_eq!(paginated_data.total_items, 0);

    // 2. Add a new committee member (as not approved)
    let new_member_request = SetCommitteeMemberRequest {
        member_wallet: TEST_WALLET.to_string(),
        is_approved: false,
    };
    let (status, _, body_json) = make_auth_request(
        &router,
        "POST",
        "/api/committee",
        Some(new_member_request),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<CommitteeMemberData> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
    let member = response.data.unwrap();
    assert_eq!(member.member_wallet, TEST_WALLET);
    assert!(!member.is_approved);

    // 3. Get the newly added member
    let (status, _, body_json) = make_auth_request::<()>(
        &router,
        "GET",
        &format!("/api/committee/{}", TEST_WALLET),
        None,
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<Option<CommitteeMemberData>> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
    let member = response.data.unwrap().unwrap();
    assert_eq!(member.member_wallet, TEST_WALLET);
    assert!(!member.is_approved);

    // 4. List members again (should have one)
    let (status, _, body_json) =
        make_auth_request::<()>(&router, "GET", "/api/committee", None, &admin_token).await;
    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<PaginatedResponse<CommitteeMemberData>> =
        serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
    let paginated_data = response.data.unwrap();
    assert_eq!(paginated_data.total_items, 1);
    assert_eq!(paginated_data.items[0].member_wallet, TEST_WALLET);

    // 5. Update the member to be approved
    let update_member_request = SetCommitteeMemberRequest {
        member_wallet: TEST_WALLET.to_string(),
        is_approved: true,
    };
    let (status, _, body_json) = make_auth_request(
        &router,
        "POST",
        "/api/committee",
        Some(update_member_request),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<CommitteeMemberData> = serde_json::from_value(body_json).unwrap();
    let member = response.data.unwrap();
    assert_eq!(member.member_wallet, TEST_WALLET);
    assert!(member.is_approved);

    println!("✅ Committee lifecycle test passed");
}

#[tokio::test]
async fn test_get_committee_member_not_found() {
    let router = setup_test_environment().await;
    let admin_token = generate_admin_token("admin");

    let non_existent_wallet = "0x0000000000000000000000000000000000000000";
    let (status, _, body_json) = make_auth_request::<()>(
        &router,
        "GET",
        &format!("/api/committee/{}", non_existent_wallet),
        None,
        &admin_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<Option<CommitteeMemberData>> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
    // The endpoint should return a successful response with `data` being `Some(None)` 
    // when the member is not found.
    assert!(response.data.is_some() && response.data.unwrap().is_none());

    println!("✅ Get non-existent committee member test passed");
}
