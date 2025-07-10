use axum::{
    extract::{Query, State},
    Json,
};
use std::sync::Arc;
use tracing::info;

use common::{
    ApiResult,
    ApiResponse,
    models::report::{
        ActivityReportQuery, CommitteeActivityReport, MemberActivityStats,
        VotingReport, ExecutionVotingDetail, SystemActivityReport, GovernanceSummary,
    },
};
use crate::AppState;
use crate::services::{committee::CommitteeService, vote::VoteService};

/// Generate committee activity report
pub async fn get_committee_activity_report(
    State(state): State<Arc<AppState>>,
    _query: Query<ActivityReportQuery>,
) -> ApiResult<Json<ApiResponse<CommitteeActivityReport>>> {
    info!("Generating committee activity report");

    let all_members = CommitteeService::get_all_members(&state.db_pool, 1, 1000).await?;
    let active_count = CommitteeService::get_active_member_count(&state.db_pool).await?;

    let mut member_stats = Vec::new();
    let total_possible_votes = 10; 

    for member in &all_members {
        let member_votes = VoteService::get_votes_by_voter(&state.db_pool, &member.wallet_address, 1, 1000).await?;
        
        let votes_cast = member_votes.len() as i32;
        let approvals = member_votes.iter().filter(|v| v.decision == "APPROVE").count() as i32;
        let rejections = member_votes.iter().filter(|v| v.decision == "REJECT").count() as i32;
        let participation_rate = if total_possible_votes > 0 {
            (votes_cast as f64 / total_possible_votes as f64) * 100.0
        } else {
            0.0
        };

        member_stats.push(MemberActivityStats {
            wallet_address: member.wallet_address.clone(),
            name: None, 
            is_active: member.is_active,
            votes_cast,
            approvals,
            rejections,
            participation_rate,
        });
    }

    let report = CommitteeActivityReport {
        total_members: all_members.len() as i32,
        active_members: active_count as i32,
        period_votes_cast: member_stats.iter().map(|m| m.votes_cast).sum(),
        period_approvals: member_stats.iter().map(|m| m.approvals).sum(),
        period_rejections: member_stats.iter().map(|m| m.rejections).sum(),
        member_stats,
    };

    Ok(Json(ApiResponse::success(report)))
}

/// Generate voting report
pub async fn get_voting_report(
    State(state): State<Arc<AppState>>,
    _query: Query<ActivityReportQuery>,
) -> ApiResult<Json<ApiResponse<VotingReport>>> {
    info!("Generating voting report");

    let mock_executions = vec![1, 2, 3, 4, 5]; 
    let mut execution_details = Vec::new();
    let mut completed_votes = 0;
    let mut approved_executions = 0;

    for execution_id in &mock_executions {
        let tally = VoteService::get_vote_tally(&state.db_pool, *execution_id).await?;
        let is_complete = VoteService::is_voting_complete(&state.db_pool, *execution_id).await?;
        let is_approved = if is_complete {
            VoteService::is_execution_approved(&state.db_pool, *execution_id).await?
        } else {
            false
        };

        if is_complete {
            completed_votes += 1;
        }
        if is_approved {
            approved_executions += 1;
        }

        execution_details.push(ExecutionVotingDetail {
            execution_id: *execution_id as i32,
            total_votes: tally.total_votes as i32,
            approve_votes: tally.approve_votes as i32,
            reject_votes: tally.reject_votes as i32,
            is_complete,
            is_approved,
            voting_start_time: None,
            voting_end_time: None,
        });
    }

    let report = VotingReport {
        total_executions: mock_executions.len() as i32,
        completed_votes,
        pending_votes: mock_executions.len() as i32 - completed_votes,
        approved_executions,
        rejected_executions: completed_votes - approved_executions,
        average_voting_time_hours: 24.5,
        execution_details,
    };

    Ok(Json(ApiResponse::success(report)))
}

/// Generate system activity report
pub async fn get_system_activity_report(
    State(state): State<Arc<AppState>>,
    _query: Query<ActivityReportQuery>,
) -> ApiResult<Json<ApiResponse<SystemActivityReport>>> {
    info!("Generating system activity report");

    let transactions = state.blockchain_sync_service.list_transactions(Some(1000)).await?;
    let mut pending_transactions = 0;
    let mut confirmed_transactions = 0;
    for tx in &transactions {
        if tx.status.as_deref() == Some("PENDING") {
            pending_transactions += 1;
        } else if tx.status.as_deref() == Some("CONFIRMED") {
            confirmed_transactions += 1;
        }
    }
    
    // These are mock values for now
    let report = SystemActivityReport {
        blockchain_transactions: transactions.len() as i32,
        pending_transactions: pending_transactions as i32,
        confirmed_transactions: confirmed_transactions as i32,
        algorithm_executions: 5,
        dataset_registrations: 2,
        committee_changes: 3,
        votes_cast: 15,
    };

    Ok(Json(ApiResponse::success(report)))
}

/// Generate governance summary report
pub async fn get_governance_summary(
    State(state): State<Arc<AppState>>,
    _query: Query<ActivityReportQuery>,
) -> ApiResult<Json<ApiResponse<GovernanceSummary>>> {
    info!("Generating governance summary report");

    let active_committee_count = CommitteeService::get_active_member_count(&state.db_pool).await?;
    let all_members = CommitteeService::get_all_members(&state.db_pool, 1, 1000).await?;
    let sync_status = state.blockchain_sync_service.get_sync_status().await?;
    
    // Mock data for other fields
    let report = GovernanceSummary {
        committee_size: active_committee_count as i32,
        total_registered_members: all_members.len() as i32,
        blockchain_sync_status: if sync_status.is_connected { "connected" } else { "disconnected" }.to_string(),
        blocks_behind: sync_status.blocks_behind,
        pending_votes: 5, 
        recent_approvals: 3,
        recent_rejections: 1,
        system_health: "OK".to_string(),
    };

    Ok(Json(ApiResponse::success(report)))
} 