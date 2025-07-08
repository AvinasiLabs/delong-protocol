//! Voting system data models
//!
//! This module contains all data structures related to the voting system,
//! including vote operations, duration management, and vote data.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Vote decision enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub enum VoteDecision {
    /// Approve the algorithm
    #[serde(rename = "approve")]
    Approve,
    /// Reject the algorithm
    #[serde(rename = "reject")]
    Reject,
    /// Abstain from voting
    #[serde(rename = "abstain")]
    Abstain,
}

/// Vote status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub enum VoteStatus {
    /// Voting is active
    #[serde(rename = "active")]
    Active,
    /// Voting has completed
    #[serde(rename = "completed")]
    Completed,
    /// Voting was cancelled
    #[serde(rename = "cancelled")]
    Cancelled,
    /// Voting has expired
    #[serde(rename = "expired")]
    Expired,
}

/// Request payload for setting vote duration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct SetVoteDurationRequest {
    /// Duration in seconds for voting periods
    pub duration: u64,
}

/// Vote data model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct VoteData {
    /// Unique identifier for the vote record
    pub id: u64,
    /// Algorithm Content Identifier (CID) being voted on
    pub algo_cid: String,
    /// Wallet address of the voter
    pub voter: String,
    /// Vote decision (true for approve, false for reject)
    pub approve: bool,
    /// Timestamp when the vote was cast
    pub voted_at: String,
    /// Creation timestamp in RFC3339 format
    pub created_at: String,
    /// Last update timestamp in RFC3339 format
    pub updated_at: String,
}

/// Response for vote duration setting
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct VoteDurationResponse {
    /// The set duration in seconds
    pub duration: u64,
}

/// Extended vote data with additional information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ExtendedVoteData {
    /// Basic vote information
    #[serde(flatten)]
    pub vote: VoteData,
    /// Vote decision as enum
    pub decision: VoteDecision,
    /// Optional comment from the voter
    pub comment: Option<String>,
    /// Vote weight (for weighted voting systems)
    pub weight: Option<f64>,
}

/// Request for casting a vote
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct CastVoteRequest {
    /// Algorithm CID to vote on
    pub algo_cid: String,
    /// Vote decision
    pub decision: VoteDecision,
    /// Optional comment
    pub comment: Option<String>,
    /// Voter's wallet signature for verification
    pub signature: Option<String>,
}

/// Voting session information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct VotingSession {
    /// Unique identifier for the voting session
    pub id: String,
    /// Algorithm CID being voted on
    pub algo_cid: String,
    /// Current status of the voting session
    pub status: VoteStatus,
    /// Session start timestamp
    pub started_at: String,
    /// Session end timestamp (if set)
    pub ends_at: Option<String>,
    /// Duration of the voting session in seconds
    pub duration: u64,
    /// Total number of eligible voters
    pub eligible_voters: u32,
    /// Number of votes cast so far
    pub votes_cast: u32,
    /// Number of approve votes
    pub approve_count: u32,
    /// Number of reject votes
    pub reject_count: u32,
    /// Number of abstain votes
    pub abstain_count: u32,
}

/// Vote summary for reporting
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct VoteSummary {
    /// Algorithm CID
    pub algo_cid: String,
    /// Total votes cast
    pub total_votes: u32,
    /// Approve votes
    pub approve_votes: u32,
    /// Reject votes
    pub reject_votes: u32,
    /// Abstain votes
    pub abstain_votes: u32,
    /// Approval percentage
    pub approval_percentage: f64,
    /// Whether the vote passed
    pub passed: bool,
}

/// Query parameters for vote listing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct VoteQuery {
    /// Filter by algorithm CID (optional)
    pub algo_cid: Option<String>,
    /// Filter by voter wallet (optional)
    pub voter: Option<String>,
    /// Filter by vote decision (optional)
    pub decision: Option<VoteDecision>,
    /// Filter by vote date range - start (optional)
    pub voted_after: Option<String>,
    /// Filter by vote date range - end (optional)
    pub voted_before: Option<String>,
    /// Page number for pagination
    pub page: u32,
    /// Number of items per page
    pub limit: u32,
}

impl VoteDecision {
    /// Convert boolean to vote decision (for backwards compatibility)
    pub fn from_bool(approve: bool) -> Self {
        if approve {
            VoteDecision::Approve
        } else {
            VoteDecision::Reject
        }
    }

    /// Convert vote decision to boolean (for backwards compatibility)
    pub fn to_bool(&self) -> Option<bool> {
        match self {
            VoteDecision::Approve => Some(true),
            VoteDecision::Reject => Some(false),
            VoteDecision::Abstain => None,
        }
    }

    /// Check if the decision is positive
    pub fn is_positive(&self) -> bool {
        matches!(self, VoteDecision::Approve)
    }

    /// Check if the decision is negative
    pub fn is_negative(&self) -> bool {
        matches!(self, VoteDecision::Reject)
    }

    /// Check if the decision is neutral
    pub fn is_neutral(&self) -> bool {
        matches!(self, VoteDecision::Abstain)
    }
}

impl VoteStatus {
    /// Check if voting is still active
    pub fn is_active(&self) -> bool {
        matches!(self, VoteStatus::Active)
    }

    /// Check if voting has ended
    pub fn is_ended(&self) -> bool {
        matches!(
            self,
            VoteStatus::Completed | VoteStatus::Cancelled | VoteStatus::Expired
        )
    }

    /// Check if voting completed successfully
    pub fn is_completed(&self) -> bool {
        matches!(self, VoteStatus::Completed)
    }
}

impl SetVoteDurationRequest {
    /// Create a new vote duration request
    pub fn new(duration: u64) -> Self {
        Self { duration }
    }

    /// Validate the duration request
    pub fn validate(&self) -> Result<(), String> {
        if self.duration == 0 {
            return Err("Vote duration cannot be zero".to_string());
        }

        // Minimum 1 hour, maximum 30 days
        if self.duration < 3600 {
            return Err("Vote duration cannot be less than 1 hour (3600 seconds)".to_string());
        }

        if self.duration > 2_592_000 {
            // 30 days
            return Err("Vote duration cannot exceed 30 days (2,592,000 seconds)".to_string());
        }

        Ok(())
    }

    /// Get duration in hours
    pub fn duration_hours(&self) -> f64 {
        self.duration as f64 / 3600.0
    }

    /// Get duration in days
    pub fn duration_days(&self) -> f64 {
        self.duration as f64 / 86400.0
    }
}

impl VoteData {
    /// Create a new vote data instance
    pub fn new(
        id: u64,
        algo_cid: String,
        voter: String,
        approve: bool,
        voted_at: String,
        created_at: String,
        updated_at: String,
    ) -> Self {
        Self {
            id,
            algo_cid,
            voter,
            approve,
            voted_at,
            created_at,
            updated_at,
        }
    }

    /// Get the vote decision as enum
    pub fn decision(&self) -> VoteDecision {
        VoteDecision::from_bool(self.approve)
    }

    /// Check if this is an approval vote
    pub fn is_approval(&self) -> bool {
        self.approve
    }

    /// Check if this is a rejection vote
    pub fn is_rejection(&self) -> bool {
        !self.approve
    }
}

impl VoteDurationResponse {
    /// Create a new vote duration response
    pub fn new(duration: u64) -> Self {
        Self { duration }
    }

    /// Get duration in human-readable format
    pub fn formatted_duration(&self) -> String {
        let hours = self.duration / 3600;
        let minutes = (self.duration % 3600) / 60;
        let seconds = self.duration % 60;

        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }
}

impl CastVoteRequest {
    /// Create a new cast vote request
    pub fn new(algo_cid: String, decision: VoteDecision) -> Self {
        Self {
            algo_cid,
            decision,
            comment: None,
            signature: None,
        }
    }

    /// Validate the cast vote request
    pub fn validate(&self) -> Result<(), String> {
        if self.algo_cid.trim().is_empty() {
            return Err("Algorithm CID cannot be empty".to_string());
        }

        // Basic IPFS CID validation
        if !self.algo_cid.starts_with("Qm") && !self.algo_cid.starts_with("bafy") {
            return Err("Invalid IPFS CID format".to_string());
        }

        if let Some(comment) = &self.comment {
            if comment.len() > 1000 {
                return Err("Comment cannot exceed 1000 characters".to_string());
            }
        }

        Ok(())
    }
}

impl VotingSession {
    /// Create a new voting session
    pub fn new(
        id: String,
        algo_cid: String,
        duration: u64,
        eligible_voters: u32,
        started_at: String,
    ) -> Self {
        let ends_at = if duration > 0 {
            // Calculate end time
            if let Ok(start_time) = chrono::DateTime::parse_from_rfc3339(&started_at) {
                let end_time = start_time + chrono::Duration::seconds(duration as i64);
                Some(end_time.to_rfc3339())
            } else {
                None
            }
        } else {
            None
        };

        Self {
            id,
            algo_cid,
            status: VoteStatus::Active,
            started_at,
            ends_at,
            duration,
            eligible_voters,
            votes_cast: 0,
            approve_count: 0,
            reject_count: 0,
            abstain_count: 0,
        }
    }

    /// Check if the voting session has expired
    pub fn is_expired(&self) -> bool {
        if let Some(ends_at) = &self.ends_at {
            if let Ok(end_time) = chrono::DateTime::parse_from_rfc3339(ends_at) {
                return chrono::Utc::now() > end_time;
            }
        }
        false
    }

    /// Get the participation rate as percentage
    pub fn participation_rate(&self) -> f64 {
        if self.eligible_voters == 0 {
            0.0
        } else {
            (self.votes_cast as f64 / self.eligible_voters as f64) * 100.0
        }
    }

    /// Get the approval rate as percentage
    pub fn approval_rate(&self) -> f64 {
        if self.votes_cast == 0 {
            0.0
        } else {
            (self.approve_count as f64 / self.votes_cast as f64) * 100.0
        }
    }
}

impl VoteSummary {
    /// Create a new vote summary
    pub fn new(
        algo_cid: String,
        approve_votes: u32,
        reject_votes: u32,
        abstain_votes: u32,
    ) -> Self {
        let total_votes = approve_votes + reject_votes + abstain_votes;
        let approval_percentage = if total_votes > 0 {
            (approve_votes as f64 / total_votes as f64) * 100.0
        } else {
            0.0
        };

        // Simple majority rule for passed determination
        let passed = approve_votes > reject_votes;

        Self {
            algo_cid,
            total_votes,
            approve_votes,
            reject_votes,
            abstain_votes,
            approval_percentage,
            passed,
        }
    }

    /// Check if the vote meets quorum (configurable threshold)
    pub fn meets_quorum(&self, required_votes: u32) -> bool {
        self.total_votes >= required_votes
    }

    /// Check if the vote meets approval threshold
    pub fn meets_approval_threshold(&self, threshold_percent: f64) -> bool {
        self.approval_percentage >= threshold_percent
    }
}

impl VoteQuery {
    /// Create a new vote query with defaults
    pub fn new() -> Self {
        Self {
            algo_cid: None,
            voter: None,
            decision: None,
            voted_after: None,
            voted_before: None,
            page: 1,
            limit: 20,
        }
    }
}

impl Default for VoteQuery {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vote_decision_conversions() {
        assert_eq!(VoteDecision::from_bool(true), VoteDecision::Approve);
        assert_eq!(VoteDecision::from_bool(false), VoteDecision::Reject);

        assert_eq!(VoteDecision::Approve.to_bool(), Some(true));
        assert_eq!(VoteDecision::Reject.to_bool(), Some(false));
        assert_eq!(VoteDecision::Abstain.to_bool(), None);
    }

    #[test]
    fn test_vote_decision_checks() {
        assert!(VoteDecision::Approve.is_positive());
        assert!(!VoteDecision::Approve.is_negative());
        assert!(!VoteDecision::Approve.is_neutral());

        assert!(!VoteDecision::Reject.is_positive());
        assert!(VoteDecision::Reject.is_negative());
        assert!(!VoteDecision::Reject.is_neutral());

        assert!(!VoteDecision::Abstain.is_positive());
        assert!(!VoteDecision::Abstain.is_negative());
        assert!(VoteDecision::Abstain.is_neutral());
    }

    #[test]
    fn test_vote_status_checks() {
        assert!(VoteStatus::Active.is_active());
        assert!(!VoteStatus::Active.is_ended());

        assert!(!VoteStatus::Completed.is_active());
        assert!(VoteStatus::Completed.is_ended());
        assert!(VoteStatus::Completed.is_completed());

        assert!(VoteStatus::Expired.is_ended());
        assert!(!VoteStatus::Expired.is_completed());
    }

    #[test]
    fn test_set_vote_duration_request_validation() {
        // Valid duration (1 day)
        let valid_request = SetVoteDurationRequest::new(86400);
        assert!(valid_request.validate().is_ok());

        // Zero duration
        let zero_duration = SetVoteDurationRequest::new(0);
        assert!(zero_duration.validate().is_err());

        // Too short (30 minutes)
        let too_short = SetVoteDurationRequest::new(1800);
        assert!(too_short.validate().is_err());

        // Too long (31 days)
        let too_long = SetVoteDurationRequest::new(2_678_400);
        assert!(too_long.validate().is_err());
    }

    #[test]
    fn test_set_vote_duration_request_conversions() {
        let request = SetVoteDurationRequest::new(86400); // 1 day
        assert_eq!(request.duration_hours(), 24.0);
        assert_eq!(request.duration_days(), 1.0);
    }

    #[test]
    fn test_vote_data_creation() {
        let vote = VoteData::new(
            1,
            "QmTest123".to_string(),
            "0x1234567890abcdef".to_string(),
            true,
            "2023-01-01T12:00:00Z".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );

        assert_eq!(vote.id, 1);
        assert_eq!(vote.algo_cid, "QmTest123");
        assert!(vote.is_approval());
        assert!(!vote.is_rejection());
        assert_eq!(vote.decision(), VoteDecision::Approve);
    }

    #[test]
    fn test_vote_duration_response_formatting() {
        let response = VoteDurationResponse::new(7200); // 2 hours
        assert_eq!(response.formatted_duration(), "2h 0m 0s");

        let response = VoteDurationResponse::new(3661); // 1h 1m 1s
        assert_eq!(response.formatted_duration(), "1h 1m 1s");

        let response = VoteDurationResponse::new(61); // 1m 1s
        assert_eq!(response.formatted_duration(), "1m 1s");

        let response = VoteDurationResponse::new(30); // 30s
        assert_eq!(response.formatted_duration(), "30s");
    }

    #[test]
    fn test_cast_vote_request_validation() {
        // Valid request
        let valid_request = CastVoteRequest::new("QmTest123".to_string(), VoteDecision::Approve);
        assert!(valid_request.validate().is_ok());

        // Empty CID
        let empty_cid = CastVoteRequest::new("".to_string(), VoteDecision::Approve);
        assert!(empty_cid.validate().is_err());

        // Invalid CID format
        let invalid_cid = CastVoteRequest::new("invalid_cid".to_string(), VoteDecision::Approve);
        assert!(invalid_cid.validate().is_err());
    }

    #[test]
    fn test_voting_session_creation() {
        let session = VotingSession::new(
            "session_123".to_string(),
            "QmTest123".to_string(),
            86400, // 1 day
            10,    // 10 eligible voters
            "2023-01-01T00:00:00Z".to_string(),
        );

        assert_eq!(session.id, "session_123");
        assert_eq!(session.algo_cid, "QmTest123");
        assert!(session.status.is_active());
        assert_eq!(session.eligible_voters, 10);
        assert_eq!(session.participation_rate(), 0.0);
        assert_eq!(session.approval_rate(), 0.0);
    }

    #[test]
    fn test_vote_summary_creation() {
        let summary = VoteSummary::new("QmTest123".to_string(), 7, 3, 0);

        assert_eq!(summary.algo_cid, "QmTest123");
        assert_eq!(summary.total_votes, 10);
        assert_eq!(summary.approve_votes, 7);
        assert_eq!(summary.reject_votes, 3);
        assert_eq!(summary.approval_percentage, 70.0);
        assert!(summary.passed);
        assert!(summary.meets_approval_threshold(50.0));
        assert!(!summary.meets_approval_threshold(80.0));
    }

    #[test]
    fn test_serialization_deserialization() {
        let vote = VoteData::new(
            1,
            "QmTest123".to_string(),
            "0x1234567890abcdef".to_string(),
            true,
            "2023-01-01T12:00:00Z".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );

        let json = serde_json::to_string(&vote).unwrap();
        let deserialized: VoteData = serde_json::from_str(&json).unwrap();
        assert_eq!(vote, deserialized);

        let request = SetVoteDurationRequest::new(3600);
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: SetVoteDurationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, deserialized);
    }
}
