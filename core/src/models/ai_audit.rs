//! AI audit-related models for the DeLong Protocol
//!
//! This module contains the database model and operations for AI audit functionality.

use crate::AppError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

/// AI audit report model matching PostgreSQL schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAuditReport {
    pub id: i32,
    pub github_url: String,
    pub commit_hash: String,
    pub repo_url: String,
    pub algorithm_id: Option<i32>,
    pub execution_id: Option<i32>,
    pub audit_status: String,
    pub audit_score: Option<i32>,
    pub audit_result: Option<Value>,
    pub raw_response: Option<Value>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl AiAuditReport {
    /// Create a new audit report
    pub async fn create(
        pool: &PgPool,
        github_url: &str,
        commit_hash: &str,
        repo_url: &str,
        algorithm_id: Option<i32>,
        execution_id: Option<i32>,
    ) -> Result<i32, AppError> {
        let result = sqlx::query!(
            r#"
            INSERT INTO ai_audit_reports (
                github_url, commit_hash, repo_url, algorithm_id, execution_id,
                audit_status, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, 'processing', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            RETURNING id
            "#,
            github_url,
            commit_hash,
            repo_url,
            algorithm_id,
            execution_id
        )
        .fetch_one(pool)
        .await?;

        Ok(result.id)
    }

    /// Find an existing audit report by GitHub URL and commit hash
    pub async fn find_by_github_and_commit(
        pool: &PgPool,
        github_url: &str,
        commit_hash: &str,
    ) -> Result<Option<Self>, AppError> {
        let record = sqlx::query!(
            r#"
            SELECT id, github_url, commit_hash, repo_url, algorithm_id, execution_id,
                   audit_status, audit_score, audit_result, raw_response, error_message,
                   created_at, completed_at
            FROM ai_audit_reports
            WHERE github_url = $1 AND commit_hash = $2
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            github_url,
            commit_hash
        )
        .fetch_optional(pool)
        .await?;

        Ok(record.map(|r| AiAuditReport {
            id: r.id,
            github_url: r.github_url,
            commit_hash: r.commit_hash,
            repo_url: r.repo_url,
            algorithm_id: r.algorithm_id,
            execution_id: r.execution_id,
            audit_status: r.audit_status,
            audit_score: Some(r.audit_score),
            audit_result: Some(r.audit_result),
            raw_response: Some(r.raw_response),
            error_message: r.error_message,
            created_at: r.created_at,
            completed_at: r.completed_at,
        }))
    }

    /// Find audit report by ID
    pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<Self>, AppError> {
        let record = sqlx::query!(
            r#"
            SELECT id, github_url, commit_hash, repo_url, algorithm_id, execution_id,
                   audit_status, audit_score, audit_result, raw_response, error_message,
                   created_at, completed_at
            FROM ai_audit_reports
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(record.map(|r| AiAuditReport {
            id: r.id,
            github_url: r.github_url,
            commit_hash: r.commit_hash,
            repo_url: r.repo_url,
            algorithm_id: r.algorithm_id,
            execution_id: r.execution_id,
            audit_status: r.audit_status,
            audit_score: Some(r.audit_score),
            audit_result: Some(r.audit_result),
            raw_response: Some(r.raw_response),
            error_message: r.error_message,
            created_at: r.created_at,
            completed_at: r.completed_at,
        }))
    }

    /// Find audit reports by algorithm ID
    pub async fn find_by_algorithm_id(
        pool: &PgPool,
        algorithm_id: i32,
    ) -> Result<Vec<Self>, AppError> {
        let records = sqlx::query!(
            r#"
            SELECT id, github_url, commit_hash, repo_url, algorithm_id, execution_id,
                   audit_status, audit_score, audit_result, raw_response, error_message,
                   created_at, completed_at
            FROM ai_audit_reports
            WHERE algorithm_id = $1
            ORDER BY created_at DESC
            "#,
            algorithm_id
        )
        .fetch_all(pool)
        .await?;

        Ok(records
            .into_iter()
            .map(|r| AiAuditReport {
                id: r.id,
                github_url: r.github_url,
                commit_hash: r.commit_hash,
                repo_url: r.repo_url,
                algorithm_id: r.algorithm_id,
                execution_id: r.execution_id,
                audit_status: r.audit_status,
                audit_score: Some(r.audit_score),
                audit_result: Some(r.audit_result),
                raw_response: Some(r.raw_response),
                error_message: r.error_message,
                created_at: r.created_at,
                completed_at: r.completed_at,
            })
            .collect())
    }

    /// Find audit reports by execution ID
    pub async fn find_by_execution_id(
        pool: &PgPool,
        execution_id: i32,
    ) -> Result<Vec<Self>, AppError> {
        let records = sqlx::query!(
            r#"
            SELECT id, github_url, commit_hash, repo_url, algorithm_id, execution_id,
                   audit_status, audit_score, audit_result, raw_response, error_message,
                   created_at, completed_at
            FROM ai_audit_reports
            WHERE execution_id = $1
            ORDER BY created_at DESC
            "#,
            execution_id
        )
        .fetch_all(pool)
        .await?;

        Ok(records
            .into_iter()
            .map(|r| AiAuditReport {
                id: r.id,
                github_url: r.github_url,
                commit_hash: r.commit_hash,
                repo_url: r.repo_url,
                algorithm_id: r.algorithm_id,
                execution_id: r.execution_id,
                audit_status: r.audit_status,
                audit_score: Some(r.audit_score),
                audit_result: Some(r.audit_result),
                raw_response: Some(r.raw_response),
                error_message: r.error_message,
                created_at: r.created_at,
                completed_at: r.completed_at,
            })
            .collect())
    }

    /// Update audit report status
    pub async fn update_status(
        pool: &PgPool,
        id: i32,
        status: &str,
        score: Option<i32>,
        result: Option<Value>,
        error_message: Option<String>,
    ) -> Result<(), AppError> {
        let completed_at = if status == "completed" || status == "failed" {
            Some(Utc::now())
        } else {
            None
        };

        sqlx::query!(
            r#"
            UPDATE ai_audit_reports
            SET audit_status = $2,
                audit_score = $3,
                audit_result = $4,
                error_message = $5,
                completed_at = $6,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            "#,
            id,
            status,
            score,
            result,
            error_message,
            completed_at
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Find audit report with flexible query parameters
    pub async fn find_by_query(
        pool: &PgPool,
        id: Option<i32>,
        algorithm_id: Option<i32>,
        execution_id: Option<i32>,
        github_url: Option<&str>,
        commit_hash: Option<&str>,
    ) -> Result<Option<Self>, AppError> {
        // For flexible queries, we need to handle different combinations
        // Let's prioritize the most specific queries first

        if let Some(id) = id {
            return Self::find_by_id(pool, id).await;
        }

        if let (Some(url), Some(hash)) = (github_url, commit_hash) {
            return Self::find_by_github_and_commit(pool, url, hash).await;
        }

        if let Some(algorithm_id) = algorithm_id {
            let reports = Self::find_by_algorithm_id(pool, algorithm_id).await?;
            return Ok(reports.into_iter().next());
        }

        if let Some(execution_id) = execution_id {
            let reports = Self::find_by_execution_id(pool, execution_id).await?;
            return Ok(reports.into_iter().next());
        }

        Ok(None)
    }

    /// Get all audit reports with pagination
    pub async fn list(
        pool: &PgPool,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<Self>, i64), AppError> {
        let records = sqlx::query!(
            r#"
            SELECT id, github_url, commit_hash, repo_url, algorithm_id, execution_id,
                   audit_status, audit_score, audit_result, raw_response, error_message,
                   created_at, completed_at
            FROM ai_audit_reports
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(pool)
        .await?;

        let reports = records
            .into_iter()
            .map(|r| AiAuditReport {
                id: r.id,
                github_url: r.github_url,
                commit_hash: r.commit_hash,
                repo_url: r.repo_url,
                algorithm_id: r.algorithm_id,
                execution_id: r.execution_id,
                audit_status: r.audit_status,
                audit_score: Some(r.audit_score),
                audit_result: Some(r.audit_result),
                raw_response: Some(r.raw_response),
                error_message: r.error_message,
                created_at: r.created_at,
                completed_at: r.completed_at,
            })
            .collect();

        let total = sqlx::query_scalar!("SELECT COUNT(*) as count FROM ai_audit_reports")
            .fetch_one(pool)
            .await?
            .unwrap_or(0);

        Ok((reports, total))
    }

    /// Delete an audit report by ID
    pub async fn delete(pool: &PgPool, id: i32) -> Result<bool, AppError> {
        let result = sqlx::query!("DELETE FROM ai_audit_reports WHERE id = $1", id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Calculate audit score from results
    pub fn calculate_score(audit_result: &Value) -> i32 {
        if let Some(results) = audit_result.as_array() {
            let total_files = results.len() as f32;
            if total_files == 0.0 {
                return 100;
            }

            let mut total_score = 0.0;
            for result in results {
                if let Some(issues) = result.get("issues").and_then(|v| v.as_array()) {
                    let file_score = match issues.len() {
                        0 => 100.0,
                        1..=2 => 80.0,
                        3..=5 => 60.0,
                        6..=10 => 40.0,
                        _ => 20.0,
                    };
                    total_score += file_score;
                } else {
                    total_score += 100.0;
                }
            }

            (total_score / total_files) as i32
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_calculate_score() {
        // Test with no issues
        let result = json!([
            {"file": "test1.rs", "issues": []},
            {"file": "test2.rs", "issues": []}
        ]);
        assert_eq!(AiAuditReport::calculate_score(&result), 100);

        // Test with some issues
        let result = json!([
            {"file": "test1.rs", "issues": [{"type": "error"}]},
            {"file": "test2.rs", "issues": [{"type": "warning"}, {"type": "info"}]}
        ]);
        assert_eq!(AiAuditReport::calculate_score(&result), 80);

        // Test with many issues
        let result = json!([
            {"file": "test1.rs", "issues": [
                {"type": "error"}, {"type": "error"}, {"type": "error"},
                {"type": "error"}, {"type": "error"}, {"type": "error"}
            ]}
        ]);
        assert_eq!(AiAuditReport::calculate_score(&result), 40);

        // Test with empty array
        let result = json!([]);
        assert_eq!(AiAuditReport::calculate_score(&result), 100);

        // Test with invalid format
        let result = json!({"invalid": "format"});
        assert_eq!(AiAuditReport::calculate_score(&result), 0);
    }
}
