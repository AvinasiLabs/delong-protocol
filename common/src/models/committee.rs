//! Committee management data models
//!
//! This module contains all data structures related to committee member management,
//! including member operations, membership checks, and committee data.

use serde::{Deserialize, Serialize};

/// Request payload for setting committee member
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetCommitteeMemberRequest {
    /// Wallet address of the committee member
    pub member_wallet: String,
    /// Whether the member is approved
    pub is_approved: bool,
}

/// Committee member data model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitteeMemberData {
    /// Unique identifier for the committee member
    pub id: u64,
    /// Wallet address of the committee member
    pub member_wallet: String,
    /// Whether the member is approved
    pub is_approved: bool,
    /// Creation timestamp in RFC3339 format
    pub created_at: String,
    /// Last update timestamp in RFC3339 format
    pub updated_at: String,
}

/// Response for committee member operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitteeMemberResponse {
    /// ID of the created or updated committee member
    pub id: u64,
}

/// Response for committee membership check
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MembershipCheckResponse {
    /// Whether the wallet address is a committee member
    pub is_member: bool,
}

impl SetCommitteeMemberRequest {
    /// Create a new committee member request
    pub fn new(member_wallet: String, is_approved: bool) -> Self {
        Self {
            member_wallet,
            is_approved,
        }
    }

    /// Validate the request
    pub fn validate(&self) -> Result<(), String> {
        if self.member_wallet.trim().is_empty() {
            return Err("Member wallet address cannot be empty".to_string());
        }

        // Basic wallet address format validation (should start with 0x and be 42 chars)
        if !self.member_wallet.starts_with("0x") || self.member_wallet.len() != 42 {
            return Err("Invalid wallet address format".to_string());
        }

        Ok(())
    }
}

impl CommitteeMemberData {
    /// Create a new committee member data instance
    pub fn new(
        id: u64,
        member_wallet: String,
        is_approved: bool,
        created_at: String,
        updated_at: String,
    ) -> Self {
        Self {
            id,
            member_wallet,
            is_approved,
            created_at,
            updated_at,
        }
    }

    /// Check if the member is active (approved)
    pub fn is_active(&self) -> bool {
        self.is_approved
    }
}

impl CommitteeMemberResponse {
    /// Create a new committee member response
    pub fn new(id: u64) -> Self {
        Self { id }
    }
}

impl MembershipCheckResponse {
    /// Create a new membership check response
    pub fn new(is_member: bool) -> Self {
        Self { is_member }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_committee_member_request_creation() {
        let request = SetCommitteeMemberRequest::new(
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            true,
        );

        assert_eq!(
            request.member_wallet,
            "0x1234567890abcdef1234567890abcdef12345678"
        );
        assert!(request.is_approved);
    }

    #[test]
    fn test_set_committee_member_request_validation() {
        // Valid request
        let valid_request = SetCommitteeMemberRequest::new(
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            true,
        );
        assert!(valid_request.validate().is_ok());

        // Empty wallet
        let empty_wallet = SetCommitteeMemberRequest::new("".to_string(), true);
        assert!(empty_wallet.validate().is_err());

        // Invalid wallet format (no 0x prefix)
        let invalid_format = SetCommitteeMemberRequest::new(
            "1234567890abcdef1234567890abcdef12345678".to_string(),
            true,
        );
        assert!(invalid_format.validate().is_err());

        // Invalid wallet format (wrong length)
        let wrong_length = SetCommitteeMemberRequest::new("0x12345".to_string(), true);
        assert!(wrong_length.validate().is_err());
    }

    #[test]
    fn test_committee_member_data_creation() {
        let data = CommitteeMemberData::new(
            1,
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            true,
            "2023-01-01T00:00:00Z".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );

        assert_eq!(data.id, 1);
        assert_eq!(
            data.member_wallet,
            "0x1234567890abcdef1234567890abcdef12345678"
        );
        assert!(data.is_approved);
        assert!(data.is_active());
    }

    #[test]
    fn test_committee_member_data_is_active() {
        let active_member = CommitteeMemberData::new(
            1,
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            true,
            "2023-01-01T00:00:00Z".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );
        assert!(active_member.is_active());

        let inactive_member = CommitteeMemberData::new(
            2,
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            false,
            "2023-01-01T00:00:00Z".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );
        assert!(!inactive_member.is_active());
    }

    #[test]
    fn test_committee_member_response_creation() {
        let response = CommitteeMemberResponse::new(42);
        assert_eq!(response.id, 42);
    }

    #[test]
    fn test_membership_check_response_creation() {
        let member_response = MembershipCheckResponse::new(true);
        assert!(member_response.is_member);

        let non_member_response = MembershipCheckResponse::new(false);
        assert!(!non_member_response.is_member);
    }

    #[test]
    fn test_serialization_deserialization() {
        let request = SetCommitteeMemberRequest::new(
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            true,
        );

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: SetCommitteeMemberRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, deserialized);

        let data = CommitteeMemberData::new(
            1,
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            true,
            "2023-01-01T00:00:00Z".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );

        let json = serde_json::to_string(&data).unwrap();
        let deserialized: CommitteeMemberData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
