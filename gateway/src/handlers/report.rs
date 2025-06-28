//! Test report management handlers
//!
//! This module contains handlers for test report management,
//! including uploading test reports to the system.

use axum::{extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{handlers::ApiResponse, routes::AppState, utils::http_client::forward_post};

/// Request payload for uploading test report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadReportRequest {
    pub report_data: String,
    pub report_type: String,
    pub algorithm_id: Option<String>,
    pub dataset_id: Option<String>,
}

/// Response for test report upload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadReportResponse {
    pub report_id: String,
    pub upload_time: String,
    pub status: String,
}

/// Handler for uploading test report
///
/// POST /api/reports
/// Forwards the request to the Core service for test report processing
pub async fn upload_report_handler(
    State(state): State<AppState>,
    Json(payload): Json<UploadReportRequest>,
) -> Result<Json<ApiResponse<UploadReportResponse>>, StatusCode> {
    info!(
        "Uploading test report: type={}, algo_id={:?}, dataset_id={:?}",
        payload.report_type, payload.algorithm_id, payload.dataset_id
    );

    // Forward request to Core service
    let core_url = format!("{}/api/reports", state.config.services.core_url);

    match forward_post::<UploadReportRequest, UploadReportResponse>(
        state.http_client.as_ref(),
        &core_url,
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
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to upload test report: {}", e);
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
    fn test_upload_report_request_serialization() {
        let request = UploadReportRequest {
            report_data: "test report content".to_string(),
            report_type: "algorithm_test".to_string(),
            algorithm_id: Some("algo_123".to_string()),
            dataset_id: Some("dataset_456".to_string()),
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

        assert!(api_response.success);
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
