//! Tests for voting endpoints

use axum::http::StatusCode;
use common::{
    models::vote::{CastVoteRequest, VoteData, VoteDecision, VoteSummary},
    ApiResponse, ResponseCode,
};

use super::helpers::{generate_user_token, make_auth_request, setup_test_environment};

#[tokio::test]
async fn test_cast_vote_on_nonexistent_execution() {
    let router = setup_test_environment().await;
    let user_token = generate_user_token("committee_member_1");
    let execution_id = 99999; // Non-existent

    let vote_request = CastVoteRequest {
        algo_cid: "some_cid".to_string(),
        decision: VoteDecision::Approve,
        comment: Some("Looks good".to_string()),
        signature: None,
    };

    let (status, _, body_json) = make_auth_request(
        &router,
        "POST",
        &format!("/api/votes/{}", execution_id),
        Some(vote_request),
        &user_token,
    )
    .await;

    // The service should return a 404 when the execution ID does not exist.
    assert_eq!(status, StatusCode::NOT_FOUND);
    let response: ApiResponse<()> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::NotFound);
}

#[tokio::test]
async fn test_get_votes_for_execution() {
    let router = setup_test_environment().await;
    let user_token = generate_user_token("user1");
    let execution_id = 99999; // Non-existent, should return empty list

    let (status, _, body_json) = make_auth_request::<()>(
        &router,
        "GET",
        &format!("/api/votes/{}/votes", execution_id),
        None,
        &user_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    // The handler returns a Vec<VoteData>, not paginated.
    let response: ApiResponse<Vec<VoteData>> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
    assert!(response.data.unwrap().is_empty());
}

#[tokio::test]
async fn test_get_vote_tally() {
    let router = setup_test_environment().await;
    let user_token = generate_user_token("user1");
    let execution_id = 99999; // Non-existent

    let (status, _, body_json) = make_auth_request::<()>(
        &router,
        "GET",
        &format!("/api/votes/{}/tally", execution_id),
        None,
        &user_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<VoteSummary> = serde_json::from_value(body_json).unwrap();
    assert_eq!(response.code, ResponseCode::Success);
    let summary = response.data.unwrap();
    assert_eq!(summary.total_votes, 0);
    assert_eq!(summary.approve_votes, 0);
    assert_eq!(summary.reject_votes, 0);
}
