//! Test report management handlers
//!
//! This module contains handlers for test report management,
//! including uploading test reports to the system.

use axum::{extract::State, response::Json};

use tracing::{error, info};

use crate::{handlers::ApiResponse, routes::AppState, services::http_client::forward_post};

use common::prelude::{UploadReportRequest, UploadReportResponse};

/// Handler for uploading test report
///
/// POST /api/reports
/// Forwards the request to the Secure service (TEE Hardware) for test report processing
#[utoipa::path(
    post,
    path = "/api/reports",
    tag = "reports",
    summary = "Upload test report",
    description = "Upload a test report for algorithm validation and processing",
    request_body = UploadReportRequest,
    responses(
        (status = 200, description = "Test report uploaded successfully", body = ApiResponse<UploadReportResponse>),
        (status = 400, description = "Invalid request parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 403, description = "Forbidden - insufficient permissions", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn upload_report_handler(
    State(state): State<AppState>,
    Json(payload): Json<UploadReportRequest>,
) -> Json<ApiResponse<UploadReportResponse>> {
    info!(
        "Uploading test report: type={}, algo_id={:?}, dataset_id={:?}",
        payload.report_type, payload.algorithm_id, payload.dataset_id
    );

    // Forward request to Secure service
    let secure_url = format!("{}/api/reports", state.config.services.secure_url);

    match forward_post::<UploadReportRequest, UploadReportResponse>(
        state.http_client.as_ref(),
        &secure_url,
        payload,
        None,
    )
    .await
    {
        Ok(response) => {
            info!(
                "Test report uploaded successfully: id={}, status={}",
                response.report_id, response.status
            );
            Json(ApiResponse::success(response))
        }
        Err(e) => {
            error!("Failed to upload test report: {:?}", e);
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
    fn test_upload_report_request_serialization() {
        let request = UploadReportRequest {
            report_data: "test report content".to_string(),
            report_type: "algorithm_test".to_string(),
            algorithm_id: Some("algo_123".to_string()),
            dataset_id: Some("dataset_456".to_string()),
            title: Some("Test Report".to_string()),
            description: Some("Test description".to_string()),
            tags: Some(vec!["test".to_string(), "algorithm".to_string()]),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("report_data"));
        assert!(json.contains("report_type"));
        assert!(json.contains("algorithm_id"));
        assert!(json.contains("dataset_id"));
    }

    #[test]
    fn test_upload_report_request_with_optional_fields() {
        let request = UploadReportRequest {
            report_data: "test report content".to_string(),
            report_type: "general_test".to_string(),
            algorithm_id: None,
            dataset_id: None,
            title: None,
            description: None,
            tags: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("report_data"));
        assert!(json.contains("report_type"));
        assert!(json.contains("null"));
    }

    #[test]
    fn test_upload_report_response_deserialization() {
        let json = r#"
        {
            "report_id": "report_789",
            "upload_time": "2023-01-01T12:00:00Z",
            "status": "uploaded"
        }
        "#;

        let response: UploadReportResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.report_id, "report_789");
        assert_eq!(response.upload_time, "2023-01-01T12:00:00Z");
        assert_eq!(response.status, "uploaded");
    }

    #[test]
    fn test_api_response_with_upload_report() {
        let response = UploadReportResponse {
            report_id: "test_report_123".to_string(),
            upload_time: "2023-01-01T10:30:00Z".to_string(),
            status: "processing".to_string(),
        };
        let api_response = ApiResponse::success(response);

        assert_eq!(api_response.code, ResponseCode::Success);
        assert!(api_response.data.is_some());
        let data = api_response.data.unwrap();
        assert_eq!(data.report_id, "test_report_123");
        assert_eq!(data.status, "processing");
    }

    #[test]
    fn test_upload_report_response_serialization() {
        let response = UploadReportResponse {
            report_id: "report_456".to_string(),
            upload_time: "2023-12-01T15:45:00Z".to_string(),
            status: "completed".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("report_456"));
        assert!(json.contains("2023-12-01T15:45:00Z"));
        assert!(json.contains("completed"));
    }
}
