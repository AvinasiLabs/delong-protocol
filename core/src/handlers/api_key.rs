use axum::extract::{Path, State};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use utoipa::ToSchema;
use validator::Validate;

use avinapi::prelude::{
    AppError, JsonResult, PaginatedResult, PaginationQuery, ValidatedJson, ValidatedQuery, data,
    paginated,
};

use crate::{middleware::AuthUser, models::api_key::ApiKey, routes::AppState};

// ============================================================================
// Request/Response/Query Structures
// ============================================================================

/// Create API key request
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateApiKeyRequest {
    #[validate(length(
        min = 1,
        max = 255,
        message = "Name must be between 1 and 255 characters"
    ))]
    pub name: String,

    #[serde(default)]
    pub permissions: Vec<String>,

    pub expires_at: Option<DateTime<Utc>>,
}

/// Update API key request
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct UpdateApiKeyRequest {
    #[validate(length(
        min = 1,
        max = 255,
        message = "Name must be between 1 and 255 characters"
    ))]
    pub name: Option<String>,

    pub permissions: Option<Vec<String>>,
    pub is_active: Option<bool>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// API key filter query parameters (excludes pagination)
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ApiKeyFilterQuery {
    pub is_active: Option<bool>,
    pub rate_limit_tier: Option<String>,
}

/// API key response
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApiKeyResponse {
    pub id: i32,
    pub api_key: String,
    pub name: String,
    pub user_id: i32,
    pub permissions: Vec<String>,
    pub rate_limit_tier: String,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// API key list response (with preview)
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApiKeyListResponse {
    pub id: i32,
    pub name: String,
    pub api_key_preview: String,
    pub permissions: Vec<String>,
    pub rate_limit_tier: String,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// API key statistics response
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApiKeyStats {
    pub total_keys: i64,
    pub active_keys: i64,
    pub inactive_keys: i64,
    pub expired_keys: i64,
    pub by_tier: serde_json::Value,
}

// ============================================================================
// Conversion Traits
// ============================================================================

impl From<ApiKey> for ApiKeyResponse {
    fn from(key: ApiKey) -> Self {
        ApiKeyResponse {
            id: key.id,
            api_key: key.api_key,
            name: key.name,
            user_id: key.user_id,
            permissions: key.permissions,
            rate_limit_tier: key.rate_limit_tier.to_string(),
            is_active: key.is_active,
            expires_at: key.expires_at,
            last_used_at: key.last_used_at,
            created_at: key.created_at,
            updated_at: key.updated_at,
        }
    }
}

impl From<ApiKey> for ApiKeyListResponse {
    fn from(key: ApiKey) -> Self {
        // Only show first 8 characters of API key
        let api_key_preview = if key.api_key.len() > 8 {
            format!("{}...", &key.api_key[..8])
        } else {
            format!("{}...", key.api_key)
        };

        ApiKeyListResponse {
            id: key.id,
            name: key.name,
            api_key_preview,
            permissions: key.permissions,
            rate_limit_tier: key.rate_limit_tier.to_string(),
            is_active: key.is_active,
            expires_at: key.expires_at,
            last_used_at: key.last_used_at,
            created_at: key.created_at,
            updated_at: key.updated_at,
        }
    }
}

// ============================================================================
// Handler Functions
// ============================================================================

/// Create a new API key for the authenticated user
#[utoipa::path(
    post,
    path = "/api/api-keys",
    tag = "API Keys",
    request_body = CreateApiKeyRequest,
    responses(
        (status = 200, description = "API key creation result", body = ApiKeyResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_api_key(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ValidatedJson(payload): ValidatedJson<CreateApiKeyRequest>,
) -> JsonResult<ApiKeyResponse> {
    info!(
        "User {} creating new API key: {}",
        auth_user.user_id(),
        payload.name
    );

    // Create API key for the authenticated user
    let api_key = ApiKey::create(
        &state.db,
        auth_user.user_id(),
        payload.name,
        payload.permissions,
        payload.expires_at,
    )
    .await
    .map_err(|e| {
        error!("Failed to create API key: {}", e);
        match e {
            AppError::Conflict(msg) => AppError::Conflict(msg),
            AppError::Validation(msg) => AppError::Validation(msg),
            AppError::Database(ref msg) if msg.to_string().contains("duplicate") => {
                AppError::Conflict("API key with this name already exists".to_string())
            }
            _ => AppError::Internal("Failed to create API key".to_string()),
        }
    })?;

    info!("API key created successfully with ID: {}", api_key.id);
    data!(ApiKeyResponse::from(api_key))
}

/// List user's own API keys
#[utoipa::path(
    get,
    path = "/api/api-keys",
    tag = "API Keys",

    responses(
        (status = 200, description = "API keys list result", body = Vec<ApiKeyListResponse>)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_user_api_keys(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ValidatedQuery(filters): ValidatedQuery<ApiKeyFilterQuery>,
    ValidatedQuery(pagination): ValidatedQuery<PaginationQuery>,
) -> PaginatedResult<ApiKeyListResponse> {
    info!(
        "User {} listing API keys (page: {}, per_page: {})",
        auth_user.user_id(),
        pagination.page,
        pagination.per_page
    );

    // Get current user's API keys
    let (api_keys, total) = ApiKey::get_user_keys(
        &state.db,
        auth_user.user_id(),
        filters.is_active,
        filters.rate_limit_tier,
        pagination.page,
        pagination.per_page,
    )
    .await
    .map_err(|e| {
        error!("Failed to list API keys: {}", e);
        AppError::Internal("Failed to list API keys".to_string())
    })?;

    let items: Vec<ApiKeyListResponse> =
        api_keys.into_iter().map(ApiKeyListResponse::from).collect();

    paginated!(items, total as u64, pagination.page, pagination.per_page)
}

/// Get details of a specific API key
#[utoipa::path(
    get,
    path = "/api/api-keys/{id}",
    tag = "API Keys",
    params(
        ("id" = i32, Path, description = "API key ID")
    ),
    responses(
        (status = 200, description = "API key details result", body = ApiKeyResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_api_key(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<i32>,
) -> JsonResult<ApiKeyResponse> {
    info!("User {} getting API key: {}", auth_user.user_id(), id);

    let api_key = ApiKey::find_by_id(&state.db, id).await.map_err(|e| {
        error!("Failed to find API key: {}", e);
        match e {
            AppError::NotFound(_) => AppError::NotFound("API key not found".to_string()),
            _ => AppError::Internal("Failed to get API key".to_string()),
        }
    })?;

    // Verify the API key belongs to the current user
    if api_key.user_id != auth_user.user_id() {
        return Err(AppError::NotFound("API key not found".to_string()));
    }

    data!(ApiKeyResponse::from(api_key))
}

/// Update an API key
#[utoipa::path(
    put,
    path = "/api/api-keys/{id}",
    tag = "API Keys",
    params(
        ("id" = i32, Path, description = "API key ID")
    ),
    request_body = UpdateApiKeyRequest,
    responses(
        (status = 200, description = "API key update result", body = ApiKeyResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_api_key(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<i32>,
    ValidatedJson(payload): ValidatedJson<UpdateApiKeyRequest>,
) -> JsonResult<ApiKeyResponse> {
    info!("User {} updating API key {}", auth_user.user_id(), id);

    // First verify the API key belongs to the user
    let existing = ApiKey::find_by_id(&state.db, id).await.map_err(|e| {
        error!("Failed to find API key: {}", e);
        match e {
            AppError::NotFound(_) => AppError::NotFound("API key not found".to_string()),
            _ => AppError::Internal("Failed to update API key".to_string()),
        }
    })?;

    if existing.user_id != auth_user.user_id() {
        return Err(AppError::NotFound("API key not found".to_string()));
    }

    // Update the API key
    let updated = ApiKey::update(
        &state.db,
        id,
        payload.name,
        payload.permissions,
        payload.is_active,
        Some(payload.expires_at),
    )
    .await
    .map_err(|e| {
        error!("Failed to update API key: {}", e);
        match e {
            AppError::NotFound(_) => AppError::NotFound("API key not found".to_string()),
            AppError::Database(ref msg) if msg.to_string().contains("duplicate") => {
                AppError::Validation("API key with this name already exists".to_string())
            }
            _ => AppError::Internal("Failed to update API key".to_string()),
        }
    })?;

    info!("API key {} updated successfully", id);
    data!(ApiKeyResponse::from(updated))
}

/// Revoke (delete) an API key
#[utoipa::path(
    delete,
    path = "/api/api-keys/{id}",
    tag = "API Keys",
    params(
        ("id" = i32, Path, description = "API key ID")
    ),
    responses(
        (status = 200, description = "API key revocation result")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn revoke_api_key(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<i32>,
) -> JsonResult<serde_json::Value> {
    info!("User {} revoking API key {}", auth_user.user_id(), id);

    // First verify the API key belongs to the user
    let api_key = ApiKey::find_by_id(&state.db, id).await.map_err(|e| {
        error!("Failed to find API key: {}", e);
        match e {
            AppError::NotFound(_) => AppError::NotFound("API key not found".to_string()),
            _ => AppError::Internal("Failed to revoke API key".to_string()),
        }
    })?;

    if api_key.user_id != auth_user.user_id() {
        return Err(AppError::NotFound("API key not found".to_string()));
    }

    // Delete the API key
    ApiKey::delete(&state.db, id).await.map_err(|e| {
        error!("Failed to revoke API key: {}", e);
        match e {
            AppError::NotFound(_) => AppError::NotFound("API key not found".to_string()),
            _ => AppError::Internal("Failed to revoke API key".to_string()),
        }
    })?;

    info!("API key {} revoked successfully", id);
    data!(serde_json::json!({
        "message": "API key revoked successfully",
        "id": id
    }))
}

/// Get API key statistics for the authenticated user
#[utoipa::path(
    get,
    path = "/api/api-keys/stats",
    tag = "API Keys",
    responses(
        (status = 200, description = "API key statistics result", body = ApiKeyStats)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_user_api_key_stats(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> JsonResult<ApiKeyStats> {
    info!("User {} getting API key statistics", auth_user.user_id());

    let stats = ApiKey::get_user_stats(&state.db, auth_user.user_id())
        .await
        .map_err(|e| {
            error!("Failed to get API key statistics: {}", e);
            AppError::Internal("Failed to get API key statistics".to_string())
        })?;

    data!(ApiKeyStats {
        total_keys: stats.total_keys,
        active_keys: stats.active_keys,
        inactive_keys: stats.inactive_keys,
        expired_keys: stats.expired_keys,
        by_tier: stats.by_tier,
    })
}
