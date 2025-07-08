//! Algorithm execution data models
//!
//! This module contains shared data structures for algorithm execution
//! across all services in the DeLong Protocol.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Request payload for submitting algorithm execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct AlgoExeSubmissionRequest {
    /// GitHub repository URL containing the algorithm
    pub github_repo: String,

    /// Git commit hash for reproducible algorithm execution
    pub commit_hash: String,

    /// Wallet address of the scientist submitting the algorithm
    pub scientist_wallet: String,

    /// Dataset identifier to be used for algorithm execution
    pub dataset: String,
}

/// Algorithm execution data model
///
/// Represents the complete state and metadata of an algorithm execution
/// throughout its lifecycle from submission to completion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct AlgoExeData {
    /// Unique identifier for the algorithm execution
    pub id: u64,

    /// Algorithm identifier
    pub algo_id: String,

    /// Dataset used for this execution
    pub used_dataset: String,

    /// Wallet address of the scientist who submitted the algorithm
    pub scientist_wallet: String,

    /// Current review status (pending, approved, rejected)
    pub review_status: String,

    /// Voting period start time (ISO 8601 format)
    pub vote_start_time: Option<String>,

    /// Voting period end time (ISO 8601 format)
    pub vote_end_time: Option<String>,

    /// Execution status (queued, running, completed, failed)
    pub status: String,

    /// Algorithm execution start time (ISO 8601 format)
    pub start_time: Option<String>,

    /// Algorithm execution end time (ISO 8601 format)
    pub end_time: Option<String>,

    /// Execution result data (JSON or URL to result)
    pub result: Option<String>,

    /// Error message if execution failed
    pub error_msg: Option<String>,

    /// Record creation timestamp (ISO 8601 format)
    pub created_at: String,

    /// Record last update timestamp (ISO 8601 format)
    pub updated_at: String,

    /// Human-readable algorithm name
    pub algo_name: Option<String>,

    /// Link to algorithm source code
    pub algo_link: Option<String>,

    /// IPFS Content Identifier for algorithm data
    pub cid: Option<String>,
}

/// Response for algorithm execution submission
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct AlgoExeSubmissionResponse {
    /// Unique identifier assigned to the submitted execution
    pub id: u64,
}

/// Algorithm execution status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AlgoExeStatus {
    /// Execution is queued and waiting to start
    Queued,

    /// Algorithm is currently running
    Running,

    /// Execution completed successfully
    Completed,

    /// Execution failed with error
    Failed,

    /// Execution was cancelled
    Cancelled,
}

impl std::fmt::Display for AlgoExeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlgoExeStatus::Queued => write!(f, "queued"),
            AlgoExeStatus::Running => write!(f, "running"),
            AlgoExeStatus::Completed => write!(f, "completed"),
            AlgoExeStatus::Failed => write!(f, "failed"),
            AlgoExeStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Algorithm review status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AlgoReviewStatus {
    /// Under committee review
    Pending,

    /// Approved by committee
    Approved,

    /// Rejected by committee
    Rejected,
}

impl std::fmt::Display for AlgoReviewStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlgoReviewStatus::Pending => write!(f, "pending"),
            AlgoReviewStatus::Approved => write!(f, "approved"),
            AlgoReviewStatus::Rejected => write!(f, "rejected"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let deserialized: AlgoExeSubmissionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, deserialized);
    }

    #[test]
    fn test_algo_exe_data_serialization() {
        let data = AlgoExeData {
            id: 1,
            algo_id: "algo_123".to_string(),
            used_dataset: "dataset_001".to_string(),
            scientist_wallet: "0x1234567890abcdef".to_string(),
            review_status: "pending".to_string(),
            vote_start_time: None,
            vote_end_time: None,
            status: "running".to_string(),
            start_time: Some("2023-01-01T00:00:00Z".to_string()),
            end_time: None,
            result: None,
            error_msg: None,
            created_at: "2023-01-01T00:00:00Z".to_string(),
            updated_at: "2023-01-01T00:00:00Z".to_string(),
            algo_name: Some("Test Algorithm".to_string()),
            algo_link: Some("https://github.com/user/repo".to_string()),
            cid: Some("QmTest123".to_string()),
        };

        let json = serde_json::to_string(&data).unwrap();
        let deserialized: AlgoExeData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_algo_exe_submission_response_serialization() {
        let response = AlgoExeSubmissionResponse { id: 123 };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("123"));

        let deserialized: AlgoExeSubmissionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response, deserialized);
    }

    #[test]
    fn test_algo_exe_status_enum() {
        let status = AlgoExeStatus::Running;
        assert_eq!(status.to_string(), "running");

        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");

        let deserialized: AlgoExeStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, deserialized);
    }

    #[test]
    fn test_algo_review_status_enum() {
        let status = AlgoReviewStatus::Approved;
        assert_eq!(status.to_string(), "approved");

        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"approved\"");

        let deserialized: AlgoReviewStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, deserialized);
    }

    #[test]
    fn test_algo_exe_status_all_variants() {
        let statuses = vec![
            AlgoExeStatus::Queued,
            AlgoExeStatus::Running,
            AlgoExeStatus::Completed,
            AlgoExeStatus::Failed,
            AlgoExeStatus::Cancelled,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: AlgoExeStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn test_algo_review_status_all_variants() {
        let statuses = vec![
            AlgoReviewStatus::Pending,
            AlgoReviewStatus::Approved,
            AlgoReviewStatus::Rejected,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: AlgoReviewStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, deserialized);
        }
    }
}
