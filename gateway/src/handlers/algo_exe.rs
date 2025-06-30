//! Algorithm execution management handlers
//!
//! This module contains handlers for algorithm execution lifecycle management,
//! including submission, status tracking, and result retrieval.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use tracing::{error, info};

use crate::{handlers::ApiResponse, routes::AppState, utils::http_client::forward_post};
use common::{
    AlgoExeData, AlgoExeSubmissionRequest, AlgoExeSubmissionResponse, PaginatedResponse,
    PaginationParams,
};

/// Handler for submitting algorithm execution
///
/// POST /api/algo-exes
/// Forwards the request to the Secure service (TEE Hardware) for algorithm execution submission
pub async fn submit_algo_exe_handler(
    State(state): State<AppState>,
    Json(payload): Json<AlgoExeSubmissionRequest>,
) -> Result<Json<ApiResponse<AlgoExeSubmissionResponse>>, StatusCode> {
    info!(
        "Submitting algorithm execution: repo={}, commit={}, scientist={}",
        payload.github_repo, payload.commit_hash, payload.scientist_wallet
    );

    // Forward request to Secure service
    let secure_url = format!("{}/api/algo-exes", state.config.services.secure_url);

    match forward_post::<AlgoExeSubmissionRequest, AlgoExeSubmissionResponse>(
        state.http_client.as_ref(),
        &secure_url,
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

    // Forward request to Secure service
    let secure_url = format!(
        "{}/api/algo-exes?page={}&limit={}",
        state.config.services.secure_url, params.page, params.limit
    );

    match crate::utils::http_client::forward_get::<PaginatedResponse<AlgoExeData>>(
        state.http_client.as_ref(),
        &secure_url,
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

    // Forward request to Secure service
    let secure_url = format!("{}/api/algo-exes/{}", state.config.services.secure_url, id);

    match crate::utils::http_client::forward_get::<AlgoExeData>(
        state.http_client.as_ref(),
        &secure_url,
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
    fn test_api_response_success() {
        let response = AlgoExeSubmissionResponse { id: 123 };
        let api_response = ApiResponse::success(response);

        assert_eq!(api_response.code, ResponseCode::Success);
        assert!(api_response.data.is_some());
        assert_eq!(api_response.data.unwrap().id, 123);
    }
}
