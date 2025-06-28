//! Algorithm execution management handlers
//!
//! This module contains handlers for algorithm execution lifecycle management,
//! including submission, status tracking, and result retrieval.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{
    handlers::{ApiResponse, PaginatedResponse, PaginationParams},
    routes::AppState,
    utils::http_client::forward_post,
};

/// Request payload for submitting algorithm execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgoExeSubmissionRequest {
    pub github_repo: String,
    pub commit_hash: String,
    pub scientist_wallet: String,
    pub dataset: String,
}

/// Algorithm execution data model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgoExeData {
    pub id: u64,
    pub algo_id: String,
    pub used_dataset: String,
    pub scientist_wallet: String,
    pub review_status: String,
    pub vote_start_time: Option<String>,
    pub vote_end_time: Option<String>,
    pub status: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub result: Option<String>,
    pub error_msg: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub algo_name: Option<String>,
    pub algo_link: Option<String>,
    pub cid: Option<String>,
}

/// Response for algorithm execution submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgoExeSubmissionResponse {
    pub id: u64,
}

/// Handler for submitting algorithm execution
///
/// POST /api/algo-exes
/// Forwards the request to the Core service for algorithm execution submission
pub async fn submit_algo_exe_handler(
    State(state): State<AppState>,
    Json(payload): Json<AlgoExeSubmissionRequest>,
) -> Result<Json<ApiResponse<AlgoExeSubmissionResponse>>, StatusCode> {
    info!(
        "Submitting algorithm execution: repo={}, commit={}, scientist={}",
        payload.github_repo, payload.commit_hash, payload.scientist_wallet
    );

    // Forward request to Core service
    let core_url = format!("{}/api/algo-exes", state.config.services.core_url);

    match forward_post::<AlgoExeSubmissionRequest, AlgoExeSubmissionResponse>(
        state.http_client.as_ref(),
        &core_url,
        payload,
        None,
    )
    .await
    {
        Ok(response) => {
            info!(
                "Algorithm execution submitted successfully: id={}",
                response.id
            );
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to submit algorithm execution: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handler for getting algorithm execution list
///
/// GET /api/algo-exes
/// Returns paginated list of algorithm executions
pub async fn get_algo_exes_handler(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ApiResponse<PaginatedResponse<AlgoExeData>>>, StatusCode> {
    info!(
        "Getting algorithm executions list: page={}, limit={}",
        params.page, params.limit
    );

    // Forward request to Core service
    let core_url = format!(
        "{}/api/algo-exes?page={}&limit={}",
        state.config.services.core_url, params.page, params.limit
    );

    match crate::utils::http_client::forward_get::<PaginatedResponse<AlgoExeData>>(
        state.http_client.as_ref(),
        &core_url,
        None,
    )
    .await
    {
        Ok(response) => {
            info!(
                "Retrieved {} algorithm executions (page {}/{})",
                response.items.len(),
                response.page,
                response.total_pages
            );
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to get algorithm executions: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handler for getting algorithm execution details
///
/// GET /api/algo-exes/{id}
/// Returns detailed information about a specific algorithm execution
pub async fn get_algo_exe_handler(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<ApiResponse<AlgoExeData>>, StatusCode> {
    info!("Getting algorithm execution details: id={}", id);

    // Forward request to Core service
    let core_url = format!("{}/api/algo-exes/{}", state.config.services.core_url, id);

    match crate::utils::http_client::forward_get::<AlgoExeData>(
        state.http_client.as_ref(),
        &core_url,
        None,
    )
    .await
    {
        Ok(response) => {
            info!(
                "Retrieved algorithm execution: id={}, status={}",
                response.id, response.status
            );
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to get algorithm execution {}: {}", id, e);
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
    fn test_algo_exe_submission_request_serialization() {
        let request = AlgoExeSubmissionRequest {
            github_repo: "https://github.com/user/repo".to_string(),
            commit_hash: "abc123def456".to_string(),
            scientist_wallet: "0x1234567890abcdef".to_string(),
            dataset: "dataset_001".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("github_repo"));
        assert!(json.contains("commit_hash"));
        assert!(json.contains("scientist_wallet"));
        assert!(json.contains("dataset"));
    }

    #[test]
    fn test_algo_exe_data_deserialization() {
        let json = r#"
        {
            "id": 1,
            "algo_id": "algo_123",
            "used_dataset": "dataset_001",
            "scientist_wallet": "0x1234567890abcdef",
            "review_status": "pending",
            "vote_start_time": null,
            "vote_end_time": null,
            "status": "running",
            "start_time": "2023-01-01T00:00:00Z",
            "end_time": null,
            "result": null,
            "error_msg": null,
            "created_at": "2023-01-01T00:00:00Z",
            "updated_at": "2023-01-01T00:00:00Z",
            "algo_name": "Test Algorithm",
            "algo_link": "https://github.com/user/repo",
            "cid": "QmTest123"
        }
        "#;

        let data: AlgoExeData = serde_json::from_str(json).unwrap();
        assert_eq!(data.id, 1);
        assert_eq!(data.algo_id, "algo_123");
        assert_eq!(data.status, "running");
        assert_eq!(data.algo_name, Some("Test Algorithm".to_string()));
    }

    #[test]
    fn test_pagination_params_default() {
        let params = PaginationParams::default();
        assert_eq!(params.page, 1);
        assert_eq!(params.limit, 20);
    }

    #[test]
    fn test_api_response_success() {
        let response = AlgoExeSubmissionResponse { id: 123 };
        let api_response = ApiResponse::success(response);

        assert!(api_response.success);
        assert!(api_response.data.is_some());
        assert_eq!(api_response.data.unwrap().id, 123);
    }
}
