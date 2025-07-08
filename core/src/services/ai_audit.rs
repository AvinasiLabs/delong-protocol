//! AI Audit Service Module
//!
//! This module provides functionality for integrating with external AI audit services,
//! generating mock audit data for testing, and calculating audit scores based on
//! code analysis results.

use crate::errors::{AppError, AppResult};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn};

/// AI Audit Service for handling code audit operations
pub struct AiAuditService {
    client: Client,
    service_url: Option<String>,
    timeout: Duration,
}

/// External AI audit service response
#[derive(Debug, Deserialize, Serialize)]
pub struct ExternalAuditResponse {
    pub status: String,
    pub results: Vec<AuditFileResult>,
    pub metadata: Option<AuditMetadata>,
}

/// Individual file audit result
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AuditFileResult {
    pub status: String,
    pub file_path: String,
    pub confidence: f64,
    pub breaches: Option<Vec<SecurityBreach>>,
}

/// Security breach information
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SecurityBreach {
    pub code: String,
    pub reason: String,
    pub severity: Option<String>,
    pub line_number: Option<i32>,
}

/// Audit metadata
#[derive(Debug, Deserialize, Serialize)]
pub struct AuditMetadata {
    pub total_files: i32,
    pub processing_time_ms: i64,
    pub algorithm_version: String,
    pub timestamp: String,
}

impl AiAuditService {
    /// Create a new AI audit service instance
    pub fn new(service_url: Option<String>) -> AppResult<Self> {
        let timeout = Duration::from_secs(120);
        let client = Client::builder()
            .timeout(timeout) // 2 minutes timeout for AI processing
            .build()
            .map_err(|e| AppError::SystemError {
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        Ok(Self {
            client,
            service_url,
            timeout,
        })
    }

    /// Build repository download URL from GitHub URL and commit hash
    pub fn build_repo_url(github_url: &str, commit_hash: &str) -> AppResult<String> {
        // Validate GitHub URL format
        if !github_url.contains("github.com") {
            return Err(AppError::InvalidRequest {
                message: "Invalid GitHub URL format".to_string(),
            });
        }

        // Extract user and repository name from GitHub URL
        // Support formats: https://github.com/user/repo, https://github.com/user/repo.git
        let url_parts: Vec<&str> = github_url.split('/').collect();
        if url_parts.len() < 5 {
            return Err(AppError::InvalidRequest {
                message: "Invalid GitHub URL format - cannot extract user/repo".to_string(),
            });
        }

        let user = url_parts[url_parts.len() - 2];
        let repo = url_parts[url_parts.len() - 1]
            .trim_end_matches(".git")
            .trim_end_matches('/');

        // Validate commit hash format (40 characters hex)
        if commit_hash.len() != 40 || !commit_hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(AppError::InvalidRequest {
                message: "Invalid commit hash format".to_string(),
            });
        }

        // Build codeload URL for direct tar.gz download
        let repo_url = format!(
            "https://codeload.github.com/{}/{}/tar.gz/{}",
            user, repo, commit_hash
        );

        info!("Built repository URL: {}", repo_url);
        Ok(repo_url)
    }

    /// Call external AI audit service
    pub async fn call_external_audit_service(
        &self,
        repo_url: &str,
    ) -> AppResult<Vec<AuditFileResult>> {
        let service_url = self
            .service_url
            .as_ref()
            .ok_or_else(|| AppError::ConfigError {
                message: "AI_AUDIT_SERVICE_URL not configured".to_string(),
            })?;

        let request_body = serde_json::json!({
            "repo_url": repo_url
        });

        info!("Calling external AI audit service: {}", service_url);

        let response = self
            .client
            .post(format!("{}/api/delong/v1/code_audit/audit", service_url))
            .json(&request_body)
            .timeout(self.timeout) // Use configured timeout
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError {
                service: "AI Audit Service".to_string(),
                message: format!("Failed to call AI audit service: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::ExternalServiceError {
                service: "AI Audit Service".to_string(),
                message: format!("AI audit service returned error {}: {}", status, error_text),
            });
        }

        let audit_response: ExternalAuditResponse =
            response
                .json()
                .await
                .map_err(|e| AppError::ExternalServiceError {
                    service: "AI Audit Service".to_string(),
                    message: format!("Failed to parse AI audit response: {}", e),
                })?;

        info!(
            "AI audit service returned {} file results",
            audit_response.results.len()
        );

        Ok(audit_response.results)
    }

    /// Generate mock audit result for testing
    pub fn generate_mock_audit_result() -> Vec<AuditFileResult> {
        vec![
            AuditFileResult {
                status: "ACCEPT".to_string(),
                file_path: "src/main.rs".to_string(),
                confidence: 0.95,
                breaches: None,
            },
            AuditFileResult {
                status: "ACCEPT".to_string(),
                file_path: "src/lib.rs".to_string(),
                confidence: 0.88,
                breaches: None,
            },
            AuditFileResult {
                status: "ACCEPT".to_string(),
                file_path: "src/utils.rs".to_string(),
                confidence: 0.92,
                breaches: None,
            },
            AuditFileResult {
                status: "REJECT".to_string(),
                file_path: "src/debug.rs".to_string(),
                confidence: 0.96,
                breaches: Some(vec![
                    SecurityBreach {
                        code: "println!(\"{:?}\", user_data);".to_string(),
                        reason: "Potential data leakage: printing sensitive user data to console".to_string(),
                        severity: Some("HIGH".to_string()),
                        line_number: Some(42),
                    },
                    SecurityBreach {
                        code: "std::fs::write(\"/tmp/debug.log\", data)".to_string(),
                        reason: "Insecure file operations: writing sensitive data to predictable location".to_string(),
                        severity: Some("MEDIUM".to_string()),
                        line_number: Some(58),
                    },
                ]),
            },
            AuditFileResult {
                status: "ACCEPT".to_string(),
                file_path: "src/config.rs".to_string(),
                confidence: 0.85,
                breaches: None,
            },
            AuditFileResult {
                status: "REJECT".to_string(),
                file_path: "src/network.rs".to_string(),
                confidence: 0.91,
                breaches: Some(vec![
                    SecurityBreach {
                        code: "reqwest::get(&url).await?.text().await?".to_string(),
                        reason: "Potential SSRF vulnerability: unvalidated URL in HTTP request".to_string(),
                        severity: Some("HIGH".to_string()),
                        line_number: Some(127),
                    },
                ]),
            },
        ]
    }

    /// Calculate audit score based on audit results
    pub fn calculate_audit_score(audit_results: &[AuditFileResult]) -> i32 {
        if audit_results.is_empty() {
            return 0;
        }

        let total_files = audit_results.len();
        let accepted_files = audit_results
            .iter()
            .filter(|r| r.status == "ACCEPT")
            .count();
        let _rejected_files = total_files - accepted_files;

        // Base score calculation: percentage of accepted files
        let base_score = (accepted_files as f64 / total_files as f64) * 100.0;

        // Apply confidence weighting
        let mut weighted_score = 0.0;
        let mut total_weight = 0.0;

        for result in audit_results {
            let weight = result.confidence;
            let file_score = if result.status == "ACCEPT" {
                100.0
            } else {
                0.0
            };

            // Apply severity penalty for rejected files
            let severity_penalty = if result.status == "REJECT" {
                if let Some(breaches) = &result.breaches {
                    let high_severity_count = breaches
                        .iter()
                        .filter(|b| b.severity.as_ref().map_or(false, |s| s == "HIGH"))
                        .count();
                    let medium_severity_count = breaches
                        .iter()
                        .filter(|b| b.severity.as_ref().map_or(false, |s| s == "MEDIUM"))
                        .count();

                    // Higher penalty for high severity issues
                    high_severity_count as f64 * 20.0 + medium_severity_count as f64 * 10.0
                } else {
                    10.0 // Default penalty for rejected files without breach details
                }
            } else {
                0.0
            };

            weighted_score += (file_score - severity_penalty) * weight;
            total_weight += weight;
        }

        let final_score = if total_weight > 0.0 {
            let avg_weighted_score = weighted_score / total_weight;
            // Combine base score and weighted score
            (base_score * 0.3 + avg_weighted_score * 0.7).max(0.0)
        } else {
            base_score
        };

        // Round to nearest integer and clamp between 0 and 100
        (final_score.round() as i32).clamp(0, 100)
    }

    /// Validate audit result format
    pub fn validate_audit_result(results: &[AuditFileResult]) -> AppResult<()> {
        if results.is_empty() {
            return Err(AppError::InvalidRequest {
                message: "Audit results cannot be empty".to_string(),
            });
        }

        for (index, result) in results.iter().enumerate() {
            // Validate status
            if !["ACCEPT", "REJECT"].contains(&result.status.as_str()) {
                return Err(AppError::InvalidRequest {
                    message: format!("Invalid status '{}' at index {}", result.status, index),
                });
            }

            // Validate file path
            if result.file_path.is_empty() {
                return Err(AppError::InvalidRequest {
                    message: format!("Empty file path at index {}", index),
                });
            }

            // Validate confidence range
            if result.confidence < 0.0 || result.confidence > 1.0 {
                return Err(AppError::InvalidRequest {
                    message: format!(
                        "Invalid confidence {} at index {}, must be between 0.0 and 1.0",
                        result.confidence, index
                    ),
                });
            }

            // Validate that rejected files should have breaches
            if result.status == "REJECT" && result.breaches.is_none() {
                warn!(
                    "Rejected file '{}' has no breach information",
                    result.file_path
                );
            }
        }

        Ok(())
    }

    /// Get audit summary statistics
    pub fn get_audit_summary(results: &[AuditFileResult]) -> AuditSummary {
        let total_files = results.len();
        let accepted_files = results.iter().filter(|r| r.status == "ACCEPT").count();
        let rejected_files = total_files - accepted_files;

        let total_breaches = results
            .iter()
            .filter_map(|r| r.breaches.as_ref())
            .map(|b| b.len())
            .sum::<usize>();

        let high_severity_breaches = results
            .iter()
            .filter_map(|r| r.breaches.as_ref())
            .flatten()
            .filter(|b| b.severity.as_ref().map_or(false, |s| s == "HIGH"))
            .count();

        let medium_severity_breaches = results
            .iter()
            .filter_map(|r| r.breaches.as_ref())
            .flatten()
            .filter(|b| b.severity.as_ref().map_or(false, |s| s == "MEDIUM"))
            .count();

        let low_severity_breaches = results
            .iter()
            .filter_map(|r| r.breaches.as_ref())
            .flatten()
            .filter(|b| b.severity.as_ref().map_or(false, |s| s == "LOW"))
            .count();

        let avg_confidence = if total_files > 0 {
            results.iter().map(|r| r.confidence).sum::<f64>() / total_files as f64
        } else {
            0.0
        };

        AuditSummary {
            total_files,
            accepted_files,
            rejected_files,
            total_breaches,
            high_severity_breaches,
            medium_severity_breaches,
            low_severity_breaches,
            avg_confidence,
            score: Self::calculate_audit_score(results),
        }
    }
}

/// Audit summary statistics
#[derive(Debug, Serialize)]
pub struct AuditSummary {
    pub total_files: usize,
    pub accepted_files: usize,
    pub rejected_files: usize,
    pub total_breaches: usize,
    pub high_severity_breaches: usize,
    pub medium_severity_breaches: usize,
    pub low_severity_breaches: usize,
    pub avg_confidence: f64,
    pub score: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_repo_url_valid() {
        let github_url = "https://github.com/user/repo";
        let commit_hash = "1234567890abcdef1234567890abcdef12345678";

        let result = AiAuditService::build_repo_url(github_url, commit_hash);
        assert!(result.is_ok());

        let url = result.unwrap();
        assert_eq!(
            url,
            "https://codeload.github.com/user/repo/tar.gz/1234567890abcdef1234567890abcdef12345678"
        );
    }

    #[test]
    fn test_build_repo_url_with_git_extension() {
        let github_url = "https://github.com/user/repo.git";
        let commit_hash = "1234567890abcdef1234567890abcdef12345678";

        let result = AiAuditService::build_repo_url(github_url, commit_hash);
        assert!(result.is_ok());

        let url = result.unwrap();
        assert_eq!(
            url,
            "https://codeload.github.com/user/repo/tar.gz/1234567890abcdef1234567890abcdef12345678"
        );
    }

    #[test]
    fn test_build_repo_url_invalid_github_url() {
        let github_url = "https://invalid.com/user/repo";
        let commit_hash = "1234567890abcdef1234567890abcdef12345678";

        let result = AiAuditService::build_repo_url(github_url, commit_hash);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_repo_url_invalid_commit_hash() {
        let github_url = "https://github.com/user/repo";
        let commit_hash = "invalid_hash";

        let result = AiAuditService::build_repo_url(github_url, commit_hash);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_audit_score_all_accepted() {
        let results = vec![
            AuditFileResult {
                status: "ACCEPT".to_string(),
                file_path: "file1.rs".to_string(),
                confidence: 0.9,
                breaches: None,
            },
            AuditFileResult {
                status: "ACCEPT".to_string(),
                file_path: "file2.rs".to_string(),
                confidence: 0.95,
                breaches: None,
            },
        ];

        let score = AiAuditService::calculate_audit_score(&results);
        assert_eq!(score, 100);
    }

    #[test]
    fn test_calculate_audit_score_mixed_results() {
        let results = vec![
            AuditFileResult {
                status: "ACCEPT".to_string(),
                file_path: "file1.rs".to_string(),
                confidence: 0.9,
                breaches: None,
            },
            AuditFileResult {
                status: "REJECT".to_string(),
                file_path: "file2.rs".to_string(),
                confidence: 0.95,
                breaches: Some(vec![SecurityBreach {
                    code: "test".to_string(),
                    reason: "test reason".to_string(),
                    severity: Some("HIGH".to_string()),
                    line_number: Some(1),
                }]),
            },
        ];

        let score = AiAuditService::calculate_audit_score(&results);
        assert!(score < 100);
        assert!(score >= 0);
    }

    #[test]
    fn test_validate_audit_result_valid() {
        let results = vec![AuditFileResult {
            status: "ACCEPT".to_string(),
            file_path: "file1.rs".to_string(),
            confidence: 0.9,
            breaches: None,
        }];

        let result = AiAuditService::validate_audit_result(&results);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_audit_result_empty() {
        let results = vec![];
        let result = AiAuditService::validate_audit_result(&results);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_audit_result_invalid_status() {
        let results = vec![AuditFileResult {
            status: "INVALID".to_string(),
            file_path: "file1.rs".to_string(),
            confidence: 0.9,
            breaches: None,
        }];

        let result = AiAuditService::validate_audit_result(&results);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_mock_audit_result() {
        let results = AiAuditService::generate_mock_audit_result();
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.status == "ACCEPT"));
        assert!(results.iter().any(|r| r.status == "REJECT"));
    }

    #[test]
    fn test_get_audit_summary() {
        let results = AiAuditService::generate_mock_audit_result();
        let summary = AiAuditService::get_audit_summary(&results);

        assert_eq!(summary.total_files, results.len());
        assert!(summary.accepted_files > 0);
        assert!(summary.rejected_files > 0);
        assert!(summary.total_breaches > 0);
        assert!(summary.score >= 0 && summary.score <= 100);
    }
}
