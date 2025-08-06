pub mod config;

pub mod error;
pub mod handlers;
pub mod infra;
pub mod models;
pub mod routes;

pub mod workers;

#[cfg(test)]
pub mod test_helpers;

// Re-export commonly used types
pub use config::Config;

// Re-export error types from avinapi
pub use avinapi::prelude::{AppError, AppResult as Result};

// Re-export common crate functionality
pub use common::*;

use regex::Regex;
use std::sync::LazyLock;

/// Ethereum address validation regex (0x followed by 40 hex characters)
pub static ETHEREUM_ADDRESS_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^0x[a-fA-F0-9]{40}$").unwrap());

/// GitHub URL validation regex
pub static GITHUB_URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^https://github\.com/[a-zA-Z0-9_-]+/[a-zA-Z0-9_.-]+/?$").unwrap()
});

/// Git commit hash validation regex (7-40 hex characters)
pub static GIT_COMMIT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-fA-F0-9]{7,40}$").unwrap());

/// IPFS CID validation regex (supports both CIDv0 and CIDv1)
pub static IPFS_CID_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(Qm[1-9A-HJ-NP-Za-km-z]{44}|b[A-Za-z2-7]{58}|B[A-Z2-7]{58}|z[1-9A-HJ-NP-Za-km-z]{48}|F[0-9A-F]{50})$").unwrap()
});
