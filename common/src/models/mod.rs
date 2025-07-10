//! Data models for DeLong Protocol
//!
//! This module contains shared data structures used across all services
//! in the DeLong Protocol ecosystem. All models are designed to be
//! serializable and compatible with database operations.
//!
//! # Modules
//!
//! - [`algo_exe`] - Algorithm execution related models
//! - [`auth`] - Authentication and authorization models
//! - [`committee`] - Committee member management models
//! - [`contract`] - Smart contract metadata models
//! - [`dataset`] - Dataset management models (static and dynamic)
//! - [`pagination`] - Pagination utilities and response wrappers
//! - [`report`] - Test report management models
//! - [`vote`] - Voting system models
//! - [`websocket`] - WebSocket notification models
//!
//! # Examples
//!
//! ```rust
//! use delong_core::models::{
//!     AlgoExeSubmissionRequest, PaginatedResponse, PaginationParams,
//!     CreateApiKeyRequest, Permission, RateLimitTier,
//!     SetCommitteeMemberRequest, ContractData,
//! };
//!
//! // Create a pagination request
//! let params = PaginationParams::new(1, 20).unwrap();
//!
//! // Create an algorithm execution request
//! let request = AlgoExeSubmissionRequest {
//!     github_repo: "https://github.com/user/repo".to_string(),
//!     commit_hash: "abc123".to_string(),
//!     scientist_wallet: "0x123...".to_string(),
//!     dataset: "dataset_001".to_string(),
//! };
//!
//! // Create an API key request
//! let api_key_request = CreateApiKeyRequest::new(
//!     "My API Key".to_string(),
//!     vec![Permission::DataRead, Permission::DataWrite],
//! );
//! ```

pub mod algo_exe;
pub mod auth;
pub mod blockchain;
pub mod committee;
pub mod contract;
pub mod dataset;
pub mod pagination;
pub mod report;
pub mod runtime;
pub mod vote;
pub mod websocket;

// Re-export commonly used types from each module for convenience

// Algorithm execution models
pub use algo_exe::{
    AlgoExeData, AlgoExeStatus, AlgoExeSubmissionRequest, AlgoExeSubmissionResponse,
    AlgoReviewStatus,
};

// Authentication models
pub use auth::{
    ApiKeyInfo, ApiKeyListQuery, AuthContext, AuthConfig, AuthMethod, Claims, CreateApiKeyRequest,
    CreateApiKeyResponse, JwtClaims, JwtUtils, Permission, RateLimitTier, RevokeApiKeyRequest,
    RevokeApiKeyResponse, UserSession, ValidateApiKeyRequest, ValidateApiKeyResponse,
    is_valid_api_key_format,
};

// Blockchain models
pub use blockchain::{
    BlockchainTransaction, CreateTransactionRequest, EntityType, TransactionQuery,
    TransactionResponse, TransactionStatus as BlockchainTransactionStatus, 
    UpdateTransactionStatusRequest, is_valid_tx_hash, JOIN_CONFIRMED_TX,
};

// Committee models
pub use committee::{
    CommitteeMemberData, CommitteeMemberResponse, MembershipCheckResponse,
    SetCommitteeMemberRequest,
};

// Contract models
pub use contract::{
    ContractData, ContractResponse, CreateContractRequest, ExtendedContractData,
    UpdateContractRequest,
};

// Dataset models
pub use dataset::{
    CreateDatasetRequest, DatasetFormat, DatasetPaginatedResponse, DatasetStatus,
    DynamicDatasetInfo, DynamicDatasetListQuery, StaticDatasetInfo, StaticDatasetListQuery,
    UpdateDatasetRequest, UpdateStaticDatasetRequest,
};

// Pagination models
pub use pagination::{MAX_LIMIT, PaginatedResponse, PaginationParams};

// Report models
pub use report::{
    ReportInfo, ReportQuery, ReportStatus, ReportSummary, ReportType, UploadReportRequest,
    UploadReportResponse,
};

// Vote models
pub use vote::{
    CastVoteRequest, ExtendedVoteData, SetVoteDurationRequest, VoteData, VoteDecision,
    VoteDurationResponse, VoteQuery, VoteStatus, VoteSummary, VotingSession,
};

// WebSocket models
pub use websocket::{
    BlockchainTransactionNotification, ConnectionStats, NotificationMessage, SubscriptionRequest,
    SubscriptionResponse, TransactionStatus, WebSocketClient, generate_client_id,
};
