//! Voting management handlers
//!
//! This module contains handlers for voting operations,
//! including listing votes and setting vote duration.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{
    handlers::{ApiResponse, PaginatedResponse, PaginationParams},
    routes::AppState,
    utils::http_client::{forward_get, forward_post},
};

/// Request payload for setting vote duration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetVoteDurationRequest {
    pub duration: u64, // Duration in seconds
}

/// Vote data model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteData {
    pub id: u64,
    pub algo_cid: String,
    pub voter: String,
    pub approve: bool,
    pub voted_at: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Response for vote duration setting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteDurationResponse {
    pub duration: u64,
}

/// Handler for getting vote list
///
/// GET /api/votes
/// Returns paginated list of votes
pub async fn get_votes_handler(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ApiResponse<PaginatedResponse<VoteData>>>, StatusCode> {
    info!(
        "Getting votes list: page={}, limit={}",
        params.page, params.limit
    );

    // Forward request to Core service
    let core_url = format!(
        "{}/api/votes?page={}&limit={}",
        state.config.services.core_url, params.page, params.limit
    );

    match forward_get::<PaginatedResponse<VoteData>>(state.http_client.as_ref(), &core_url, None)
        .await
    {
        Ok(response) => {
            info!(
                "Retrieved {} votes (page {}/{})",
                response.items.len(),
                response.page,
                response.total_pages
            );
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to get votes: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handler for setting vote duration
///
/// POST /api/votes/duration
/// Sets the duration for voting periods
pub async fn set_vote_duration_handler(
    State(state): State<AppState>,
    Json(payload): Json<SetVoteDurationRequest>,
) -> Result<Json<ApiResponse<VoteDurationResponse>>, StatusCode> {
    info!("Setting vote duration: {} seconds", payload.duration);

    // Forward request to Core service
    let core_url = format!("{}/api/votes/duration", state.config.services.core_url);

    match forward_post::<SetVoteDurationRequest, VoteDurationResponse>(
        state.http_client.as_ref(),
        &core_url,
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
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to set vote duration: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GatewayConfig, ServicesConfig};

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

        assert!(api_response.success);
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

        let paginated = PaginatedResponse::new(votes, 50, 1, 20);
        assert_eq!(paginated.items.len(), 2);
        assert_eq!(paginated.total, 50);
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
