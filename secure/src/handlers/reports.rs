use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use common::ApiResponse;
use crate::AppState;
use crate::services::{CommitteeService, VoteService, blockchain_sync::BlockchainSyncService};

/// Query parameters for reports
#[derive(Debug, Deserialize)]
pub struct ReportQuery {
    pub from_date: Option<String>, // ISO 8601 format
    pub to_date: Option<String>,   // ISO 8601 format
    pub execution_id: Option<i32>,
}

/// Committee activity report
#[derive(Debug, Serialize)]
pub struct CommitteeActivityReport {
    pub total_members: i32,
    pub active_members: i32,
    pub period_votes_cast: i32,
    pub period_approvals: i32,
    pub period_rejections: i32,
    pub member_stats: Vec<MemberActivityStats>,
}

/// Individual member activity statistics
#[derive(Debug, Serialize)]
pub struct MemberActivityStats {
    pub wallet_address: String,
    pub name: Option<String>,
    pub is_active: bool,
    pub votes_cast: i32,
    pub approvals: i32,
    pub rejections: i32,
    pub participation_rate: f64, // Percentage of votes they participated in
}

/// Voting report
#[derive(Debug, Serialize)]
pub struct VotingReport {
    pub total_executions: i32,
    pub completed_votes: i32,
    pub pending_votes: i32,
    pub approved_executions: i32,
    pub rejected_executions: i32,
    pub average_voting_time_hours: f64,
    pub execution_details: Vec<ExecutionVotingDetail>,
}

/// Execution voting detail
#[derive(Debug, Serialize)]
pub struct ExecutionVotingDetail {
    pub execution_id: i32,
    pub total_votes: i32,
    pub approve_votes: i32,
    pub reject_votes: i32,
    pub is_complete: bool,
    pub is_approved: bool,
    pub voting_start_time: Option<chrono::DateTime<chrono::Utc>>,
    pub voting_end_time: Option<chrono::DateTime<chrono::Utc>>,
}

/// System activity report
#[derive(Debug, Serialize)]
pub struct SystemActivityReport {
    pub blockchain_transactions: i32,
    pub pending_transactions: i32,
    pub confirmed_transactions: i32,
    pub algorithm_executions: i32,
    pub dataset_registrations: i32,
    pub committee_changes: i32,
    pub votes_cast: i32,
}

/// Generate committee activity report
pub async fn get_committee_activity_report(
    State(state): State<Arc<AppState>>,
    Query(_query): Query<ReportQuery>,
) -> Json<ApiResponse<CommitteeActivityReport>> {
    info!("Generating committee activity report");

    // Get committee statistics
    let all_members = match CommitteeService::get_all_members(&state.db_pool, 1, 1000).await {
        Ok(members) => members,
        Err(e) => {
            tracing::error!(error = %e, "Failed to get all members");
            return Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to get committee members".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    };
    
    let active_count = match CommitteeService::get_active_member_count(&state.db_pool).await {
        Ok(count) => count as i32,
        Err(e) => {
            tracing::error!(error = %e, "Failed to get active member count");
            return Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to get active member count".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    };

    // Calculate member activity stats
    let mut member_stats = Vec::new();
    let total_possible_votes = 10; // Placeholder - would calculate from executions in period

    for member in &all_members {
        let member_votes = match VoteService::get_votes_by_voter(&state.db_pool, &member.wallet_address, 1, 1000).await {
            Ok(votes) => votes,
            Err(e) => {
                tracing::error!(error = %e, wallet = %member.wallet_address, "Failed to get member votes");
                continue; // Skip this member and continue with others
            }
        };
        
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
            name: None, // Name field not available in CommitteeMember
            is_active: member.is_active,
            votes_cast,
            approvals,
            rejections,
            participation_rate,
        });
    }

    // Calculate period totals
    let period_votes_cast: i32 = member_stats.iter().map(|m| m.votes_cast).sum();
    let period_approvals: i32 = member_stats.iter().map(|m| m.approvals).sum();
    let period_rejections: i32 = member_stats.iter().map(|m| m.rejections).sum();

    let report = CommitteeActivityReport {
        total_members: all_members.len() as i32,
        active_members: active_count,
        period_votes_cast,
        period_approvals,
        period_rejections,
        member_stats,
    };

    Json(ApiResponse::success(report))
}

/// Generate voting report
pub async fn get_voting_report(
    State(state): State<Arc<AppState>>,
    Query(_query): Query<ReportQuery>,
) -> Json<ApiResponse<VotingReport>> {
    info!("Generating voting report");

    // For this demo, we'll simulate execution data
    // In production, this would query actual execution records
    let mock_executions = vec![1, 2, 3, 4, 5]; // Mock execution IDs
    let mut execution_details = Vec::new();
    let mut completed_votes = 0;
    let mut approved_executions = 0;

    for execution_id in &mock_executions {
                    let tally = VoteService::get_vote_tally(&state.db_pool, *execution_id).await.unwrap_or_default();
            let is_complete = VoteService::is_voting_complete(&state.db_pool, *execution_id).await.unwrap_or(false);
            let is_approved = if is_complete {
                VoteService::is_execution_approved(&state.db_pool, *execution_id).await.unwrap_or(false)
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
            voting_start_time: None, // Would get from execution start time
            voting_end_time: None,   // Would get from last vote time
        });
    }

    let report = VotingReport {
        total_executions: mock_executions.len() as i32,
        completed_votes,
        pending_votes: mock_executions.len() as i32 - completed_votes,
        approved_executions,
        rejected_executions: completed_votes - approved_executions,
        average_voting_time_hours: 24.5, // Placeholder calculation
        execution_details,
    };

    Json(ApiResponse::success(report))
}

/// Generate system activity report
pub async fn get_system_activity_report(
    State(_state): State<Arc<AppState>>,
    Query(_query): Query<ReportQuery>,
) -> Json<ApiResponse<SystemActivityReport>> {
    info!("Generating system activity report");

    let blockchain_sync = match BlockchainSyncService::new().await {
        Ok(sync) => sync,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create blockchain sync service");
            return Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to initialize blockchain service".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    };
    // Get blockchain transaction stats
    let transactions = match blockchain_sync.list_transactions(Some(1000)).await {
        Ok(txs) => txs,
        Err(e) => {
            tracing::error!(error = %e, "Failed to list transactions");
            return Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to get transaction statistics".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    };
    let pending_transactions = transactions.iter().filter(|tx| tx.status.as_deref() == Some("PENDING")).count() as i32;
    let confirmed_transactions = transactions.iter().filter(|tx| tx.status.as_deref() == Some("CONFIRMED")).count() as i32;

    // Count different transaction types
    let algorithm_executions = transactions.iter()
        .filter(|tx| tx.entity_type.as_deref() == Some("algorithm"))
        .count() as i32;
    let dataset_registrations = transactions.iter()
        .filter(|tx| tx.entity_type.as_deref() == Some("dataset"))
        .count() as i32;

    // Get voting stats - simplified for demo
    let votes_cast = 50; // Placeholder - would count all votes in period
    let committee_changes = 5; // Placeholder - would count committee updates

    let report = SystemActivityReport {
        blockchain_transactions: transactions.len() as i32,
        pending_transactions,
        confirmed_transactions,
        algorithm_executions,
        dataset_registrations,
        committee_changes,
        votes_cast,
    };

    Json(ApiResponse::success(report))
}

/// Get governance summary report
pub async fn get_governance_summary(
    State(state): State<Arc<AppState>>,
    Query(_query): Query<ReportQuery>,
) -> Json<ApiResponse<GovernanceSummary>> {
    info!("Generating governance summary report");


    let blockchain_sync = match BlockchainSyncService::new().await {
        Ok(sync) => sync,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create blockchain sync service");
            return Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to initialize blockchain service".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    };

    // Get basic stats
    let active_members = match CommitteeService::get_active_member_count(&state.db_pool).await {
        Ok(count) => count as i32,
        Err(e) => {
            tracing::error!(error = %e, "Failed to get active member count");
            return Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to get active member count".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    };
    let all_members = match CommitteeService::get_all_members(&state.db_pool, 1, 1000).await {
        Ok(members) => members,
        Err(e) => {
            tracing::error!(error = %e, "Failed to get all members");
            return Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to get all members".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    };
    let sync_status = match blockchain_sync.get_sync_status().await {
        Ok(status) => status,
        Err(e) => {
            tracing::error!(error = %e, "Failed to get sync status");
            return Json(ApiResponse {
                code: common::ResponseCode::InternalServerError,
                data: None,
                message: "Failed to get sync status".to_string(),
                request_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    };

    let summary = GovernanceSummary {
        committee_size: active_members,
        total_registered_members: all_members.len() as i32,
        blockchain_sync_status: if sync_status.is_connected { "Connected".to_string() } else { "Disconnected".to_string() },
        blocks_behind: sync_status.blocks_behind,
        pending_votes: 3, // Placeholder
        recent_approvals: 7, // Placeholder
        recent_rejections: 2, // Placeholder
        system_health: "Healthy".to_string(), // Placeholder
    };

    Json(ApiResponse::success(summary))
}

/// Governance summary response
#[derive(Debug, Serialize)]
pub struct GovernanceSummary {
    pub committee_size: i32,
    pub total_registered_members: i32,
    pub blockchain_sync_status: String,
    pub blocks_behind: u64,
    pub pending_votes: i32,
    pub recent_approvals: i32,
    pub recent_rejections: i32,
    pub system_health: String,
} 