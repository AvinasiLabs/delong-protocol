//! Committee management handlers
//!
//! This module contains handlers for committee member management,
//! including adding members, listing members, and checking membership status.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use tracing::{error, info};

use crate::{
    handlers::{ApiResponse, PaginatedResponse, PaginationParams},
    routes::AppState,
    services::http_client::{forward_get, forward_post},
};

use core::{
    CommitteeMemberData, CommitteeMemberResponse, MembershipCheckResponse,
    SetCommitteeMemberRequest,
};

/// Handler for setting committee member
///
/// POST /api/committee
/// Forwards the request to the Secure service (TEE Hardware) to add or update committee member
#[utoipa::path(
    post,
    path = "/api/committee",
    tag = "committee",
    summary = "Set committee member",
    description = "Add or update a committee member's status and approval",
    request_body = SetCommitteeMemberRequest,
    responses(
        (status = 200, description = "Committee member set successfully", body = ApiResponse<CommitteeMemberResponse>),
        (status = 400, description = "Invalid request parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 403, description = "Forbidden - insufficient permissions", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn set_committee_member_handler(
    State(state): State<AppState>,
    Json(payload): Json<SetCommitteeMemberRequest>,
) -> Result<Json<ApiResponse<CommitteeMemberResponse>>, StatusCode> {
    info!(
        "Setting committee member: wallet={}, approved={}",
        payload.member_wallet, payload.is_approved
    );

    // Forward request to Secure service
    let secure_url = format!("{}/api/committee", state.config.services.secure_url);

    match forward_post::<SetCommitteeMemberRequest, CommitteeMemberResponse>(
        state.http_client.as_ref(),
        &secure_url,
        payload,
        None,
    )
    .await
    {
        Ok(response) => {
            info!("Committee member set successfully: id={}", response.id);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to set committee member: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handler for getting committee members list
///
/// GET /api/committee
/// Returns paginated list of committee members
#[utoipa::path(
    get,
    path = "/api/committee",
    tag = "committee",
    summary = "List committee members",
    description = "Retrieve a paginated list of committee members",
    params(
        ("page" = Option<u32>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u32>, Query, description = "Items per page (default: 20, max: 100)")
    ),
    responses(
        (status = 200, description = "Committee members retrieved successfully", body = ApiResponse<PaginatedResponse<CommitteeMemberData>>),
        (status = 400, description = "Invalid query parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_committee_members_handler(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ApiResponse<PaginatedResponse<CommitteeMemberData>>>, StatusCode> {
    info!(
        "Getting committee members list: page={}, limit={}",
        params.page, params.limit
    );

    // Forward request to Secure service
    let secure_url = format!(
        "{}/api/committee?page={}&limit={}",
        state.config.services.secure_url, params.page, params.limit
    );

    match forward_get::<PaginatedResponse<CommitteeMemberData>>(
        state.http_client.as_ref(),
        &secure_url,
        None,
    )
    .await
    {
        Ok(response) => {
            info!(
                "Retrieved {} committee members (page {}/{})",
                response.items.len(),
                response.page,
                response.total_pages
            );
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to get committee members: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handler for getting committee member details
///
/// GET /api/committee/{id}
/// Returns detailed information about a specific committee member
#[utoipa::path(
    get,
    path = "/api/committee/{id}",
    tag = "committee",
    summary = "Get committee member",
    description = "Retrieve detailed information about a specific committee member",
    params(
        ("id" = u64, Path, description = "Committee member ID")
    ),
    responses(
        (status = 200, description = "Committee member retrieved successfully", body = ApiResponse<CommitteeMemberData>),
        (status = 400, description = "Invalid committee member ID", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 404, description = "Committee member not found", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_committee_member_handler(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<ApiResponse<CommitteeMemberData>>, StatusCode> {
    info!("Getting committee member details: id={}", id);

    // Forward request to Secure service
    let secure_url = format!("{}/api/committee/{}", state.config.services.secure_url, id);

    match forward_get::<CommitteeMemberData>(state.http_client.as_ref(), &secure_url, None).await {
        Ok(response) => {
            info!(
                "Retrieved committee member: id={}, wallet={}, approved={}",
                response.id, response.member_wallet, response.is_approved
            );
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to get committee member {}: {}", id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handler for checking committee membership
///
/// GET /api/committee/check/{wallet}
/// Returns membership status for a given wallet address
#[utoipa::path(
    get,
    path = "/api/committee/check/{wallet}",
    tag = "committee",
    summary = "Check committee membership",
    description = "Check if a wallet address is a committee member and their approval status",
    params(
        ("wallet" = String, Path, description = "Wallet address to check")
    ),
    responses(
        (status = 200, description = "Membership check completed successfully", body = ApiResponse<MembershipCheckResponse>),
        (status = 400, description = "Invalid wallet address", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn check_committee_membership_handler(
    State(state): State<AppState>,
    Path(wallet): Path<String>,
) -> Result<Json<ApiResponse<MembershipCheckResponse>>, StatusCode> {
    info!("Checking committee membership: wallet={}", wallet);

    // Forward request to Secure service
    let secure_url = format!(
        "{}/api/committee/check/{}",
        state.config.services.secure_url, wallet
    );

    match forward_get::<MembershipCheckResponse>(state.http_client.as_ref(), &secure_url, None)
        .await
    {
        Ok(response) => {
            info!(
                "Committee membership check completed: wallet={}, is_member={}",
                wallet, response.is_member
            );
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to check committee membership for {}: {}", wallet, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
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
    fn test_set_committee_member_request_serialization() {
        let request = SetCommitteeMemberRequest {
            member_wallet: "0x1234567890abcdef".to_string(),
            is_approved: true,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("member_wallet"));
        assert!(json.contains("is_approved"));
        assert!(json.contains("true"));
    }

    #[test]
    fn test_committee_member_data_deserialization() {
        let json = r#"
        {
            "id": 1,
            "member_wallet": "0x1234567890abcdef",
            "is_approved": true,
            "created_at": "2023-01-01T00:00:00Z",
            "updated_at": "2023-01-01T00:00:00Z"
        }
        "#;

        let data: CommitteeMemberData = serde_json::from_str(json).unwrap();
        assert_eq!(data.id, 1);
        assert_eq!(data.member_wallet, "0x1234567890abcdef");
        assert!(data.is_approved);
    }

    #[test]
    fn test_membership_check_response() {
        let response = MembershipCheckResponse { is_member: true };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("is_member"));
        assert!(json.contains("true"));
    }

    #[test]
    fn test_committee_member_response() {
        let response = CommitteeMemberResponse { id: 42 };
        let api_response = ApiResponse::success(response);

        assert_eq!(api_response.code, ResponseCode::Success);
        assert!(api_response.data.is_some());
        assert_eq!(api_response.data.unwrap().id, 42);
    }

    #[test]
    fn test_pagination_with_committee_data() {
        let members = vec![
            CommitteeMemberData {
                id: 1,
                member_wallet: "0x1111111111111111".to_string(),
                is_approved: true,
                created_at: "2023-01-01T00:00:00Z".to_string(),
                updated_at: "2023-01-01T00:00:00Z".to_string(),
            },
            CommitteeMemberData {
                id: 2,
                member_wallet: "0x2222222222222222".to_string(),
                is_approved: false,
                created_at: "2023-01-02T00:00:00Z".to_string(),
                updated_at: "2023-01-02T00:00:00Z".to_string(),
            },
        ];

        let paginated = PaginatedResponse::new(members, 1, 20, 100);
        assert_eq!(paginated.items.len(), 2);
        assert_eq!(paginated.total_items, 100);
        assert_eq!(paginated.total_pages, 5);
    }
}
