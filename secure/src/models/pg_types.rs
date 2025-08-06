//! PostgreSQL custom type mappings for SQLx
//!
//! This module provides proper PostgreSQL ENUM type definitions for SQLx.
//! PostgreSQL has native ENUM support, so we should use it directly rather than
//! converting to/from strings at the application layer.

use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::fmt;

/// Algorithm review status - maps to PostgreSQL enum type 'review_status'
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "algo_review_status", rename_all = "lowercase")]
#[serde(rename_all = "UPPERCASE")]
pub enum ReviewStatus {
    Reviewing,
    Approved,
    Rejected,
}

impl fmt::Display for ReviewStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReviewStatus::Reviewing => write!(f, "REVIEWING"),
            ReviewStatus::Approved => write!(f, "APPROVED"),
            ReviewStatus::Rejected => write!(f, "REJECTED"),
        }
    }
}

/// Algorithm execution status - maps to PostgreSQL enum type 'execution_status'
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "algo_exe_status", rename_all = "lowercase")]
#[serde(rename_all = "UPPERCASE")]
pub enum ExecutionStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

impl fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionStatus::Queued => write!(f, "QUEUED"),
            ExecutionStatus::Running => write!(f, "RUNNING"),
            ExecutionStatus::Completed => write!(f, "COMPLETED"),
            ExecutionStatus::Failed => write!(f, "FAILED"),
        }
    }
}

/// Transaction status - maps to PostgreSQL enum type 'transaction_status'
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "transaction_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
}

// Re-export these types for convenience
pub use ExecutionStatus as AlgoExeStatus;
pub use ReviewStatus as AlgoReviewStatus;
