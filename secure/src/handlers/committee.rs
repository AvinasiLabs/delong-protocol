use axum::{
    extract::{Path, Query, State},
    Json,
    Extension,
};
use std::sync::Arc;
use tracing::info;

use common::{
    ApiResult, 
    ApiResponse, 
    AuthContext,
    models::{
        committee::{CommitteeMemberData, SetCommitteeMemberRequest, CommitteeStatsResponse},
        pagination::{PaginatedResponse, PaginationParams},
    }
};
use crate::AppState;
use crate::services::committee::{CommitteeService, CommitteeMember};
use crate::middleware::jwt::require_admin;

// Helper to convert service model to common model
fn to_committee_member_data(member: CommitteeMember) -> CommitteeMemberData {
    CommitteeMemberData {
        id: member.id as u64,
        member_wallet: member.wallet_address,
        is_approved: member.is_active,
        created_at: member.created_at.to_rfc3339(),
        updated_at: member.updated_at.to_rfc3339(),
    }
}

/// List all committee members
pub async fn list_committee_members(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> ApiResult<Json<ApiResponse<PaginatedResponse<CommitteeMemberData>>>> {
    info!(page = %params.page, limit = %params.limit, "Listing committee members");

    let paginated_result = CommitteeService::get_committee_members(&state.db_pool, params).await?;
    
    let items = paginated_result.items.into_iter().map(to_committee_member_data).collect();

    let response = PaginatedResponse::new(
        items,
        paginated_result.page,
        paginated_result.limit,
        paginated_result.total_items,
    );
    
    Ok(Json(ApiResponse::success(response)))
}

/// Get a specific committee member by wallet address
pub async fn get_committee_member(
    State(state): State<Arc<AppState>>,
    Path(wallet_address): Path<String>,
) -> ApiResult<Json<ApiResponse<Option<CommitteeMemberData>>>> {
    info!(wallet = %wallet_address, "Getting committee member");

    let member_option = CommitteeService::get_committee_member_by_wallet(&state.db_pool, &wallet_address).await?;

    Ok(Json(ApiResponse::success(member_option.map(to_committee_member_data))))
}

/// Add or update a committee member (requires admin privileges)
pub async fn upsert_committee_member(
    State(state): State<Arc<AppState>>,
    Extension(auth_context): Extension<AuthContext>,
    Json(request): Json<SetCommitteeMemberRequest>,
) -> ApiResult<Json<ApiResponse<CommitteeMemberData>>> {
    info!(wallet = %request.member_wallet, "Adding/updating committee member");

    require_admin(&auth_context)?;

    let service_req = crate::services::committee::SetCommitteeMemberRequest {
        wallet_address: request.member_wallet,
        is_active: request.is_approved,
    };

    let member = CommitteeService::set_committee_member(&state.db_pool, service_req).await?;
    
    Ok(Json(ApiResponse::success(to_committee_member_data(member))))
}

/// Get committee statistics
pub async fn get_committee_stats(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ApiResponse<CommitteeStatsResponse>>> {
    info!("Getting committee statistics");

    let active_count = CommitteeService::get_active_member_count(&state.db_pool).await?;
    let all_members = CommitteeService::get_committee_members(
        &state.db_pool, 
        PaginationParams { page: 1, limit: 10000 } // A bit of a hack to get all members
    ).await?;
    let total_count = all_members.total_items;

    let stats = CommitteeStatsResponse {
        total_members: total_count as i64,
        active_members: active_count,
        inactive_members: total_count as i64 - active_count,
    };

    Ok(Json(ApiResponse::success(stats)))
} 