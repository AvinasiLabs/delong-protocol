use crate::{
    AppError, infra::ai_audit::AiAuditService, models::ai_audit::AiAuditReport as AiAuditModel,
    routes::AppState,
};
use avinapi::prelude::*;
use axum::extract::State;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, info};
use utoipa::ToSchema;
use validator::Validate;

// ===== Request Structures =====

/// AI audit request
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct AiAuditRequest {
    /// GitHub repository URL
    #[validate(url(message = "Invalid GitHub URL format"))]
    pub github_url: String,
    /// Git commit hash
    #[validate(length(min = 1, max = 100, message = "Commit hash must be 1-100 characters"))]
    pub commit_hash: String,
    /// Optional algorithm ID to associate with audit
    pub algorithm_id: Option<i32>,
    /// Optional execution ID to associate with audit
    pub execution_id: Option<i32>,
}

/// AI audit report query parameters
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct AiAuditReportQuery {
    /// Audit report ID
    #[validate(range(min = 1, message = "ID must be positive"))]
    pub id: Option<i32>,
    /// Algorithm ID filter
    #[validate(range(min = 1, message = "Algorithm ID must be positive"))]
    pub algorithm_id: Option<i32>,
    /// Execution ID filter
    #[validate(range(min = 1, message = "Execution ID must be positive"))]
    pub execution_id: Option<i32>,
    /// GitHub URL filter
    #[validate(url(message = "Invalid GitHub URL format"))]
    pub github_url: Option<String>,
    /// Commit hash filter
    #[validate(length(min = 1, max = 100, message = "Commit hash must be 1-100 characters"))]
    pub commit_hash: Option<String>,
}

// ===== Response Structures =====

/// AI audit response
#[derive(Debug, Serialize, ToSchema)]
pub struct AiAuditResponse {
    /// Unique audit ID
    pub id: i32,
    /// Audit status (processing, completed, failed)
    pub status: String,
    /// Status message
    pub message: String,
    /// Audit score (0-100)
    pub score: Option<i32>,
    /// Detailed audit results
    pub result: Option<Value>,
    /// Repository URL
    pub repo_url: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Completion timestamp
    pub completed_at: Option<DateTime<Utc>>,
    /// Error message if failed
    pub error_message: Option<String>,
}

impl From<AiAuditModel> for AiAuditResponse {
    fn from(report: AiAuditModel) -> Self {
        let message = if report.audit_status == "completed" {
            "Audit completed successfully".to_string()
        } else if report.audit_status == "failed" {
            "Audit failed".to_string()
        } else {
            "Audit in progress".to_string()
        };

        Self {
            id: report.id,
            status: report.audit_status,
            message,
            score: report.audit_score,
            result: report.audit_result,
            repo_url: report.repo_url,
            created_at: report.created_at,
            completed_at: report.completed_at,
            error_message: report.error_message,
        }
    }
}

/// AI audit result structure for detailed audit findings
#[derive(Debug, Serialize, ToSchema)]
pub struct AiAuditResult {
    /// File that was audited
    pub file: String,
    /// List of issues found in the file
    pub issues: Vec<AiAuditIssue>,
    /// Overall score for this file (0-100)
    pub score: i32,
}

/// Individual issue found during AI audit
#[derive(Debug, Serialize, ToSchema)]
pub struct AiAuditIssue {
    /// Type of issue (error, warning, info)
    pub issue_type: String,
    /// Line number where the issue was found
    pub line: Option<i32>,
    /// Column number where the issue was found
    pub column: Option<i32>,
    /// Description of the issue
    pub message: String,
    /// Severity level (high, medium, low)
    pub severity: String,
    /// Suggested fix if available
    pub suggestion: Option<String>,
}

/// Simplified AI audit report for listing
#[derive(Debug, Serialize, ToSchema)]
pub struct AiAuditReport {
    /// Report ID
    pub id: i32,
    /// GitHub URL
    pub github_url: String,
    /// Commit hash
    pub commit_hash: String,
    /// Audit status
    pub status: String,
    /// Audit score
    pub score: Option<i32>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

// Re-export the model's AiAuditReport as AiAudit for OpenAPI
pub use crate::models::ai_audit::AiAuditReport as AiAudit;

// ===== Handler Functions =====

/// Create AI audit report
#[utoipa::path(
    post,
    path = "/api/ai-audit",
    tag = "AI Audit",
    request_body = AiAuditRequest,
    responses(
        (status = 200, description = "AI audit operation result", body = AiAuditResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_ai_audit(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<AiAuditRequest>,
) -> JsonResult<AiAuditResponse> {
    info!("Create AI audit request: {:?}", payload);

    // Build repository URL
    let repo_url = match AiAuditService::build_repo_url(&payload.github_url, &payload.commit_hash) {
        Ok(url) => url,
        Err(e) => {
            error!("Failed to build repo URL: {}", e);
            return Err(AppError::Validation(format!("Invalid GitHub URL: {}", e)));
        }
    };

    // Check if audit already exists
    if let Some(existing) = AiAuditModel::find_by_github_and_commit(
        &state.db,
        &payload.github_url,
        &payload.commit_hash,
    )
    .await?
    {
        info!("Returning existing audit for {}", payload.github_url);
        return data!(AiAuditResponse::from(existing));
    }

    // Create new audit record
    let audit_id = AiAuditModel::create(
        &state.db,
        &payload.github_url,
        &payload.commit_hash,
        &repo_url,
        payload.algorithm_id,
        payload.execution_id,
    )
    .await?;

    // Call AI audit service (async, don't wait)
    let ai_service = state.ai_audit_service.clone();
    let db = state.db.clone();
    let repo_url_clone = repo_url.clone();

    tokio::spawn(async move {
        match ai_service
            .call_external_audit_service(&repo_url_clone)
            .await
        {
            Ok(result) => {
                // Convert Vec<AuditFileResult> to serde_json::Value
                let result_json = serde_json::to_value(&result).unwrap_or(Value::Null);
                let score = AiAuditModel::calculate_score(&result_json);

                if let Err(e) = AiAuditModel::update_status(
                    &db,
                    audit_id,
                    "completed",
                    Some(score),
                    Some(result_json),
                    None,
                )
                .await
                {
                    error!("Failed to update audit record: {:?}", e);
                }
            }
            Err(e) => {
                error!("AI audit failed: {:?}", e);
                if let Err(e) = AiAuditModel::update_status(
                    &db,
                    audit_id,
                    "failed",
                    None,
                    None,
                    Some(e.to_string()),
                )
                .await
                {
                    error!("Failed to update audit record: {:?}", e);
                }
            }
        }
    });

    let response = AiAuditResponse {
        id: audit_id,
        status: "processing".to_string(),
        message: "AI audit started successfully".to_string(),
        score: None,
        result: None,
        repo_url,
        created_at: Utc::now(),
        completed_at: None,
        error_message: None,
    };

    data!(response)
}

/// Get AI audit reports by query parameters
#[utoipa::path(
    get,
    path = "/api/ai-audit",
    tag = "AI Audit",

    responses(
        (status = 200, description = "AI audit report query result", body = AiAuditResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_ai_audit_reports(
    State(state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<AiAuditReportQuery>,
) -> JsonResult<AiAuditResponse> {
    info!("Get AI audit report request: {:?}", query);

    // Validate query parameters
    if query.id.is_none()
        && query.algorithm_id.is_none()
        && query.execution_id.is_none()
        && (query.github_url.is_none() || query.commit_hash.is_none())
    {
        return Err(AppError::Validation(
            "Either id, algorithm_id, execution_id, or both github_url and commit_hash are required"
                .to_string(),
        ));
    }

    // Find report using the model's flexible query method
    let report = AiAuditModel::find_by_query(
        &state.db,
        query.id,
        query.algorithm_id,
        query.execution_id,
        query.github_url.as_deref(),
        query.commit_hash.as_deref(),
    )
    .await?;

    match report {
        Some(report) => data!(AiAuditResponse::from(report)),
        None => Err(AppError::NotFound("Audit report not found".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_ai_audit_response_from_report() {
        let report = AiAuditModel {
            id: 1,
            github_url: "https://github.com/user/repo".to_string(),
            commit_hash: "abc123".to_string(),
            repo_url: "https://github.com/user/repo/tree/abc123".to_string(),
            algorithm_id: Some(1),
            execution_id: Some(2),
            audit_status: "completed".to_string(),
            audit_score: Some(85),
            audit_result: Some(json!({"test": "result"})),
            raw_response: Some(json!({"raw": "response"})),
            error_message: None,
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
        };

        let response = AiAuditResponse::from(report);
        assert_eq!(response.id, 1);
        assert_eq!(response.status, "completed");
        assert_eq!(response.message, "Audit completed successfully");
        assert_eq!(response.score, Some(85));
    }

    #[test]
    fn test_ai_audit_response_failed_status() {
        let report = AiAuditModel {
            id: 2,
            github_url: "https://github.com/user/repo".to_string(),
            commit_hash: "def456".to_string(),
            repo_url: "https://github.com/user/repo/tree/def456".to_string(),
            algorithm_id: None,
            execution_id: None,
            audit_status: "failed".to_string(),
            audit_score: None,
            audit_result: None,
            raw_response: None,
            error_message: Some("Test error".to_string()),
            created_at: Utc::now(),
            completed_at: None,
        };

        let response = AiAuditResponse::from(report);
        assert_eq!(response.status, "failed");
        assert_eq!(response.message, "Audit failed");
        assert_eq!(response.error_message, Some("Test error".to_string()));
    }

    #[test]
    fn test_ai_audit_request_validation() {
        let valid_request = AiAuditRequest {
            github_url: "https://github.com/user/repo".to_string(),
            commit_hash: "abc123".to_string(),
            algorithm_id: Some(1),
            execution_id: Some(2),
        };

        assert!(valid_request.validate().is_ok());

        let invalid_request = AiAuditRequest {
            github_url: "not-a-url".to_string(),
            commit_hash: "".to_string(),
            algorithm_id: None,
            execution_id: None,
        };

        assert!(invalid_request.validate().is_err());
    }
}
