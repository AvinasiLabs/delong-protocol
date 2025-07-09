//! Common prelude module for convenient imports across DeLong Protocol services
//!
//! This module re-exports commonly used types, functions, and utilities from the
//! common crate to provide a convenient single import point for other services.
//!
//! # Usage
//!
//! ```rust
//! use common::prelude::*;
//! ```

pub use crate::config::{EnvLoader, LoggingConfig, RedisConfig};
pub use crate::middleware::REQUEST_ID_HEADER;
pub use crate::middleware::request_id::{
    RequestIdConfig, RequestIdGenerator, generate_request_id, get_current_request_id,
    get_request_id_from_headers, request_id_middleware, request_id_middleware_with_config,
};
pub use crate::middleware::{
    MiddlewareUtils,
    logging::{
        log_large_request, log_slow_request, logging_middleware, security_logging_middleware,
    },
};
pub use crate::models::*;
pub use crate::models::{
    // Algorithm execution models
    AlgoExeData,
    AlgoExeStatus,
    AlgoExeSubmissionRequest,
    AlgoExeSubmissionResponse,
    AlgoReviewStatus,

    // Auth models
    ApiKeyInfo,
    ApiKeyListQuery,
    // Custom response wrapper
    AuthContext,
    AuthMethod,
    // WebSocket models
    BlockchainTransactionNotification,
    // Vote models
    CastVoteRequest,
    // Committee models
    CommitteeMemberData,
    CommitteeMemberResponse,
    ConnectionStats,
    // Contract models
    ContractData,
    ContractResponse,
    CreateApiKeyRequest,
    CreateApiKeyResponse,
    CreateContractRequest,
    // Dataset models
    CreateDatasetRequest,
    DatasetFormat,
    DatasetStatus,
    DynamicDatasetInfo,
    DynamicDatasetListQuery,
    ExtendedContractData,
    ExtendedVoteData,
    JwtClaims,
    // Pagination models
    MAX_LIMIT,
    MembershipCheckResponse,
    NotificationMessage,
    // Pagination models
    PaginatedResponse,
    PaginationParams,

    Permission,
    RateLimitTier,
    // Report models
    ReportInfo,
    ReportQuery,
    ReportStatus,
    ReportSummary,
    ReportType,
    RevokeApiKeyRequest,
    RevokeApiKeyResponse,

    SetCommitteeMemberRequest,

    SetVoteDurationRequest,
    StaticDatasetInfo,
    StaticDatasetListQuery,
    SubscriptionRequest,
    SubscriptionResponse,
    TransactionStatus,

    UpdateContractRequest,

    UpdateDatasetRequest,
    UpdateStaticDatasetRequest,

    UploadReportRequest,
    UploadReportResponse,

    UserSession,
    ValidateApiKeyRequest,
    ValidateApiKeyResponse,
    VoteData,
    VoteDecision,
    VoteDurationResponse,
    VoteQuery,
    VoteStatus,
    VoteSummary,
    VotingSession,
    WebSocketClient,
    generate_client_id,
    is_valid_api_key_format,
};
pub use crate::server::{
    OpenTelemetryConfig, ServerConfig, get_env_bool, get_env_string, get_env_u16, get_env_u64,
    init_logging, parse_socket_addr, shutdown_signal,
};
pub use crate::server::{load_env_file, start_server};
pub use crate::utils::{
    clean_for_logging, current_timestamp_ms, current_timestamp_secs, env_var_as_bool,
    env_var_as_u64, env_var_or_default, format_bytes, format_duration, generate_unique_id,
    hash_string, is_safe_for_logging, sanitize_path_for_logging, truncate_string,
};
pub use crate::{ApiError, ApiResponse, ApiResult, CommonError, ResponseCode, Result, VERSION};
pub use async_trait::async_trait;
pub use axum::{
    Router,
    extract::{Query, State},
    response::Json,
    routing::{get, post},
};
pub use chrono::{DateTime, Utc};
pub use serde::{Deserialize, Serialize};
pub use tracing::{debug, error, info, trace, warn};
pub use uuid::Uuid;
