//! Algorithm execution management handlers
//!
//! This module contains handlers for algorithm execution lifecycle management,
//! including submission, status tracking, and result retrieval.

use axum::{
    extract::{Path, Query, State},
    response::Json,
};

use tracing::{error, info, instrument};

use crate::{
    handlers::ApiResponse,
    routes::AppState,
    services::http_client::{forward_get, forward_post},
};
use common::prelude::*;

/// Handler for submitting algorithm execution
///
/// POST /api/algo-exes
/// Forwards the request to the Secure service (TEE Hardware) for algorithm execution submission
#[utoipa::path(
    post,
    path = "/api/algo-exes",
    tag = "algo-exe",
    summary = "Submit algorithm execution",
    description = "Submit a new algorithm for execution in the TEE environment",
    request_body = AlgoExeSubmissionRequest,
    responses(
        (status = 200, description = "Algorithm execution submitted successfully", body = ApiResponse<AlgoExeSubmissionResponse>),
        (status = 400, description = "Invalid request parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 403, description = "Forbidden - insufficient permissions", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
#[instrument(skip(state, payload), fields(request_id))]
pub async fn submit_algo_exe_handler(
    State(state): State<AppState>,
    Json(payload): Json<AlgoExeSubmissionRequest>,
) -> Json<ApiResponse<AlgoExeSubmissionResponse>> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);
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
            Json(ApiResponse::success(response))
        }
        Err(e) => {
            error!(
                request_id = request_id,
                error = ?e,
                "Failed to submit algorithm execution"
            );
            Json(ApiResponse {
                code: ResponseCode::InternalServerError,
                data: None,
                request_id: Some(request_id),
            })
        }
    }
}

/// Handler for getting algorithm execution list
///
/// GET /api/algo-exes
/// Returns paginated list of algorithm executions
#[utoipa::path(
    get,
    path = "/api/algo-exes",
    tag = "algo-exe",
    summary = "List algorithm executions",
    description = "Retrieve a paginated list of algorithm executions with their current status",
    params(
        ("page" = Option<u32>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u32>, Query, description = "Items per page (default: 20, max: 100)")
    ),
    responses(
        (status = 200, description = "Algorithm executions retrieved successfully", body = ApiResponse<PaginatedResponse<AlgoExeData>>),
        (status = 400, description = "Invalid query parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
#[instrument(skip(state), fields(request_id))]
pub async fn get_algo_exes_handler(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Json<ApiResponse<PaginatedResponse<AlgoExeData>>> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);
    info!(
        "Getting algorithm executions list: page={}, limit={}",
        params.page, params.limit
    );

    // Forward request to Secure service
    let secure_url = format!(
        "{}/api/algo-exes?page={}&limit={}",
        state.config.services.secure_url, params.page, params.limit
    );

    match forward_get::<PaginatedResponse<AlgoExeData>>(
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
            Json(ApiResponse::success(response))
        }
        Err(e) => {
            error!(
                request_id = request_id,
                error = ?e,
                "Failed to get algorithm executions"
            );
            Json(ApiResponse {
                code: ResponseCode::InternalServerError,
                data: None,
                request_id: Some(request_id),
            })
        }
    }
}

/// Handler for getting algorithm execution details
///
/// GET /api/algo-exes/{id}
/// Returns detailed information about a specific algorithm execution
#[utoipa::path(
    get,
    path = "/api/algo-exes/{id}",
    tag = "algo-exe",
    summary = "Get algorithm execution",
    description = "Retrieve detailed information about a specific algorithm execution",
    params(
        ("id" = u64, Path, description = "Algorithm execution ID")
    ),
    responses(
        (status = 200, description = "Algorithm execution retrieved successfully", body = ApiResponse<AlgoExeData>),
        (status = 400, description = "Invalid algorithm execution ID", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 404, description = "Algorithm execution not found", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
#[instrument(skip(state), fields(request_id, algo_exe_id = %id))]
pub async fn get_algo_exe_handler(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Json<ApiResponse<AlgoExeData>> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);
    info!("Getting algorithm execution details: id={}", id);

    // Forward request to Secure service
    let secure_url = format!("{}/api/algo-exes/{}", state.config.services.secure_url, id);

    match forward_get::<AlgoExeData>(state.http_client.as_ref(), &secure_url, None).await {
        Ok(response) => {
            info!(
                "Retrieved algorithm execution: id={}, status={}",
                response.id, response.status
            );
            Json(ApiResponse::success(response))
        }
        Err(e) => {
            error!(
                request_id = request_id,
                algo_exe_id = %id,
                error = ?e,
                "Failed to get algorithm execution"
            );
            Json(ApiResponse {
                code: ResponseCode::InternalServerError,
                data: None,
                request_id: Some(request_id),
            })
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
