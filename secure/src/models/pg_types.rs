//! PostgreSQL custom type mappings for SQLx
//!
//! This module provides proper PostgreSQL ENUM type definitions for SQLx.
//! PostgreSQL has native ENUM support, so we should use it directly rather than
//! converting to/from strings at the application layer.

use serde::{Deserialize, Serialize};
use sqlx::Type;

/// Algorithm review status - maps to PostgreSQL enum type 'review_status'
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "review_status", rename_all = "UPPERCASE")]
#[serde(rename_all = "UPPERCASE")]
pub enum ReviewStatus {
    Reviewing,
    Approved,
    Rejected,
}

/// Algorithm execution status - maps to PostgreSQL enum type 'execution_status'
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "execution_status", rename_all = "UPPERCASE")]
#[serde(rename_all = "UPPERCASE")]
pub enum ExecutionStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

/// Transaction status - maps to PostgreSQL enum type 'transaction_status'
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "transaction_status", rename_all = "UPPERCASE")]
#[serde(rename_all = "UPPERCASE")]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
}

/// Test status - maps to PostgreSQL enum type 'test_status'
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "test_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TestStatus {
    AboveRange,
    BelowRange,
    WithinRange,
    Unknown,
}

// Re-export these types for convenience
pub use ExecutionStatus as AlgoExeStatus;
pub use ReviewStatus as AlgoReviewStatus;
