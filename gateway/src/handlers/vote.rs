//! Voting management handlers
//!
//! This module contains handlers for voting operations,
//! including listing votes and setting vote duration.

use axum::{
    extract::{Query, State},
    response::Json,
};
use tracing::{error, info};

use crate::{
    handlers::{ApiResponse, PaginatedResponse, PaginationParams},
    routes::AppState,
    services::http_client::{forward_get, forward_post},
};

use common::prelude::{SetVoteDurationRequest, VoteData, VoteDurationResponse};

/// Handler for getting vote list
///
/// GET /api/votes
/// Returns paginated list of votes
#[utoipa::path(
    get,
    path = "/api/votes",
    tag = "votes",
    summary = "List votes",
    description = "Retrieve a paginated list of votes with their current status",
    params(
        ("page" = Option<u32>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u32>, Query, description = "Items per page (default: 20, max: 100)")
    ),
    responses(
        (status = 200, description = "Votes retrieved successfully", body = ApiResponse<PaginatedResponse<VoteData>>),
        (status = 400, description = "Invalid query parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_votes_handler(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Json<ApiResponse<PaginatedResponse<VoteData>>> {
    info!(
        "Getting votes list: page={}, limit={}",
        params.page, params.limit
    );

    // Forward request to Secure service
    let secure_url = format!(
        "{}/api/votes?page={}&limit={}",
        state.config.services.secure_url, params.page, params.limit
    );

    match forward_get::<PaginatedResponse<VoteData>>(state.http_client.as_ref(), &secure_url, None)
        .await
    {
        Ok(response) => {
            info!(
                "Retrieved {} votes (page {}/{})",
                response.items.len(),
                response.page,
                response.total_pages
            );
            Json(ApiResponse::success(response))
        }
        Err(e) => {
            error!("Failed to get votes: {:?}", e);
            Json(ApiResponse::internal_error())
        }
    }
}

/// Handler for setting vote duration
///
/// POST /api/votes/duration
/// Sets the duration for voting periods
#[utoipa::path(
    post,
    path = "/api/votes/duration",
    tag = "votes",
    summary = "Set vote duration",
    description = "Configure the duration for voting periods in seconds",
    request_body = SetVoteDurationRequest,
    responses(
        (status = 200, description = "Vote duration set successfully", body = ApiResponse<VoteDurationResponse>),
        (status = 400, description = "Invalid request parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 403, description = "Forbidden - insufficient permissions", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn set_vote_duration_handler(
    State(state): State<AppState>,
    Json(payload): Json<SetVoteDurationRequest>,
) -> Json<ApiResponse<VoteDurationResponse>> {
    info!("Setting vote duration: {} seconds", payload.duration);

    // Forward request to Secure service
    let secure_url = format!("{}/api/votes/duration", state.config.services.secure_url);

    match forward_post::<SetVoteDurationRequest, VoteDurationResponse>(
        state.http_client.as_ref(),
        &secure_url,
        payload,
        None,
    )
    .await
    {
        Ok(response) => {
            info!(
                "Vote duration set successfully: {} seconds",
                response.duration
            );
            Json(ApiResponse::success(response))
        }
        Err(e) => {
            error!("Failed to set vote duration: {:?}", e);
            Json(ApiResponse::internal_error())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GatewayConfig, ServicesConfig};
    use crate::handlers::ResponseCode;

    #[allow(dead_code)]
    fn create_test_config() -> GatewayConfig {
        let mut config = GatewayConfig::default();
        config.services = ServicesConfig {
            core_url: "http://localhost:8001".to_string(),
            secure_url: "http://localhost:8002".to_string(),
        };
        config
    }

    #[test]
    fn test_set_vote_duration_request_serialization() {
        let request = SetVoteDurationRequest {
            duration: 3600, // 1 hour
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("duration"));
        assert!(json.contains("3600"));
    }

    #[test]
    fn test_vote_data_deserialization() {
        let json = r#"
        {
            "id": 1,
            "algo_cid": "QmTest123",
            "voter": "0x1234567890abcdef",
            "approve": true,
            "voted_at": "2023-01-01T12:00:00Z",
            "created_at": "2023-01-01T00:00:00Z",
            "updated_at": "2023-01-01T00:00:00Z"
        }
        "#;

        let data: VoteData = serde_json::from_str(json).unwrap();
        assert_eq!(data.id, 1);
        assert_eq!(data.algo_cid, "QmTest123");
        assert_eq!(data.voter, "0x1234567890abcdef");
        assert!(data.approve);
    }

    #[test]
    fn test_vote_duration_response() {
        let response = VoteDurationResponse { duration: 7200 };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("duration"));
        assert!(json.contains("7200"));
    }

    #[test]
    fn test_api_response_with_vote_duration() {
        let response = VoteDurationResponse { duration: 1800 };
        let api_response = ApiResponse::success(response);

        assert_eq!(api_response.code, ResponseCode::Success);
        assert!(api_response.data.is_some());
        assert_eq!(api_response.data.unwrap().duration, 1800);
    }

    #[test]
    fn test_pagination_with_vote_data() {
        let votes = vec![
            VoteData {
                id: 1,
                algo_cid: "QmTest123".to_string(),
                voter: "0x1111111111111111".to_string(),
                approve: true,
                voted_at: "2023-01-01T12:00:00Z".to_string(),
                created_at: "2023-01-01T00:00:00Z".to_string(),
                updated_at: "2023-01-01T00:00:00Z".to_string(),
            },
            VoteData {
                id: 2,
                algo_cid: "QmTest456".to_string(),
                voter: "0x2222222222222222".to_string(),
                approve: false,
                voted_at: "2023-01-01T13:00:00Z".to_string(),
                created_at: "2023-01-01T01:00:00Z".to_string(),
                updated_at: "2023-01-01T01:00:00Z".to_string(),
            },
        ];

        let paginated = PaginatedResponse::new(votes, 1, 20, 50);
        assert_eq!(paginated.items.len(), 2);
        assert_eq!(paginated.total_items, 50);
        assert_eq!(paginated.total_pages, 3);
    }

    #[test]
    fn test_pagination_params_for_votes() {
        let params = PaginationParams::default();
        assert_eq!(params.page, 1);
        assert_eq!(params.limit, 20);

        // Test custom pagination
        let json = r#"{"page": 2, "limit": 50}"#;
        let custom_params: PaginationParams = serde_json::from_str(json).unwrap();
        assert_eq!(custom_params.page, 2);
        assert_eq!(custom_params.limit, 50);
    }
}
