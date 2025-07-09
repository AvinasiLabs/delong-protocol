//! AI audit-related models for the DeLong Protocol
//!
//! This module contains all data structures related to AI audit functionality,
//! including audit reports, audit requests, and audit results.

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

/// AI audit status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "ai_audit_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AiAuditStatus {
    Pending,
    Completed,
    Failed,
    InProgress,
}

/// AI audit report model matching PostgreSQL schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiAuditReport {
    pub id: Uuid,
    pub dataset_id: Option<Uuid>,
    pub algorithm_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub report_type: String,
    pub status: AiAuditStatus,
    pub findings: serde_json::Value,
    pub recommendations: serde_json::Value,
    pub risk_score: Option<BigDecimal>,
    pub confidence_score: Option<BigDecimal>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
}

/// AI audit report response (without sensitive data)
#[derive(Debug, Clone, Serialize)]
pub struct AiAuditReportResponse {
    pub id: Uuid,
    pub dataset_id: Option<Uuid>,
    pub algorithm_id: Option<Uuid>,
    pub report_type: String,
    pub status: AiAuditStatus,
    pub findings: serde_json::Value,
    pub recommendations: serde_json::Value,
    pub risk_score: Option<BigDecimal>,
    pub confidence_score: Option<BigDecimal>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
}

impl From<AiAuditReport> for AiAuditReportResponse {
    fn from(report: AiAuditReport) -> Self {
        Self {
            id: report.id,
            dataset_id: report.dataset_id,
            algorithm_id: report.algorithm_id,
            report_type: report.report_type,
            status: report.status,
            findings: report.findings,
            recommendations: report.recommendations,
            risk_score: report.risk_score,
            confidence_score: report.confidence_score,
            created_at: report.created_at,
            completed_at: report.completed_at,
            metadata: report.metadata,
        }
    }
}

/// Create AI audit request
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateAiAuditRequest {
    pub dataset_id: Option<Uuid>,
    pub algorithm_id: Option<Uuid>,
    #[validate(length(min = 1, max = 100))]
    pub report_type: String,
    pub metadata: Option<serde_json::Value>,
}

/// Update AI audit request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateAiAuditRequest {
    pub status: Option<AiAuditStatus>,
    pub findings: Option<serde_json::Value>,
    pub recommendations: Option<serde_json::Value>,
    pub risk_score: Option<BigDecimal>,
    pub confidence_score: Option<BigDecimal>,
    pub metadata: Option<serde_json::Value>,
}

/// AI audit query parameters
#[derive(Debug, Deserialize)]
pub struct AiAuditQuery {
    pub dataset_id: Option<Uuid>,
    pub algorithm_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub report_type: Option<String>,
    pub status: Option<AiAuditStatus>,
    pub min_risk_score: Option<BigDecimal>,
    pub max_risk_score: Option<BigDecimal>,
    pub min_confidence_score: Option<BigDecimal>,
    pub max_confidence_score: Option<BigDecimal>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

/// Security vulnerability found during audit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityVulnerability {
    pub severity: String,
    pub category: String,
    pub description: String,
    pub line_number: Option<u32>,
    pub file_path: Option<String>,
    pub recommendation: Option<String>,
    pub cwe_id: Option<String>,
    pub cvss_score: Option<f64>,
}

/// Security finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub id: String,
    pub title: String,
    pub description: String,
    pub severity: String,
    pub category: String,
    pub location: FindingLocation,
    pub recommendation: String,
    pub references: Vec<String>,
    pub confidence: f64,
}

/// Finding location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingLocation {
    pub file_path: String,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub column_start: Option<u32>,
    pub column_end: Option<u32>,
    pub function_name: Option<String>,
}

/// Audit recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecommendation {
    pub id: String,
    pub title: String,
    pub description: String,
    pub priority: String,
    pub category: String,
    pub effort: String,
    pub impact: String,
    pub implementation_steps: Vec<String>,
    pub resources: Vec<String>,
}

/// AI audit result (for processing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAuditResult {
    pub findings: Vec<SecurityFinding>,
    pub recommendations: Vec<AuditRecommendation>,
    pub risk_score: BigDecimal,
    pub confidence_score: BigDecimal,
    pub summary: AuditSummary,
    pub metrics: AuditMetrics,
}

/// Audit summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSummary {
    pub total_files_analyzed: u32,
    pub total_lines_analyzed: u32,
    pub total_findings: u32,
    pub critical_findings: u32,
    pub high_findings: u32,
    pub medium_findings: u32,
    pub low_findings: u32,
    pub info_findings: u32,
    pub overall_security_rating: String,
}

/// Audit metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditMetrics {
    pub complexity_score: f64,
    pub maintainability_score: f64,
    pub test_coverage: Option<f64>,
    pub code_quality_score: f64,
    pub performance_score: f64,
    pub security_score: f64,
}

/// Create AI audit response
#[derive(Debug, Serialize)]
pub struct CreateAiAuditResponse {
    pub report: AiAuditReportResponse,
    pub message: String,
}

/// AI audit list response
#[derive(Debug, Serialize)]
pub struct AiAuditListResponse {
    pub reports: Vec<AiAuditReportResponse>,
    pub total: i64,
    pub page: i32,
    pub limit: i32,
    pub total_pages: i32,
}

/// AI audit statistics
#[derive(Debug, Serialize)]
pub struct AiAuditStatistics {
    pub total_audits: i64,
    pub pending_audits: i64,
    pub completed_audits: i64,
    pub failed_audits: i64,
    pub in_progress_audits: i64,
    pub average_risk_score: f64,
    pub average_confidence_score: f64,
    pub total_findings: i64,
    pub critical_findings: i64,
    pub high_findings: i64,
    pub medium_findings: i64,
    pub low_findings: i64,
}

/// Audit dashboard data
#[derive(Debug, Serialize)]
pub struct AuditDashboard {
    pub statistics: AiAuditStatistics,
    pub recent_audits: Vec<AiAuditReportResponse>,
    pub trending_issues: Vec<TrendingIssue>,
    pub risk_distribution: RiskDistribution,
    pub audit_timeline: Vec<AuditTimelineEntry>,
}

/// Trending issue
#[derive(Debug, Serialize)]
pub struct TrendingIssue {
    pub issue_type: String,
    pub category: String,
    pub count: i32,
    pub severity: String,
    pub trend: String,
    pub description: String,
}

/// Risk distribution
#[derive(Debug, Serialize)]
pub struct RiskDistribution {
    pub low_risk: i32,
    pub medium_risk: i32,
    pub high_risk: i32,
    pub critical_risk: i32,
    pub total: i32,
}

/// Audit timeline entry
#[derive(Debug, Serialize)]
pub struct AuditTimelineEntry {
    pub date: String,
    pub audits_completed: i32,
    pub average_risk_score: f64,
    pub critical_findings: i32,
    pub high_findings: i32,
}

/// Batch audit request
#[derive(Debug, Deserialize, Validate)]
pub struct BatchAuditRequest {
    pub requests: Vec<CreateAiAuditRequest>,
    pub priority: Option<String>,
    pub notify_on_completion: Option<bool>,
}

/// Batch audit response
#[derive(Debug, Serialize)]
pub struct BatchAuditResponse {
    pub submitted_audits: Vec<AiAuditReportResponse>,
    pub failed_submissions: Vec<BatchAuditError>,
    pub batch_id: Uuid,
    pub total_submitted: i32,
    pub total_failed: i32,
}

/// Batch audit error
#[derive(Debug, Serialize)]
pub struct BatchAuditError {
    pub index: usize,
    pub request: CreateAiAuditRequest,
    pub error: String,
}

/// Audit notification settings
#[derive(Debug, Serialize, Deserialize)]
pub struct AuditNotificationSettings {
    pub notify_on_completion: bool,
    pub notify_on_high_risk: bool,
    pub notify_on_critical_findings: bool,
    pub email_notifications: bool,
    pub webhook_url: Option<String>,
}

/// Audit webhook payload
#[derive(Debug, Serialize)]
pub struct AuditWebhookPayload {
    pub event_type: String,
    pub report_id: Uuid,
    pub status: AiAuditStatus,
    pub risk_score: Option<f64>,
    pub findings_count: i32,
    pub critical_findings_count: i32,
    pub timestamp: DateTime<Utc>,
}

/// Audit export request
#[derive(Debug, Deserialize, Validate)]
pub struct AuditExportRequest {
    pub report_ids: Vec<Uuid>,
    pub format: String, // "json", "csv", "pdf"
    pub include_findings: Option<bool>,
    pub include_recommendations: Option<bool>,
}

/// Audit export response
#[derive(Debug, Serialize)]
pub struct AuditExportResponse {
    pub download_url: String,
    pub expires_at: DateTime<Utc>,
    pub format: String,
    pub file_size: u64,
}

/// Audit comparison request
#[derive(Debug, Deserialize, Validate)]
pub struct AuditComparisonRequest {
    pub report_id_1: Uuid,
    pub report_id_2: Uuid,
    pub comparison_type: String,
}

/// Audit comparison response
#[derive(Debug, Serialize)]
pub struct AuditComparisonResponse {
    pub report_1: AiAuditReportResponse,
    pub report_2: AiAuditReportResponse,
    pub comparison: AuditComparison,
}

/// Audit comparison data
#[derive(Debug, Serialize)]
pub struct AuditComparison {
    pub risk_score_diff: f64,
    pub confidence_score_diff: f64,
    pub findings_diff: FindingsDiff,
    pub recommendations_diff: RecommendationsDiff,
    pub improvement_areas: Vec<String>,
    pub regression_areas: Vec<String>,
}

/// Findings difference
#[derive(Debug, Serialize)]
pub struct FindingsDiff {
    pub added: Vec<SecurityFinding>,
    pub removed: Vec<SecurityFinding>,
    pub modified: Vec<SecurityFinding>,
    pub unchanged: Vec<SecurityFinding>,
}

/// Recommendations difference
#[derive(Debug, Serialize)]
pub struct RecommendationsDiff {
    pub added: Vec<AuditRecommendation>,
    pub removed: Vec<AuditRecommendation>,
    pub modified: Vec<AuditRecommendation>,
    pub unchanged: Vec<AuditRecommendation>,
}
