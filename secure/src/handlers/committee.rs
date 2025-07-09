use axum::{
    extract::{Path, Query, State},
    Json,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use common::{ApiResponse, AuthContext};
use crate::AppState;
use crate::services::CommitteeService;
use crate::middleware::jwt::require_admin;

/// Query parameters for listing committee members
#[derive(Debug, Deserialize)]
pub struct ListCommitteeQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub active_only: Option<bool>,
}

/// Request to add/update committee member
#[derive(Debug, Deserialize, Serialize)]
pub struct CommitteeMemberRequest {
    pub wallet_address: String,
    pub is_active: bool,
}

/// Response for committee member
#[derive(Debug, Serialize)]
pub struct CommitteeMemberResponse {
    pub id: i32,
    pub wallet_address: String,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// List all committee members
pub async fn list_committee_members(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListCommitteeQuery>,
) -> Json<ApiResponse<Vec<CommitteeMemberResponse>>> {
    info!("Listing committee members");

    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20);
    let active_only = query.active_only.unwrap_or(false);

    let members_result = if active_only {
        CommitteeService::get_active_members(&state.db_pool, page as i32, page_size as i32).await
    } else {
        CommitteeService::get_all_members(&state.db_pool, page as i32, page_size as i32).await
    };

    match members_result {
        Ok(members) => {
            let response_members: Vec<CommitteeMemberResponse> = members.into_iter().map(|member| {
                CommitteeMemberResponse {
                    id: member.id as i32,
                    wallet_address: member.wallet_address,
                    is_active: member.is_active,
                    created_at: member.created_at,
                    updated_at: member.updated_at,
                }
            }).collect();

            Json(ApiResponse::success(response_members))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list committee members");
            Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to list committee members".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
}

/// Get a specific committee member by wallet address
pub async fn get_committee_member(
    State(state): State<Arc<AppState>>,
    Path(wallet_address): Path<String>,
) -> Json<ApiResponse<CommitteeMemberResponse>> {
    info!(wallet = %wallet_address, "Getting committee member");

    match CommitteeService::get_member_by_wallet(&state.db_pool, &wallet_address).await {
        Ok(member) => {
            let response = CommitteeMemberResponse {
                id: member.id as i32,
                wallet_address: member.wallet_address,
                is_active: member.is_active,
                created_at: member.created_at,
                updated_at: member.updated_at,
            };
            Json(ApiResponse::success(response))
        }
        Err(e) => {
            tracing::error!(error = %e, wallet = %wallet_address, "Failed to get committee member");
            Json(ApiResponse {
                code: common::ResponseCode::NotFound,
                data: None,
                message: "Committee member not found".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
}

/// Add or update a committee member (requires admin privileges)
pub async fn upsert_committee_member(
    State(state): State<Arc<AppState>>,
    Extension(auth_context): Extension<AuthContext>,
    Json(request): Json<CommitteeMemberRequest>,
) -> Json<ApiResponse<CommitteeMemberResponse>> {
    info!(wallet = %request.wallet_address, "Adding/updating committee member");

    // Check admin privileges
    if let Err(_e) = require_admin(&auth_context) {
        tracing::warn!(
            user_id = %auth_context.user_id,
            role = %auth_context.role,
            "Unauthorized attempt to modify committee member"
        );
        return Json(ApiResponse {
            code: common::ResponseCode::Forbidden,
            data: None,
            message: "Admin privileges required".to_string(),
            request_id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    match CommitteeService::set_member(
        &state.db_pool,
        &request.wallet_address,
        request.is_active
    ).await {
        Ok(member) => {
            let response = CommitteeMemberResponse {
                id: member.id as i32,
                wallet_address: member.wallet_address,
                is_active: member.is_active,
                created_at: member.created_at,
                updated_at: member.updated_at,
            };
            Json(ApiResponse::success(response))
        }
        Err(e) => {
            tracing::error!(error = %e, wallet = %request.wallet_address, "Failed to upsert committee member");
            Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to add/update committee member".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
}

/// Get committee statistics
pub async fn get_committee_stats(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<CommitteeStatsResponse>> {
    info!("Getting committee statistics");

    match CommitteeService::get_active_member_count(&state.db_pool).await {
        Ok(active_count) => {
            match CommitteeService::get_all_members(&state.db_pool, 1, 1000).await {
                Ok(all_members) => {
                    let total_count = all_members.len() as i32;
                    let stats = CommitteeStatsResponse {
                        total_members: total_count,
                        active_members: active_count as i32,
                        inactive_members: total_count - (active_count as i32),
                    };
                    Json(ApiResponse::success(stats))
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to get all members");
                    Json(ApiResponse {
                        code: common::ResponseCode::InternalServerError,
                        data: None,
                        message: "Failed to get committee statistics".to_string(),
                        request_id: None,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    })
                }
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to get active member count");
            Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to get committee statistics".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
}

/// Committee statistics response
#[derive(Debug, Serialize)]
pub struct CommitteeStatsResponse {
    pub total_members: i32,
    pub active_members: i32,
    pub inactive_members: i32,
} 