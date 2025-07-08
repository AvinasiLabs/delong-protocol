//! Admin handlers
//!
//! This module contains HTTP handlers for administrative functions,
//! including user management, role/permission management, and API key management.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use common::{ApiError, ApiResponse, ResponseCode};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;
use tracing::{error, info, warn};
use validator::Validate;

use crate::{
    AppState,
    models::{
        api_key::{ApiKey, ApiKeyQuery, ApiKeyResponse, ApiKeyStats, CreateApiKeyRequest},
        user::{
            CreateUserRequest, RolesPermissionsResponse, User, UserListResponse, UserQueryParams,
            UserResponse, get_roles_and_permissions,
        },
    },
};

/// Query parameters for admin user listing
#[derive(Debug, Deserialize)]
pub struct AdminUserQuery {
    pub role: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

/// Admin user creation request
#[derive(Debug, Deserialize, Validate)]
pub struct AdminCreateUserRequest {
    #[validate(length(min = 3, max = 100))]
    pub username: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 6, max = 128))]
    pub password: Option<String>,
    pub role: String,
    #[serde(rename = "walletAddress")]
    pub wallet_address: Option<String>,
}

/// Admin user update request
#[derive(Debug, Deserialize, Validate)]
pub struct AdminUpdateUserRequest {
    #[validate(length(min = 3, max = 100))]
    pub username: Option<String>,
    #[validate(email)]
    pub email: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
    pub wallet_address: Option<String>,
}

/// Admin user response
#[derive(Debug, serde::Serialize)]
pub struct AdminUserResponse {
    pub user: UserResponse,
    pub message: String,
}

/// Permission check request
#[derive(Debug, Deserialize)]
pub struct PermissionCheckRequest {
    pub user_role: String,
    pub permission: String,
}

/// Permission check response
#[derive(Debug, Serialize)]
pub struct PermissionCheckResponse {
    pub user_id: i32,
    pub permission: String,
    pub has_permission: bool,
    pub user_role: String,
}

/// Get users list (admin only)
/// GET /admin/users
pub async fn get_users(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AdminUserQuery>,
    // TODO: Add admin authorization middleware
) -> Result<Json<ApiResponse<UserListResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Admin get users request");

    // Convert query to internal format with proper type conversion
    let user_query = UserQueryParams {
        role: query.role,
        status: query.status,
        search: query.search,
        page: query.page,
        limit: query.limit,
    };

    // Get users from database
    let user_list = User::get_users(&state.db, user_query).await.map_err(|e| {
        error!("Failed to get users: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(ResponseCode::InternalServerError)),
        )
    })?;

    Ok(Json(ApiResponse::success(user_list)))
}

/// Create new user (admin only)
/// POST /admin/users
pub async fn create_user_admin(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AdminCreateUserRequest>,
    // TODO: Add admin authorization middleware
) -> Result<Json<ApiResponse<AdminUserResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Admin create user request for email: {}", payload.email);

    // Validate input
    if let Err(validation_errors) = payload.validate() {
        warn!(
            "Admin user creation validation failed: {:?}",
            validation_errors
        );
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // Validate role - include committee role support
    let valid_roles = ["scientist", "committee", "admin"];
    if !valid_roles.contains(&payload.role.as_str()) {
        warn!("Invalid role provided: {}", payload.role);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // Check if user already exists
    if let Ok(Some(_)) = User::find_by_email(&state.db, &payload.email).await {
        warn!("User already exists with email: {}", payload.email);
        return Err((
            StatusCode::CONFLICT,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // Create user request
    let create_request = CreateUserRequest {
        username: payload.username,
        email: payload.email,
        password: payload.password,
        role: Some(payload.role),
        wallet_address: payload.wallet_address,
    };

    // Create user
    let user = User::create(&state.db, create_request).await.map_err(|e| {
        error!("Failed to create user: {}", e);
        match e {
            ApiError::AlreadyExists(_) => (
                StatusCode::CONFLICT,
                Json(ApiResponse::error(ResponseCode::BadRequest)),
            ),
            ApiError::ValidationFailed => (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(ResponseCode::BadRequest)),
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            ),
        }
    })?;

    info!("User created successfully by admin: {}", user.email);

    let response = AdminUserResponse {
        user: user.into(),
        message: "User created successfully".to_string(),
    };

    Ok(Json(ApiResponse::success(response)))
}

/// Get user by ID (admin only)
/// GET /admin/users/:id
pub async fn get_user_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    // TODO: Add admin authorization middleware
) -> Result<Json<ApiResponse<UserResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Admin get user by ID request: {}", id);

    let user = User::find_by_id(&state.db, id)
        .await
        .map_err(|e| {
            error!("Failed to find user: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            )
        })?
        .ok_or_else(|| {
            warn!("User not found with ID: {}", id);
            (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::error(ResponseCode::NotFound)),
            )
        })?;

    Ok(Json(ApiResponse::success(user.into())))
}

/// Update user (admin only)
/// PUT /admin/users/:id
pub async fn update_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Json(payload): Json<AdminUpdateUserRequest>,
    // TODO: Add admin authorization middleware
) -> Result<Json<ApiResponse<UserResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Admin update user request for ID: {}", id);

    // Validate input
    if let Err(validation_errors) = payload.validate() {
        warn!(
            "Admin user update validation failed: {:?}",
            validation_errors
        );
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // Validate role if provided
    if let Some(ref role) = payload.role {
        let valid_roles = ["scientist", "committee", "admin"];
        if !valid_roles.contains(&role.as_str()) {
            warn!("Invalid role provided: {}", role);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(ResponseCode::BadRequest)),
            ));
        }
    }

    // Validate status if provided
    if let Some(ref status) = payload.status {
        let valid_statuses = ["active", "inactive", "suspended"];
        if !valid_statuses.contains(&status.as_str()) {
            warn!("Invalid status provided: {}", status);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(ResponseCode::BadRequest)),
            ));
        }
    }

    // Convert to internal update request
    let update_request = crate::models::user::UpdateUserRequest {
        username: payload.username,
        email: payload.email,
        role: payload.role,
        status: payload.status,
        wallet_address: payload.wallet_address,
        avatar_url: None,
        email_verified: None,
    };

    // Update user
    let updated_user = User::update(&state.db, id, update_request)
        .await
        .map_err(|e| {
            error!("Failed to update user: {}", e);
            match e {
                ApiError::NotFound => (
                    StatusCode::NOT_FOUND,
                    Json(ApiResponse::error(ResponseCode::NotFound)),
                ),
                ApiError::ValidationFailed => (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::error(ResponseCode::BadRequest)),
                ),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::error(ResponseCode::InternalServerError)),
                ),
            }
        })?;

    info!("User updated successfully by admin: {}", updated_user.email);

    Ok(Json(ApiResponse::success(updated_user.into())))
}

/// Delete user (admin only)
/// DELETE /admin/users/:id
pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    // TODO: Add admin authorization middleware
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Admin delete user request for ID: {}", id);

    User::delete(&state.db, id).await.map_err(|e| {
        error!("Failed to delete user: {}", e);
        match e {
            ApiError::NotFound => (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::error(ResponseCode::NotFound)),
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            ),
        }
    })?;

    info!("User deleted successfully by admin: {}", id);

    let response_data = serde_json::json!({
        "message": "User deleted successfully"
    });

    Ok(Json(ApiResponse::success(response_data)))
}

/// Get all roles and permissions (admin only)
/// GET /admin/roles
pub async fn get_roles(
    State(state): State<Arc<AppState>>,
    // TODO: Add admin authorization middleware
) -> Result<Json<ApiResponse<RolesPermissionsResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Admin get roles and permissions request");

    let roles_permissions = get_roles_and_permissions(&state.db).await.map_err(|e| {
        error!("Failed to get roles and permissions: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(ResponseCode::InternalServerError)),
        )
    })?;

    Ok(Json(ApiResponse::success(roles_permissions)))
}

/// Get permissions list (admin only)
/// GET /admin/permissions
pub async fn get_permissions(
    State(state): State<Arc<AppState>>,
    // TODO: Add admin authorization middleware
) -> Result<
    Json<ApiResponse<Vec<crate::models::user::PermissionInfo>>>,
    (StatusCode, Json<ApiResponse<()>>),
> {
    info!("Admin get permissions request");

    let roles_permissions = get_roles_and_permissions(&state.db).await.map_err(|e| {
        error!("Failed to get permissions: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(ResponseCode::InternalServerError)),
        )
    })?;

    Ok(Json(ApiResponse::success(roles_permissions.permissions)))
}

/// Check user permission (admin only)
/// GET /admin/permissions/:user_id/:permission
pub async fn check_permission(
    State(state): State<Arc<AppState>>,
    Path((user_id, permission)): Path<(i32, String)>,
    // TODO: Add admin authorization middleware
) -> Result<Json<ApiResponse<PermissionCheckResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!(
        "Admin check permission request for user {} and permission {}",
        user_id, permission
    );

    // Find user
    let user = User::find_by_id(&state.db, user_id)
        .await
        .map_err(|e| {
            error!("Failed to find user: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            )
        })?
        .ok_or_else(|| {
            warn!("User not found for permission check: {}", user_id);
            (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::error(ResponseCode::NotFound)),
            )
        })?;

    // Check permission by querying role_permissions
    let has_permission = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM users u
            JOIN roles r ON u.role = r.name
            JOIN role_permissions rp ON r.id = rp.role_id
            JOIN permissions p ON rp.permission_id = p.id
            WHERE u.id = $1 AND p.name = $2
        )
        "#,
    )
    .bind(user_id)
    .bind(&permission)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        error!("Failed to check permission: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(ResponseCode::InternalServerError)),
        )
    })?;

    let response = PermissionCheckResponse {
        user_id,
        permission,
        has_permission,
        user_role: user.role.clone(),
    };

    Ok(Json(ApiResponse::success(response)))
}

/// List API keys (admin only)
/// GET /admin/api-keys
pub async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ApiKeyQuery>,
    // TODO: Add admin authorization middleware
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Admin list API keys request");

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(10).clamp(1, 100);
    let offset = (page - 1) * limit;

    // Build query with filters
    let mut sql = String::from(
        r#"
        SELECT ak.id, ak.api_key, ak.name, ak.user_id, ak.permissions, ak.rate_limit_tier,
               ak.is_active, ak.expires_at, ak.last_used_at, ak.created_at, ak.updated_at,
               u.username, u.email
        FROM api_keys ak
        JOIN users u ON ak.user_id = u.id
        WHERE 1=1
        "#,
    );

    let mut count_sql = String::from(
        r#"
        SELECT COUNT(*)
        FROM api_keys ak
        JOIN users u ON ak.user_id = u.id
        WHERE 1=1
        "#,
    );

    // Add filters
    if let Some(is_active) = query.is_active {
        let filter = format!(" AND ak.is_active = {}", is_active);
        sql.push_str(&filter);
        count_sql.push_str(&filter);
    }

    if let Some(tier) = query.rate_limit_tier {
        let filter = format!(" AND ak.rate_limit_tier = '{}'", tier);
        sql.push_str(&filter);
        count_sql.push_str(&filter);
    }

    // Add ordering and pagination
    sql.push_str(" ORDER BY ak.created_at DESC");
    sql.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

    // Execute queries
    let rows = sqlx::query(&sql).fetch_all(&state.db).await.map_err(|e| {
        error!("Failed to list API keys: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(ResponseCode::InternalServerError)),
        )
    })?;

    let total: i64 = sqlx::query_scalar(&count_sql)
        .fetch_one(&state.db)
        .await
        .map_err(|e| {
            error!("Failed to count API keys: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            )
        })?;

    // Convert rows to response format
    let api_keys: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            let permissions: Vec<String> =
                serde_json::from_value(row.get("permissions")).unwrap_or_default();
            let api_key: String = row.get("api_key");
            let api_key_preview = format!("{}...", &api_key[..8.min(api_key.len())]);

            serde_json::json!({
                "id": row.get::<i32, _>("id"),
                "name": row.get::<String, _>("name"),
                "api_key_preview": api_key_preview,
                "user_id": row.get::<i32, _>("user_id"),
                "username": row.get::<String, _>("username"),
                "email": row.get::<String, _>("email"),
                "permissions": permissions,
                "rate_limit_tier": row.get::<String, _>("rate_limit_tier"),
                "is_active": row.get::<bool, _>("is_active"),
                "expires_at": row.get::<Option<DateTime<Utc>>, _>("expires_at"),
                "last_used_at": row.get::<Option<DateTime<Utc>>, _>("last_used_at"),
                "created_at": row.get::<DateTime<Utc>, _>("created_at"),
                "updated_at": row.get::<DateTime<Utc>, _>("updated_at")
            })
        })
        .collect();

    let response_data = serde_json::json!({
        "api_keys": api_keys,
        "total": total,
        "page": page,
        "limit": limit,
        "total_pages": ((total as f64) / (limit as f64)).ceil() as i32
    });

    Ok(Json(ApiResponse::success(response_data)))
}

/// Create API key (admin only)
/// POST /admin/api-keys
pub async fn create_api_key(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateApiKeyRequest>,
    // TODO: Add admin authorization middleware and get user_id from auth
) -> Result<Json<ApiResponse<ApiKeyResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Admin create API key request");

    // Validate input
    if let Err(validation_errors) = payload.validate() {
        warn!(
            "API key creation validation failed: {:?}",
            validation_errors
        );
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // TODO: Get user_id from authentication middleware
    let user_id = 1; // Placeholder - should come from auth middleware

    let api_key = ApiKey::create(&state.db, user_id, payload)
        .await
        .map_err(|e| {
            error!("Failed to create API key: {}", e);
            match e {
                ApiError::Conflict => (
                    StatusCode::CONFLICT,
                    Json(ApiResponse::error(ResponseCode::BadRequest)),
                ),
                ApiError::ValidationFailed => (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::error(ResponseCode::BadRequest)),
                ),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::error(ResponseCode::InternalServerError)),
                ),
            }
        })?;

    info!("API key created successfully with ID: {}", api_key.id);
    Ok(Json(ApiResponse::success(api_key)))
}

/// Revoke API key (admin only)
/// DELETE /admin/api-keys/:id
pub async fn revoke_api_key(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    // TODO: Add admin authorization middleware
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Admin revoke API key request for ID: {}", id);

    ApiKey::delete(&state.db, id).await.map_err(|e| {
        error!("Failed to revoke API key: {}", e);
        match e {
            ApiError::NotFound => (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::error(ResponseCode::NotFound)),
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            ),
        }
    })?;

    info!("API key {} revoked successfully", id);
    Ok(Json(ApiResponse::success(serde_json::json!({
        "revoked": true,
        "id": id
    }))))
}

/// Get API key statistics (admin only)
/// GET /admin/api-keys/stats
pub async fn get_api_key_stats(
    State(state): State<Arc<AppState>>,
    // TODO: Add admin authorization middleware
) -> Result<Json<ApiResponse<ApiKeyStats>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Admin get API key statistics request");

    let stats = ApiKey::get_stats(&state.db).await.map_err(|e| {
        error!("Failed to get API key statistics: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(ResponseCode::InternalServerError)),
        )
    })?;

    Ok(Json(ApiResponse::success(stats)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_create_user_validation() {
        let request = AdminCreateUserRequest {
            username: "a".to_string(), // Too short
            email: "invalid-email".to_string(),
            password: Some("123".to_string()), // Too short
            role: "scientist".to_string(),
            wallet_address: None,
        };

        assert!(request.validate().is_err());
    }

    #[test]
    fn test_admin_create_user_valid() {
        let request = AdminCreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: Some("password123".to_string()),
            role: "scientist".to_string(),
            wallet_address: Some("0x1234567890123456789012345678901234567890".to_string()),
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_admin_create_user_committee_role() {
        let request = AdminCreateUserRequest {
            username: "committee_user".to_string(),
            email: "committee@example.com".to_string(),
            password: Some("password123".to_string()),
            role: "committee".to_string(),
            wallet_address: Some("0x1234567890123456789012345678901234567890".to_string()),
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_admin_update_user_validation() {
        let request = AdminUpdateUserRequest {
            username: Some("a".to_string()), // Too short
            email: Some("invalid-email".to_string()),
            role: Some("invalid_role".to_string()),
            status: Some("invalid_status".to_string()),
            wallet_address: None,
        };

        assert!(request.validate().is_err());
    }

    #[test]
    fn test_admin_update_user_valid() {
        let request = AdminUpdateUserRequest {
            username: Some("updated_user".to_string()),
            email: Some("updated@example.com".to_string()),
            role: Some("committee".to_string()),
            status: Some("active".to_string()),
            wallet_address: Some("0x1234567890123456789012345678901234567890".to_string()),
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_create_api_key_validation() {
        let request = CreateApiKeyRequest {
            name: "".to_string(), // Invalid: empty name
            permissions: None,
            rate_limit_tier: None,
            expires_at: None,
        };

        assert!(request.validate().is_err());
    }

    #[test]
    fn test_create_api_key_valid() {
        let request = CreateApiKeyRequest {
            name: "Test API Key".to_string(),
            permissions: Some(vec!["api_key.read".to_string()]),
            rate_limit_tier: Some(crate::models::api_key::RateLimitTier::Premium),
            expires_at: None,
        };

        assert!(request.validate().is_ok());
    }
}
