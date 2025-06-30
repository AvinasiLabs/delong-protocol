//! Test report management data models
//!
//! This module contains all data structures related to test report management,
//! including report submission, status tracking, and report metadata.

use serde::{Deserialize, Serialize};

/// Report type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReportType {
    /// Algorithm execution test report
    #[serde(rename = "algorithm_test")]
    AlgorithmTest,
    /// Dataset validation report
    #[serde(rename = "dataset_validation")]
    DatasetValidation,
    /// Performance benchmark report
    #[serde(rename = "performance_benchmark")]
    PerformanceBenchmark,
    /// Security audit report
    #[serde(rename = "security_audit")]
    SecurityAudit,
    /// General test report
    #[serde(rename = "general_test")]
    GeneralTest,
    /// Custom report type
    #[serde(rename = "custom")]
    Custom(String),
}

/// Report status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReportStatus {
    /// Report is being uploaded
    #[serde(rename = "uploading")]
    Uploading,
    /// Report is being processed
    #[serde(rename = "processing")]
    Processing,
    /// Report has been processed successfully
    #[serde(rename = "completed")]
    Completed,
    /// Report processing failed
    #[serde(rename = "failed")]
    Failed,
    /// Report is archived
    #[serde(rename = "archived")]
    Archived,
}

/// Request payload for uploading test report
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UploadReportRequest {
    /// The report content/data
    pub report_data: String,
    /// Type of the report
    pub report_type: String,
    /// Associated algorithm ID (optional)
    pub algorithm_id: Option<String>,
    /// Associated dataset ID (optional)
    pub dataset_id: Option<String>,
    /// Optional report title
    pub title: Option<String>,
    /// Optional report description
    pub description: Option<String>,
    /// Tags for categorizing the report (optional)
    pub tags: Option<Vec<String>>,
}

/// Response for test report upload
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UploadReportResponse {
    /// Unique identifier for the uploaded report
    pub report_id: String,
    /// Timestamp when the report was uploaded
    pub upload_time: String,
    /// Current status of the report
    pub status: String,
}

/// Detailed report information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReportInfo {
    /// Unique identifier for the report
    pub id: String,
    /// Report title
    pub title: String,
    /// Report type
    pub report_type: ReportType,
    /// Report status
    pub status: ReportStatus,
    /// Optional description
    pub description: Option<String>,
    /// Associated algorithm ID (if applicable)
    pub algorithm_id: Option<String>,
    /// Associated dataset ID (if applicable)
    pub dataset_id: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Report file size in bytes
    pub file_size: u64,
    /// Content type/format (e.g., "application/json", "text/plain")
    pub content_type: String,
    /// Upload timestamp in RFC3339 format
    pub uploaded_at: String,
    /// Processing completion timestamp (if completed)
    pub completed_at: Option<String>,
    /// User/entity that uploaded the report
    pub uploaded_by: Option<String>,
}

/// Request for querying reports
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReportQuery {
    /// Filter by report type (optional)
    pub report_type: Option<ReportType>,
    /// Filter by status (optional)
    pub status: Option<ReportStatus>,
    /// Filter by algorithm ID (optional)
    pub algorithm_id: Option<String>,
    /// Filter by dataset ID (optional)
    pub dataset_id: Option<String>,
    /// Filter by tags (optional)
    pub tags: Option<Vec<String>>,
    /// Filter by upload date range - start (optional)
    pub uploaded_after: Option<String>,
    /// Filter by upload date range - end (optional)
    pub uploaded_before: Option<String>,
    /// Page number for pagination
    pub page: u32,
    /// Number of items per page
    pub limit: u32,
}

/// Report summary for listing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReportSummary {
    /// Report ID
    pub id: String,
    /// Report title
    pub title: String,
    /// Report type as string
    pub report_type: String,
    /// Report status as string
    pub status: String,
    /// Upload timestamp
    pub uploaded_at: String,
    /// File size in bytes
    pub file_size: u64,
}

impl ReportType {
    /// Get all available report types
    pub fn all() -> Vec<ReportType> {
        vec![
            ReportType::AlgorithmTest,
            ReportType::DatasetValidation,
            ReportType::PerformanceBenchmark,
            ReportType::SecurityAudit,
            ReportType::GeneralTest,
        ]
    }

    /// Convert string to report type
    pub fn from_string(s: &str) -> ReportType {
        match s.to_lowercase().as_str() {
            "algorithm_test" => ReportType::AlgorithmTest,
            "dataset_validation" => ReportType::DatasetValidation,
            "performance_benchmark" => ReportType::PerformanceBenchmark,
            "security_audit" => ReportType::SecurityAudit,
            "general_test" => ReportType::GeneralTest,
            other => ReportType::Custom(other.to_string()),
        }
    }

    /// Convert report type to string
    pub fn to_string(&self) -> String {
        match self {
            ReportType::AlgorithmTest => "algorithm_test".to_string(),
            ReportType::DatasetValidation => "dataset_validation".to_string(),
            ReportType::PerformanceBenchmark => "performance_benchmark".to_string(),
            ReportType::SecurityAudit => "security_audit".to_string(),
            ReportType::GeneralTest => "general_test".to_string(),
            ReportType::Custom(custom) => custom.clone(),
        }
    }

    /// Check if the report type requires algorithm association
    pub fn requires_algorithm(&self) -> bool {
        matches!(
            self,
            ReportType::AlgorithmTest | ReportType::PerformanceBenchmark
        )
    }

    /// Check if the report type requires dataset association
    pub fn requires_dataset(&self) -> bool {
        matches!(self, ReportType::DatasetValidation)
    }
}

impl ReportStatus {
    /// Check if the report is in a final state
    pub fn is_final(&self) -> bool {
        matches!(
            self,
            ReportStatus::Completed | ReportStatus::Failed | ReportStatus::Archived
        )
    }

    /// Check if the report is still processing
    pub fn is_processing(&self) -> bool {
        matches!(self, ReportStatus::Uploading | ReportStatus::Processing)
    }

    /// Check if the report completed successfully
    pub fn is_successful(&self) -> bool {
        matches!(self, ReportStatus::Completed)
    }
}

impl UploadReportRequest {
    /// Create a new report upload request
    pub fn new(report_data: String, report_type: String) -> Self {
        Self {
            report_data,
            report_type,
            algorithm_id: None,
            dataset_id: None,
            title: None,
            description: None,
            tags: None,
        }
    }

    /// Create a new report upload request with all fields
    pub fn new_full(
        report_data: String,
        report_type: String,
        algorithm_id: Option<String>,
        dataset_id: Option<String>,
        title: Option<String>,
        description: Option<String>,
        tags: Option<Vec<String>>,
    ) -> Self {
        Self {
            report_data,
            report_type,
            algorithm_id,
            dataset_id,
            title,
            description,
            tags,
        }
    }

    /// Validate the upload request
    pub fn validate(&self) -> Result<(), String> {
        if self.report_data.trim().is_empty() {
            return Err("Report data cannot be empty".to_string());
        }

        if self.report_data.len() > 10_000_000 {
            // 10MB limit
            return Err("Report data cannot exceed 10MB".to_string());
        }

        if self.report_type.trim().is_empty() {
            return Err("Report type cannot be empty".to_string());
        }

        if let Some(title) = &self.title {
            if title.len() > 200 {
                return Err("Report title cannot exceed 200 characters".to_string());
            }
        }

        if let Some(description) = &self.description {
            if description.len() > 2000 {
                return Err("Report description cannot exceed 2000 characters".to_string());
            }
        }

        if let Some(tags) = &self.tags {
            if tags.len() > 20 {
                return Err("Cannot have more than 20 tags".to_string());
            }
            for tag in tags {
                if tag.len() > 50 {
                    return Err("Each tag cannot exceed 50 characters".to_string());
                }
            }
        }

        // Validate report type specific requirements
        let report_type = ReportType::from_string(&self.report_type);
        if report_type.requires_algorithm() && self.algorithm_id.is_none() {
            return Err(format!(
                "Report type '{}' requires an algorithm ID",
                self.report_type
            ));
        }

        if report_type.requires_dataset() && self.dataset_id.is_none() {
            return Err(format!(
                "Report type '{}' requires a dataset ID",
                self.report_type
            ));
        }

        Ok(())
    }

    /// Get the report type as enum
    pub fn get_report_type(&self) -> ReportType {
        ReportType::from_string(&self.report_type)
    }
}

impl UploadReportResponse {
    /// Create a new upload report response
    pub fn new(report_id: String, upload_time: String, status: String) -> Self {
        Self {
            report_id,
            upload_time,
            status,
        }
    }

    /// Get the status as enum
    pub fn get_status(&self) -> ReportStatus {
        match self.status.to_lowercase().as_str() {
            "uploading" => ReportStatus::Uploading,
            "processing" => ReportStatus::Processing,
            "completed" => ReportStatus::Completed,
            "failed" => ReportStatus::Failed,
            "archived" => ReportStatus::Archived,
            _ => ReportStatus::Processing, // Default fallback
        }
    }
}

impl ReportInfo {
    /// Create a new report info instance
    pub fn new(
        id: String,
        title: String,
        report_type: ReportType,
        status: ReportStatus,
        uploaded_at: String,
    ) -> Self {
        Self {
            id,
            title,
            report_type,
            status,
            description: None,
            algorithm_id: None,
            dataset_id: None,
            tags: Vec::new(),
            file_size: 0,
            content_type: "text/plain".to_string(),
            uploaded_at,
            completed_at: None,
            uploaded_by: None,
        }
    }

    /// Check if the report is associated with an algorithm
    pub fn has_algorithm(&self) -> bool {
        self.algorithm_id.is_some()
    }

    /// Check if the report is associated with a dataset
    pub fn has_dataset(&self) -> bool {
        self.dataset_id.is_some()
    }

    /// Get a human-readable file size
    pub fn formatted_file_size(&self) -> String {
        let size = self.file_size as f64;
        if size < 1024.0 {
            format!("{} B", size)
        } else if size < 1024.0 * 1024.0 {
            format!("{:.1} KB", size / 1024.0)
        } else if size < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.1} MB", size / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", size / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

impl ReportQuery {
    /// Create a new report query with defaults
    pub fn new() -> Self {
        Self {
            report_type: None,
            status: None,
            algorithm_id: None,
            dataset_id: None,
            tags: None,
            uploaded_after: None,
            uploaded_before: None,
            page: 1,
            limit: 20,
        }
    }
}

impl Default for ReportQuery {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_type_conversions() {
        assert_eq!(
            ReportType::from_string("algorithm_test"),
            ReportType::AlgorithmTest
        );
        assert_eq!(
            ReportType::from_string("custom_type"),
            ReportType::Custom("custom_type".to_string())
        );

        assert_eq!(
            ReportType::AlgorithmTest.to_string(),
            "algorithm_test".to_string()
        );
    }

    #[test]
    fn test_report_type_requirements() {
        assert!(ReportType::AlgorithmTest.requires_algorithm());
        assert!(!ReportType::AlgorithmTest.requires_dataset());
        assert!(ReportType::DatasetValidation.requires_dataset());
        assert!(!ReportType::DatasetValidation.requires_algorithm());
        assert!(!ReportType::GeneralTest.requires_algorithm());
        assert!(!ReportType::GeneralTest.requires_dataset());
    }

    #[test]
    fn test_report_status_checks() {
        assert!(ReportStatus::Completed.is_final());
        assert!(ReportStatus::Failed.is_final());
        assert!(!ReportStatus::Processing.is_final());

        assert!(ReportStatus::Processing.is_processing());
        assert!(ReportStatus::Uploading.is_processing());
        assert!(!ReportStatus::Completed.is_processing());

        assert!(ReportStatus::Completed.is_successful());
        assert!(!ReportStatus::Failed.is_successful());
    }

    #[test]
    fn test_upload_report_request_creation() {
        let request =
            UploadReportRequest::new("test report data".to_string(), "algorithm_test".to_string());

        assert_eq!(request.report_data, "test report data");
        assert_eq!(request.report_type, "algorithm_test");
        assert!(request.algorithm_id.is_none());
        assert!(request.dataset_id.is_none());
    }

    #[test]
    fn test_upload_report_request_validation() {
        // Valid request
        let valid_request =
            UploadReportRequest::new("valid report data".to_string(), "general_test".to_string());
        assert!(valid_request.validate().is_ok());

        // Empty report data
        let empty_data = UploadReportRequest::new("".to_string(), "general_test".to_string());
        assert!(empty_data.validate().is_err());

        // Empty report type
        let empty_type = UploadReportRequest::new("valid data".to_string(), "".to_string());
        assert!(empty_type.validate().is_err());

        // Algorithm test without algorithm ID
        let algo_test_no_id =
            UploadReportRequest::new("valid data".to_string(), "algorithm_test".to_string());
        assert!(algo_test_no_id.validate().is_err());

        // Algorithm test with algorithm ID
        let mut algo_test_with_id =
            UploadReportRequest::new("valid data".to_string(), "algorithm_test".to_string());
        algo_test_with_id.algorithm_id = Some("algo_123".to_string());
        assert!(algo_test_with_id.validate().is_ok());
    }

    #[test]
    fn test_upload_report_response() {
        let response = UploadReportResponse::new(
            "report_123".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
            "processing".to_string(),
        );

        assert_eq!(response.report_id, "report_123");
        assert_eq!(response.get_status(), ReportStatus::Processing);
    }

    #[test]
    fn test_report_info_creation() {
        let info = ReportInfo::new(
            "report_456".to_string(),
            "Test Report".to_string(),
            ReportType::AlgorithmTest,
            ReportStatus::Completed,
            "2023-01-01T00:00:00Z".to_string(),
        );

        assert_eq!(info.id, "report_456");
        assert_eq!(info.title, "Test Report");
        assert!(!info.has_algorithm());
        assert!(!info.has_dataset());
    }

    #[test]
    fn test_report_info_formatted_file_size() {
        let mut info = ReportInfo::new(
            "report_123".to_string(),
            "Test".to_string(),
            ReportType::GeneralTest,
            ReportStatus::Completed,
            "2023-01-01T00:00:00Z".to_string(),
        );

        info.file_size = 1536; // 1.5 KB
        assert_eq!(info.formatted_file_size(), "1.5 KB");

        info.file_size = 1048576; // 1 MB
        assert_eq!(info.formatted_file_size(), "1.0 MB");
    }

    #[test]
    fn test_serialization_deserialization() {
        let request =
            UploadReportRequest::new("test data".to_string(), "algorithm_test".to_string());

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: UploadReportRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, deserialized);

        let response = UploadReportResponse::new(
            "report_123".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
            "completed".to_string(),
        );

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: UploadReportResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response, deserialized);
    }
}
