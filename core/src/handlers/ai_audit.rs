//! AI audit handlers
//!
//! This module contains HTTP handlers for AI audit functionality,
//! including GitHub repository code auditing and audit report management.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use common::{ApiError, ApiResponse, ResponseCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, Row};
use std::sync::Arc;
use tracing::{error, info, warn};
use validator::Validate;

use crate::services::ai_audit::AuditFileResult;
use crate::{AiAuditService, AppState};

/// AI audit request
#[derive(Debug, Deserialize, Validate)]
pub struct AiAuditRequest {
    #[validate(url)]
    pub github_url: String,
    #[validate(length(min = 7, max = 40))]
    pub commit_hash: String,
    pub algorithm_id: Option<i32>,
    pub execution_id: Option<i32>,
}

/// AI audit response
#[derive(Debug, Serialize)]
pub struct AiAuditResponse {
    pub id: i32,
    pub status: String,
    pub message: String,
    pub score: Option<i32>,
    pub result: Option<Value>,
    pub repo_url: String,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
}

/// AI audit list query parameters
#[derive(Debug, Deserialize)]
pub struct AiAuditListQuery {
    pub algorithm_id: Option<i32>,
    pub status: Option<String>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

/// AI audit list response
#[derive(Debug, Serialize)]
pub struct AiAuditListResponse {
    pub reports: Vec<AuditReportSummary>,
    pub total: i64,
    pub page: i32,
    pub limit: i32,
}

/// Audit report summary
#[derive(Debug, Serialize, FromRow)]
pub struct AuditReportSummary {
    pub id: i32,
    pub algorithm_id: Option<i32>,
    pub execution_id: Option<i32>,
    pub github_url: String,
    pub commit_hash: String,
    pub audit_status: String,
    pub audit_score: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// AI audit report query parameters
#[derive(Debug, Deserialize)]
pub struct AiAuditReportQuery {
    pub id: Option<i32>,
    pub algorithm_id: Option<i32>,
    pub execution_id: Option<i32>,
    pub github_url: Option<String>,
    pub commit_hash: Option<String>,
}

/// Submit AI audit request
/// POST /ai-audit
pub async fn create_ai_audit(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AiAuditRequest>,
) -> Result<Json<ApiResponse<AiAuditResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("AI audit request for GitHub URL: {}", payload.github_url);

    // Validate input
    if let Err(validation_errors) = payload.validate() {
        warn!("AI audit validation failed: {:?}", validation_errors);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // Validate GitHub URL format
    if !payload.github_url.starts_with("https://github.com/") {
        warn!("Invalid GitHub URL format: {}", payload.github_url);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // Build repo URL
    let repo_url = match AiAuditService::build_repo_url(&payload.github_url, &payload.commit_hash) {
        Ok(url) => url,
        Err(e) => {
            error!("Failed to build repo URL: {}", e);
            let response_code = match e {
                crate::errors::AppError::InvalidRequest { .. } => ResponseCode::BadRequest,
                _ => ResponseCode::InternalServerError,
            };
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(response_code)),
            ));
        }
    };

    // Check for existing audit
    match check_existing_audit(&state, &payload).await {
        Ok(Some(existing)) => {
            info!("Found existing audit with ID: {}", existing.id);
            return Ok(Json(ApiResponse::success(existing)));
        }
        Ok(None) => {
            // No existing audit, proceed to create new one
        }
        Err(e) => {
            error!("Failed to check existing audit: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            ));
        }
    }

    // Create audit record in database
    let audit_record = create_audit_record(&state, &payload, &repo_url)
        .await
        .map_err(|e| {
            error!("Failed to create audit record: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            )
        })?;

    // Call external AI audit service
    match state
        .ai_audit_service
        .call_external_audit_service(&repo_url)
        .await
    {
        Ok(audit_result) => {
            // Calculate audit score
            let score = calculate_audit_score(&audit_result);

            // Update record with successful result
            match update_audit_record(
                &state,
                audit_record.id,
                "completed",
                Some(score),
                Some(audit_result.clone()),
                None,
            )
            .await
            {
                Ok(updated_record) => {
                    info!(
                        "AI audit completed successfully with ID: {}",
                        updated_record.id
                    );
                    Ok(Json(ApiResponse::success(updated_record)))
                }
                Err(e) => {
                    error!("Failed to update audit record: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiResponse::error(ResponseCode::InternalServerError)),
                    ))
                }
            }
        }
        Err(e) => {
            error!("Failed to call AI audit service: {}", e);
            // Map AppError to appropriate response code
            let response_code = match e {
                crate::errors::AppError::ConfigError { .. } => ResponseCode::InternalServerError,
                crate::errors::AppError::ExternalServiceError { .. } => {
                    ResponseCode::InternalServerError
                }
                crate::errors::AppError::InvalidRequest { .. } => ResponseCode::BadRequest,
                _ => ResponseCode::InternalServerError,
            };
            // Update record as failed
            let _ = update_audit_record(
                &state,
                audit_record.id,
                "failed",
                None,
                None,
                Some(e.to_string()),
            )
            .await;
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(response_code)),
            ))
        }
    }
}

/// Get AI audit records
/// GET /ai-audit
pub async fn get_ai_audit_reports(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AiAuditListQuery>,
) -> Result<Json<ApiResponse<AiAuditListResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Get AI audit reports request");

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(10).clamp(1, 100);
    let offset = (page - 1) * limit;

    // Build query conditions
    let mut where_conditions = vec!["1 = 1".to_string()];
    let mut params = vec![];
    let mut param_index = 1;

    if let Some(algorithm_id) = query.algorithm_id {
        where_conditions.push(format!("algorithm_id = ${}", param_index));
        params.push(algorithm_id.to_string());
        param_index += 1;
    }

    if let Some(ref status) = query.status {
        where_conditions.push(format!("audit_status = ${}", param_index));
        params.push(status.clone());
        param_index += 1;
    }

    let where_clause = where_conditions.join(" AND ");

    // Get total count
    let total_query = format!(
        "SELECT COUNT(*) FROM ai_audit_reports WHERE {}",
        where_clause
    );
    let mut count_query = sqlx::query_scalar::<_, i64>(&total_query);
    for param in &params {
        count_query = count_query.bind(param);
    }
    let total = count_query.fetch_one(&state.db).await.map_err(|e| {
        error!("Failed to get audit reports count: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(ResponseCode::InternalServerError)),
        )
    })?;

    // Get paginated reports
    let reports_query = format!(
        r#"
        SELECT id, algorithm_id, execution_id, github_url, commit_hash,
               audit_status, audit_score, created_at, completed_at
        FROM ai_audit_reports
        WHERE {}
        ORDER BY created_at DESC
        LIMIT ${} OFFSET ${}
        "#,
        where_clause,
        param_index,
        param_index + 1
    );

    let mut query_builder = sqlx::query_as::<_, AuditReportSummary>(&reports_query);
    for param in &params {
        query_builder = query_builder.bind(param);
    }
    query_builder = query_builder.bind(limit).bind(offset);

    let reports = query_builder.fetch_all(&state.db).await.map_err(|e| {
        error!("Failed to get audit reports: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(ResponseCode::InternalServerError)),
        )
    })?;

    let response = AiAuditListResponse {
        reports,
        total,
        page,
        limit,
    };

    Ok(Json(ApiResponse::success(response)))
}

/// Get AI audit report by ID or query parameters
/// GET /ai-audit/report
pub async fn get_ai_audit_report(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AiAuditReportQuery>,
) -> Result<Json<ApiResponse<AiAuditResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Get AI audit report request");

    // Build query based on parameters
    let (query_sql, params) = build_report_query(&query)?;

    let row = match params.len() {
        1 => {
            sqlx::query(&query_sql)
                .bind(&params[0])
                .fetch_optional(&state.db)
                .await
        }
        2 => {
            sqlx::query(&query_sql)
                .bind(&params[0])
                .bind(&params[1])
                .fetch_optional(&state.db)
                .await
        }
        _ => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            ));
        }
    }
    .map_err(|e| {
        error!("Failed to get audit report: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(ResponseCode::InternalServerError)),
        )
    })?;

    if let Some(row) = row {
        let response = AiAuditResponse {
            id: row.get("id"),
            status: row.get("audit_status"),
            message: "AI audit report retrieved successfully".to_string(),
            score: row.get("audit_score"),
            result: row.get("audit_result"),
            repo_url: row.get("repo_url"),
            created_at: row.get::<DateTime<Utc>, _>("created_at").to_rfc3339(),
            completed_at: row
                .get::<Option<DateTime<Utc>>, _>("completed_at")
                .map(|dt| dt.to_rfc3339()),
            error_message: row.get("error_message"),
        };
        Ok(Json(ApiResponse::success(response)))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error(ResponseCode::NotFound)),
        ))
    }
}

/// List AI audit reports with pagination (alias)
/// GET /ai-audit/list
pub async fn list_ai_audit_reports(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AiAuditListQuery>,
) -> Result<Json<ApiResponse<AiAuditListResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    get_ai_audit_reports(State(state), Query(query)).await
}

// Helper functions

/// Calculate audit score based on results
fn calculate_audit_score(audit_result: &[AuditFileResult]) -> i32 {
    if audit_result.is_empty() {
        return 0;
    }

    let rejected_files = audit_result
        .iter()
        .filter(|item| item.status == "REJECT")
        .count();

    let total_files = audit_result.len();
    let passed_files = total_files - rejected_files;

    // Base score based on pass rate
    let mut base_score = ((passed_files as f64 / total_files as f64) * 100.0).round() as i32;

    // Consider confidence weighting
    let mut weighted_score = 0.0;
    let mut total_weight = 0.0;

    for item in audit_result {
        let confidence = item.confidence;
        let weight = confidence;

        if item.status == "ACCEPT" {
            weighted_score += 100.0 * weight;
        } else {
            weighted_score += 0.0 * weight;
        }
        total_weight += weight;
    }

    if total_weight > 0.0 {
        let avg_weighted_score = (weighted_score / total_weight).round() as i32;
        // Combine base score and weighted score
        base_score = ((base_score + avg_weighted_score) / 2).max(0).min(100);
    }

    base_score.max(0).min(100)
}

/// Check for existing audit record
async fn check_existing_audit(
    state: &Arc<AppState>,
    payload: &AiAuditRequest,
) -> Result<Option<AiAuditResponse>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT id, audit_status, audit_score, audit_result, repo_url,
               created_at, completed_at, error_message
        FROM ai_audit_reports
        WHERE github_url = $1 AND commit_hash = $2
        ORDER BY created_at DESC LIMIT 1
        "#,
    )
    .bind(&payload.github_url)
    .bind(&payload.commit_hash)
    .fetch_optional(&state.db)
    .await?;

    if let Some(row) = row {
        let status: String = row.get("audit_status");

        if status == "completed" || status == "processing" {
            let message = if status == "completed" {
                "Audit already completed".to_string()
            } else {
                "Audit in progress".to_string()
            };

            return Ok(Some(AiAuditResponse {
                id: row.get("id"),
                status,
                message,
                score: row.get("audit_score"),
                result: row.get("audit_result"),
                repo_url: row.get("repo_url"),
                created_at: row.get::<DateTime<Utc>, _>("created_at").to_rfc3339(),
                completed_at: row
                    .get::<Option<DateTime<Utc>>, _>("completed_at")
                    .map(|dt| dt.to_rfc3339()),
                error_message: row.get("error_message"),
            }));
        }
    }

    Ok(None)
}

/// Create audit record in database
async fn create_audit_record(
    state: &Arc<AppState>,
    payload: &AiAuditRequest,
    repo_url: &str,
) -> Result<AiAuditResponse, ApiError> {
    let row = sqlx::query(
        r#"
        INSERT INTO ai_audit_reports (
            github_url, commit_hash, repo_url, algorithm_id, execution_id, audit_status
        )
        VALUES ($1, $2, $3, $4, $5, 'processing')
        RETURNING id, created_at
        "#,
    )
    .bind(&payload.github_url)
    .bind(&payload.commit_hash)
    .bind(repo_url)
    .bind(payload.algorithm_id)
    .bind(payload.execution_id)
    .fetch_one(&state.db)
    .await?;

    Ok(AiAuditResponse {
        id: row.get("id"),
        status: "processing".to_string(),
        message: "AI audit submitted successfully".to_string(),
        score: None,
        result: None,
        repo_url: repo_url.to_string(),
        created_at: row.get::<DateTime<Utc>, _>("created_at").to_rfc3339(),
        completed_at: None,
        error_message: None,
    })
}

/// Update audit record with results
async fn update_audit_record(
    state: &Arc<AppState>,
    audit_id: i32,
    status: &str,
    score: Option<i32>,
    result: Option<Vec<AuditFileResult>>,
    error_message: Option<String>,
) -> Result<AiAuditResponse, ApiError> {
    let result_json = result.map(|r| serde_json::to_value(r).unwrap_or(Value::Null));

    let row = sqlx::query(
        r#"
        UPDATE ai_audit_reports
        SET audit_status = $1, audit_score = $2, audit_result = $3, raw_response = $4,
            error_message = $5, completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP
        WHERE id = $6
        RETURNING id, repo_url, created_at, completed_at
        "#,
    )
    .bind(status)
    .bind(score)
    .bind(&result_json)
    .bind(&result_json)
    .bind(error_message.as_deref())
    .bind(audit_id)
    .fetch_one(&state.db)
    .await?;

    let message = match status {
        "completed" => "AI audit completed successfully".to_string(),
        "failed" => "AI audit failed".to_string(),
        _ => "AI audit status updated".to_string(),
    };

    Ok(AiAuditResponse {
        id: audit_id,
        status: status.to_string(),
        message,
        score,
        result: result_json,
        repo_url: row.get("repo_url"),
        created_at: row.get::<DateTime<Utc>, _>("created_at").to_rfc3339(),
        completed_at: row
            .get::<Option<DateTime<Utc>>, _>("completed_at")
            .map(|dt| dt.to_rfc3339()),
        error_message,
    })
}

/// Build report query based on parameters
fn build_report_query(
    query: &AiAuditReportQuery,
) -> Result<(String, Vec<String>), (StatusCode, Json<ApiResponse<()>>)> {
    let mut params = vec![];

    let query_sql = if let Some(id) = query.id {
        params.push(id.to_string());
        r#"
        SELECT id, algorithm_id, execution_id, github_url, commit_hash,
               audit_status, audit_score, audit_result, repo_url,
               created_at, completed_at, error_message
        FROM ai_audit_reports
        WHERE id = $1
        "#
        .to_string()
    } else if let Some(execution_id) = query.execution_id {
        params.push(execution_id.to_string());
        r#"
        SELECT id, algorithm_id, execution_id, github_url, commit_hash,
               audit_status, audit_score, audit_result, repo_url,
               created_at, completed_at, error_message
        FROM ai_audit_reports
        WHERE execution_id = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#
        .to_string()
    } else if let Some(algorithm_id) = query.algorithm_id {
        params.push(algorithm_id.to_string());
        r#"
        SELECT id, algorithm_id, execution_id, github_url, commit_hash,
               audit_status, audit_score, audit_result, repo_url,
               created_at, completed_at, error_message
        FROM ai_audit_reports
        WHERE algorithm_id = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#
        .to_string()
    } else if let (Some(github_url), Some(commit_hash)) = (&query.github_url, &query.commit_hash) {
        params.push(github_url.clone());
        params.push(commit_hash.clone());
        r#"
        SELECT id, algorithm_id, execution_id, github_url, commit_hash,
               audit_status, audit_score, audit_result, repo_url,
               created_at, completed_at, error_message
        FROM ai_audit_reports
        WHERE github_url = $1 AND commit_hash = $2
        ORDER BY created_at DESC
        LIMIT 1
        "#
        .to_string()
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    };

    Ok((query_sql, params))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_audit_request_validation() {
        let request = AiAuditRequest {
            github_url: "invalid-url".to_string(), // Invalid URL
            commit_hash: "abc".to_string(),        // Too short
            algorithm_id: None,
            execution_id: None,
        };

        assert!(request.validate().is_err());
    }

    #[test]
    fn test_ai_audit_request_valid() {
        let request = AiAuditRequest {
            github_url: "https://github.com/test/repo".to_string(),
            commit_hash: "a1b2c3d4e5f6g7h8".to_string(),
            algorithm_id: Some(1),
            execution_id: Some(1),
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_build_repo_url() {
        let result = AiAuditService::build_repo_url(
            "https://github.com/user/repo",
            "1234567890123456789012345678901234567890",
        );
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            "https://codeload.github.com/user/repo/tar.gz/1234567890123456789012345678901234567890"
        );

        let invalid_url = AiAuditService::build_repo_url(
            "invalid-url",
            "1234567890123456789012345678901234567890",
        );
        assert!(invalid_url.is_err());
    }

    #[test]
    fn test_calculate_audit_score() {
        let audit_result = vec![
            AuditFileResult {
                status: "ACCEPT".to_string(),
                file_path: "src/main.rs".to_string(),
                confidence: 0.95,
                breaches: None,
            },
            AuditFileResult {
                status: "REJECT".to_string(),
                file_path: "src/bad.rs".to_string(),
                confidence: 0.92,
                breaches: Some(vec![crate::services::ai_audit::SecurityBreach {
                    code: "unsafe code".to_string(),
                    reason: "Potential security issue".to_string(),
                    severity: Some("HIGH".to_string()),
                    line_number: Some(42),
                }]),
            },
        ];

        let score = calculate_audit_score(&audit_result);
        assert!(score >= 0 && score <= 100);
    }
}
