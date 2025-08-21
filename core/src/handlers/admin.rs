//! Admin handlers
//!
//! This module contains HTTP handlers for administrative functions,
//! including user management, role/permission management, and API key management.

use avinapi::prelude::{AppError, JsonResult, ValidatedJson, ValidatedQuery, data};
use axum::extract::{Path, State};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::warn;
use tracing::{error, info};
use uuid::Uuid;
use validator::Validate;

use crate::{
    handlers::auth::UserResponse,
    models::{
        PaginationResponse,
        user::{User, UserQueryParams, get_roles_and_permissions},
    },
    routes::AppState,
};

// ===== Request Structures =====

/// Create user request (admin)
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateUserRequest {
    #[validate(length(min = 2, max = 50, message = "Username must be 2-50 characters"))]
    pub username: String,
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    #[validate(length(min = 8, max = 128, message = "Password must be 8-128 characters"))]
    pub password: Option<String>,
    pub role: Option<String>,
    pub wallet_address: Option<String>,
}

/// Update user request (admin)
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateUserRequest {
    #[validate(length(min = 2, max = 50, message = "Username must be 2-50 characters"))]
    pub username: Option<String>,
    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
    pub wallet_address: Option<String>,
    pub avatar_url: Option<String>,
    pub email_verified: Option<bool>,
}

/// Create role request (admin)
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateRoleRequest {
    #[validate(length(min = 1, max = 100, message = "Role name must be 1-100 characters"))]
    pub name: String,
    pub description: Option<String>,
    pub permission_ids: Vec<Uuid>,
}

/// Update role request (admin)
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateRoleRequest {
    #[validate(length(min = 1, max = 100, message = "Role name must be 1-100 characters"))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub permission_ids: Option<Vec<Uuid>>,
}

/// Assign role request (admin)
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AssignRoleRequest {
    pub user_id: Uuid,
    pub role_id: Uuid,
    pub expires_at: Option<DateTime<Utc>>,
}

// ===== Response Structures =====

/// User list response
#[derive(Debug, Serialize)]
pub struct UserListResponse {
    pub users: Vec<UserResponse>,
    pub pagination: PaginationResponse,
}

/// Roles and permissions response
#[derive(Debug, Serialize)]
pub struct RolesPermissionsResponse {
    pub roles: Vec<RoleInfo>,
    pub permissions: Vec<PermissionInfo>,
}

/// Role information
#[derive(Debug, Serialize)]
pub struct RoleInfo {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub permission_count: i64,
}

/// Permission information
#[derive(Debug, Serialize)]
pub struct PermissionInfo {
    pub id: i32,
    pub name: String,
    pub resource: String,
    pub action: String,
    pub description: Option<String>,
}

/// Query parameters for admin user listing
/// Admin user query parameters
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AdminUserQuery {
    pub role: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    #[validate(range(min = 1, message = "Page must be at least 1"))]
    pub page: Option<i32>,
    #[validate(range(min = 1, max = 100, message = "Limit must be between 1 and 100"))]
    pub limit: Option<i32>,
}

/// Admin user response
#[derive(Debug, serde::Serialize)]
pub struct AdminUserResponse {
    pub user: UserResponse,
    pub message: String,
}

/// Permission check request
#[derive(Debug, Serialize, Deserialize, Validate, Clone)]
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
    State(state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<AdminUserQuery>,
) -> JsonResult<UserListResponse> {
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
        e
    })?;

    data!(user_list)
}

/// Create user (admin only)
/// POST /admin/users
pub async fn create_user_admin(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<CreateUserRequest>,
) -> JsonResult<UserResponse> {
    info!("Admin create user request for email: {}", payload.email);

    // Validate role - include committee role support
    let role = payload.role.as_deref().unwrap_or("scientist");
    let valid_roles = ["scientist", "committee", "admin"];
    if !valid_roles.contains(&role) {
        warn!("Invalid role provided: {}", role);
        return Err(AppError::Validation(format!("Invalid role: {}", role)));
    }

    // Check if user already exists
    if let Ok(Some(_)) = User::find_by_email(&state.db, &payload.email).await {
        warn!("User already exists with email: {}", payload.email);
        return Err(AppError::Conflict(
            "User with this email already exists".to_string(),
        ));
    }

    // Create user
    let user = User::create(
        &state.db,
        &payload.username,
        &payload.email,
        payload.password.as_deref(),
        payload.role.as_deref(),
        payload.wallet_address.as_deref(),
    )
    .await
    .map_err(|e| {
        error!("Failed to create user: {}", e);
        e
    })?;

    info!("User created successfully by admin: {}", user.email);

    let response: UserResponse = user.into();
    data!(response)
}

/// Get user by ID (admin only)
/// GET /admin/users/:id
pub async fn get_user_by_id(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> JsonResult<UserResponse> {
    info!("Admin get user by ID request: {}", id);

    // Find user by ID
    let user = User::find_by_id(&state.db, id)
        .await
        .map_err(|e| {
            error!("Failed to find user: {}", e);
            e
        })?
        .ok_or_else(|| {
            warn!("User not found with ID: {}", id);
            AppError::NotFound(format!("User {} not found", id))
        })?;

    data!(user.into())
}

/// Update user (admin only)
/// PUT /admin/users/:id
pub async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    ValidatedJson(payload): ValidatedJson<UpdateUserRequest>,
) -> JsonResult<UserResponse> {
    info!("Admin update user request for ID: {}", id);

    // Validate role if provided
    if let Some(ref role) = payload.role {
        let valid_roles = ["scientist", "committee", "admin"];
        if !valid_roles.contains(&role.as_str()) {
            warn!("Invalid role provided: {}", role);
            return Err(AppError::Validation(format!("Invalid role: {}", role)));
        }
    }

    // Validate status if provided
    if let Some(ref status) = payload.status {
        let valid_statuses = ["active", "inactive", "suspended"];
        if !valid_statuses.contains(&status.as_str()) {
            warn!("Invalid status provided: {}", status);
            return Err(AppError::Validation(format!("Invalid status: {}", status)));
        }
    }

    // Update user
    let updated_user = User::update_partial(
        &state.db,
        id,
        payload.username.as_deref(),
        payload.email.as_deref(),
        payload.role.as_deref(),
        payload.status.as_deref(),
        payload.wallet_address.as_deref(),
        payload.avatar_url.as_deref(),
        payload.email_verified,
    )
    .await
    .map_err(|e| {
        error!("Failed to update user: {}", e);
        e
    })?;

    info!("User updated successfully by admin: {}", updated_user.email);

    data!(updated_user.into())
}

/// Get all roles and permissions (admin only)
/// GET /admin/roles
pub async fn get_roles(State(state): State<AppState>) -> JsonResult<RolesPermissionsResponse> {
    info!("Admin get roles and permissions request");

    let (roles, permissions) = get_roles_and_permissions(&state.db).await.map_err(|e| {
        error!("Failed to get roles and permissions: {}", e);
        e
    })?;

    let response = RolesPermissionsResponse {
        roles: roles
            .into_iter()
            .map(|r| RoleInfo {
                id: r.id,
                name: r.name,
                description: r.description,
                permission_count: 0, // TODO: Calculate actual permission count
            })
            .collect(),
        permissions: permissions
            .into_iter()
            .map(|p| PermissionInfo {
                id: p.id,
                name: p.name,
                resource: p.resource,
                action: p.action,
                description: p.description,
            })
            .collect(),
    };

    data!(response)
}

/// Get permissions list (admin only)
/// GET /admin/permissions
pub async fn get_permissions(State(state): State<AppState>) -> JsonResult<Vec<PermissionInfo>> {
    info!("Admin get permissions request");

    let (_roles, permissions) = get_roles_and_permissions(&state.db).await.map_err(|e| {
        error!("Failed to get permissions: {}", e);
        e
    })?;

    let permission_infos: Vec<PermissionInfo> = permissions
        .into_iter()
        .map(|p| PermissionInfo {
            id: p.id,
            name: p.name,
            resource: p.resource,
            action: p.action,
            description: p.description,
        })
        .collect();

    data!(permission_infos)
}

/// Check user permission (admin only)
/// GET /admin/permissions/:user_id/:permission
pub async fn check_permission(
    State(state): State<AppState>,
    Path((user_id, permission)): Path<(i32, String)>,
) -> JsonResult<PermissionCheckResponse> {
    info!(
        "Admin check permission request for user {} and permission {}",
        user_id, permission
    );

    // Find user
    // Get user
    let user = User::find_by_id(&state.db, user_id)
        .await
        .map_err(|e| {
            error!("Failed to find user: {}", e);
            e
        })?
        .ok_or_else(|| {
            warn!("User not found for permission check: {}", user_id);
            AppError::NotFound(format!("User {} not found", user_id))
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
        e
    })?;

    let response = PermissionCheckResponse {
        user_id,
        permission,
        has_permission,
        user_role: user.role.clone(),
    };

    data!(response)
}
