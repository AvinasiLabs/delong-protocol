//! Tests for report generation endpoints

use axum::http::StatusCode;
use common::{
    models::report::{
        CommitteeActivityReport, GovernanceSummary, SystemActivityReport, VotingReport,
    },
    ApiResponse, PaginatedResponse, ResponseCode,
};

use super::helpers::{generate_admin_token, make_auth_request, setup_test_environment};

#[tokio::test]
async fn test_get_committee_activity_report() {
    let router = setup_test_environment().await;
    let admin_token = generate_admin_token("admin");

    let (status, _, body_json) = make_auth_request::<()>(
        &router,
        "GET",
        "/api/reports/committee-activity",
        None,
        &admin_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<PaginatedResponse<CommitteeActivityReport>> =
        serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
}

#[tokio::test]
async fn test_get_voting_report() {
    let router = setup_test_environment().await;
    let admin_token = generate_admin_token("admin");

    let (status, _, body_json) =
        make_auth_request::<()>(&router, "GET", "/api/reports/voting", None, &admin_token).await;

    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<VotingReport> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
    assert!(response.data.is_some());
}

#[tokio::test]
async fn test_get_system_activity_report() {
    let router = setup_test_environment().await;
    let admin_token = generate_admin_token("admin");

    let (status, _, body_json) = make_auth_request::<()>(
        &router,
        "GET",
        "/api/reports/system-activity",
        None,
        &admin_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<SystemActivityReport> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
    assert!(response.data.is_some());
}

#[tokio::test]
async fn test_get_governance_summary() {
    let router = setup_test_environment().await;
    let admin_token = generate_admin_token("admin");

    let (status, _, body_json) = make_auth_request::<()>(
        &router,
        "GET",
        "/api/reports/governance-summary",
        None,
        &admin_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<GovernanceSummary> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
    assert!(response.data.is_some());
}
