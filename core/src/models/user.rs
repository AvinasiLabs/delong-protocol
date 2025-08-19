//! User-related models for the DeLong Protocol
//!
//! This module contains all data structures related to user management,
//! including user accounts, authentication, and user operations.

use crate::{
    AppError,
    handlers::{admin::UserListResponse, auth::UserResponse},
    models::PaginationResponse,
};
use bcrypt::{DEFAULT_COST, hash, verify};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

/// User role enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Scientist,
    Admin,
    Moderator,
    Guest,
}

impl Default for UserRole {
    fn default() -> Self {
        UserRole::Scientist
    }
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Scientist => write!(f, "scientist"),
            UserRole::Admin => write!(f, "admin"),
            UserRole::Moderator => write!(f, "moderator"),
            UserRole::Guest => write!(f, "guest"),
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "scientist" => Ok(UserRole::Scientist),
            "admin" => Ok(UserRole::Admin),
            "moderator" => Ok(UserRole::Moderator),
            "guest" => Ok(UserRole::Guest),
            _ => Err(AppError::Validation(format!("Invalid role: {}", s))),
        }
    }
}

/// User status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "user_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    Active,
    Inactive,
    Suspended,
    Pending,
}

impl Default for UserStatus {
    fn default() -> Self {
        UserStatus::Active
    }
}

impl std::fmt::Display for UserStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserStatus::Active => write!(f, "active"),
            UserStatus::Inactive => write!(f, "inactive"),
            UserStatus::Suspended => write!(f, "suspended"),
            UserStatus::Pending => write!(f, "pending"),
        }
    }
}

impl std::str::FromStr for UserStatus {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(UserStatus::Active),
            "inactive" => Ok(UserStatus::Inactive),
            "suspended" => Ok(UserStatus::Suspended),
            "pending" => Ok(UserStatus::Pending),
            _ => Err(AppError::Validation(format!("Invalid status: {}", s))),
        }
    }
}

/// Google user data for OAuth
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GoogleUserData {
    pub id: String,
    pub email: String,
    pub verified_email: bool,
    pub name: String,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub picture: Option<String>,
    pub locale: Option<String>,
}

/// User model matching PostgreSQL schema and API requirements
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub username: String,
    pub password_hash: Option<String>,
    pub role: String,
    pub status: String,
    pub wallet_address: Option<String>,
    pub google_id: Option<String>,
    pub avatar_url: Option<String>,
    pub provider: String,
    pub provider_data: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub last_provider_sync: Option<DateTime<Utc>>,
    pub email_verified: Option<bool>,
    pub two_factor_enabled: Option<bool>,
    pub profile_data: serde_json::Value,
}

/// Query parameters for user list endpoints
#[derive(Debug, Serialize, Deserialize)]
pub struct UserQueryParams {
    pub page: Option<i32>,
    pub limit: Option<i32>,
    pub role: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
}

/// Role information model
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct RoleInfo {
    pub id: i32,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub level: i32,
    pub is_system: Option<bool>,
}

/// Permission information model
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct PermissionInfo {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub resource: String,
    pub action: String,
}

impl User {
    /// Create a new user
    pub async fn create(
        pool: &PgPool,
        username: &str,
        email: &str,
        password: Option<&str>,
        role: Option<&str>,
        wallet_address: Option<&str>,
    ) -> Result<Self, AppError> {
        let password_hash = if let Some(pwd) = password {
            Some(hash_password(pwd)?)
        } else {
            None
        };

        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (username, email, password_hash, role, wallet_address)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, username, email, password_hash, role, status, wallet_address,
                      google_id, avatar_url, provider, provider_data, created_at,
                      updated_at, last_login, last_provider_sync, email_verified,
                      two_factor_enabled, profile_data
            "#,
        )
        .bind(username)
        .bind(email)
        .bind(password_hash)
        .bind(role.unwrap_or("scientist"))
        .bind(wallet_address)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    /// Find user by email
    pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<Option<Self>, AppError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, username, email, password_hash, role, status, wallet_address,
                   google_id, avatar_url, provider, provider_data, created_at,
                   updated_at, last_login, last_provider_sync, email_verified,
                   two_factor_enabled, profile_data
            FROM users
            WHERE email = $1
            "#,
        )
        .bind(email)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    /// Find user by ID
    pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<Self>, AppError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, username, email, password_hash, role, status, wallet_address,
                   google_id, avatar_url, provider, provider_data, created_at,
                   updated_at, last_login, last_provider_sync, email_verified,
                   two_factor_enabled, profile_data
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    /// Find user by Google ID
    pub async fn find_by_google_id(
        pool: &PgPool,
        google_id: &str,
    ) -> Result<Option<Self>, AppError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, username, email, password_hash, role, status, wallet_address,
                   google_id, avatar_url, provider, provider_data, created_at,
                   updated_at, last_login, last_provider_sync, email_verified,
                   two_factor_enabled, profile_data
            FROM users
            WHERE google_id = $1
            "#,
        )
        .bind(google_id)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    /// Update user
    pub async fn update(
        pool: &PgPool,
        id: i32,
        username: Option<&str>,
        email: Option<&str>,
        role: Option<&str>,
        status: Option<&str>,
        wallet_address: Option<&str>,
        avatar_url: Option<&str>,
        email_verified: Option<bool>,
    ) -> Result<Self, AppError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET username = COALESCE($1, username),
                email = COALESCE($2, email),
                role = COALESCE($3, role),
                status = COALESCE($4, status),
                wallet_address = COALESCE($5, wallet_address),
                avatar_url = COALESCE($6, avatar_url),
                email_verified = COALESCE($7, email_verified),
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $8
            RETURNING id, username, email, password_hash, role, status, wallet_address,
                      google_id, avatar_url, provider, provider_data, created_at,
                      updated_at, last_login, last_provider_sync, email_verified,
                      two_factor_enabled, profile_data
            "#,
        )
        .bind(username)
        .bind(email)
        .bind(role)
        .bind(status)
        .bind(wallet_address)
        .bind(avatar_url)
        .bind(email_verified)
        .bind(id)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    /// Update last login time
    pub async fn update_last_login(pool: &PgPool, id: i32) -> Result<(), AppError> {
        sqlx::query("UPDATE users SET last_login = CURRENT_TIMESTAMP WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Update wallet address
    pub async fn update_wallet_address(
        pool: &PgPool,
        id: i32,
        wallet_address: Option<String>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE users SET wallet_address = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
        )
        .bind(wallet_address)
        .bind(id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Create user from Google OAuth data
    pub async fn create_google_user(
        pool: &PgPool,
        google_data: GoogleUserData,
    ) -> Result<Self, AppError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (username, email, google_id, avatar_url, provider,
                             provider_data, email_verified)
            VALUES ($1, $2, $3, $4, 'google', $5, $6)
            RETURNING id, username, email, password_hash, role, status, wallet_address,
                      google_id, avatar_url, provider, provider_data, created_at,
                      updated_at, last_login, last_provider_sync, email_verified,
                      two_factor_enabled, profile_data
            "#,
        )
        .bind(&google_data.name)
        .bind(&google_data.email)
        .bind(&google_data.id)
        .bind(&google_data.picture)
        .bind(serde_json::to_value(&google_data)?)
        .bind(Some(google_data.verified_email))
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    /// Link Google account to existing user
    pub async fn link_google_account(
        pool: &PgPool,
        user_id: i32,
        google_data: GoogleUserData,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE users
            SET google_id = $1,
                avatar_url = COALESCE($2, avatar_url),
                provider_data = $3,
                last_provider_sync = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $4
            "#,
        )
        .bind(google_data.id.clone())
        .bind(google_data.picture.clone())
        .bind(serde_json::to_value(&google_data)?)
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Get paginated list of users
    pub async fn get_users(
        pool: &PgPool,
        params: UserQueryParams,
    ) -> Result<UserListResponse, AppError> {
        let page = params.page.unwrap_or(1).max(1);
        let limit = params.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        // Build base query
        let mut query = String::from(
            r#"
            SELECT id, username, email, password_hash, role, status, wallet_address,
                   google_id, avatar_url, provider, provider_data, created_at,
                   updated_at, last_login, last_provider_sync, email_verified,
                   two_factor_enabled, profile_data
            FROM users
            WHERE 1=1
            "#,
        );

        let mut count_query = String::from("SELECT COUNT(*) FROM users WHERE 1=1");

        // Add filters
        if let Some(role) = &params.role {
            query.push_str(&format!(" AND role = '{}'", role));
            count_query.push_str(&format!(" AND role = '{}'", role));
        }

        if let Some(status) = &params.status {
            query.push_str(&format!(" AND status = '{}'", status));
            count_query.push_str(&format!(" AND status = '{}'", status));
        }

        if let Some(search) = &params.search {
            let search_condition = format!(
                " AND (username ILIKE '%{}%' OR email ILIKE '%{}%')",
                search, search
            );
            query.push_str(&search_condition);
            count_query.push_str(&search_condition);
        }

        // Add ordering and pagination
        query.push_str(" ORDER BY created_at DESC");
        query.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

        // Execute queries
        let users: Vec<User> = sqlx::query_as(&query).fetch_all(pool).await?;

        let total: i64 = sqlx::query_scalar(&count_query).fetch_one(pool).await?;

        let total_pages = ((total as f64) / (limit as f64)).ceil() as i32;

        Ok(UserListResponse {
            users: users.into_iter().map(UserResponse::from).collect(),
            pagination: PaginationResponse {
                page,
                limit,
                total,
                total_pages,
            },
        })
    }

    /// Delete user
    pub async fn delete(pool: &PgPool, id: i32) -> Result<(), AppError> {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Verify password
    pub fn verify_password(&self, password: &str) -> bool {
        self.password_hash
            .as_ref()
            .map_or(false, |hash| verify(password, hash).unwrap_or(false))
    }

    /// Check if user has specific role
    pub fn has_role(&self, role: &str) -> bool {
        self.role == role
    }

    /// Check if user is admin
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }

    /// Check if user is active
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }

    /// Check if user email is verified
    pub fn is_email_verified(&self) -> bool {
        self.email_verified.unwrap_or(false)
    }
}

/// Hash password using bcrypt
pub fn hash_password(password: &str) -> Result<String, AppError> {
    hash(password, DEFAULT_COST)
        .map_err(|e| AppError::Internal(format!("Failed to hash password: {}", e)))
}

/// Get roles and permissions
pub async fn get_roles_and_permissions(
    pool: &PgPool,
) -> Result<(Vec<RoleInfo>, Vec<PermissionInfo>), AppError> {
    let roles = sqlx::query_as::<_, RoleInfo>(
        r#"
        SELECT id, name, display_name, description, level, is_system
        FROM roles
        ORDER BY level
        "#,
    )
    .fetch_all(pool)
    .await?;

    let permissions = sqlx::query_as::<_, PermissionInfo>(
        r#"
        SELECT id, name, description, resource, action
        FROM permissions
        ORDER BY resource, action
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok((roles, permissions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_role_from_str() {
        assert_eq!("admin".parse::<UserRole>().unwrap(), UserRole::Admin);
        assert_eq!(
            "scientist".parse::<UserRole>().unwrap(),
            UserRole::Scientist
        );
        assert_eq!(
            "moderator".parse::<UserRole>().unwrap(),
            UserRole::Moderator
        );
        assert_eq!("guest".parse::<UserRole>().unwrap(), UserRole::Guest);
    }

    #[test]
    fn test_user_status_from_str() {
        assert_eq!("active".parse::<UserStatus>().unwrap(), UserStatus::Active);
        assert_eq!(
            "inactive".parse::<UserStatus>().unwrap(),
            UserStatus::Inactive
        );
        assert_eq!(
            "suspended".parse::<UserStatus>().unwrap(),
            UserStatus::Suspended
        );
        assert_eq!(
            "pending".parse::<UserStatus>().unwrap(),
            UserStatus::Pending
        );
    }

    #[test]
    fn test_hash_password() {
        let password = "test_password_123";
        let hash = hash_password(password).unwrap();
        assert!(verify(password, &hash).unwrap());
    }

    #[test]
    fn test_user_response_from_user() {
        let user = User {
            id: 1,
            email: "test@example.com".to_string(),
            username: "testuser".to_string(),
            password_hash: Some("hashed_password".to_string()),
            role: "scientist".to_string(),
            status: "active".to_string(),
            wallet_address: Some("0x123...".to_string()),
            google_id: None,
            avatar_url: None,
            provider: "local".to_string(),
            provider_data: serde_json::Value::Null,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login: None,
            last_provider_sync: None,
            email_verified: Some(true),
            two_factor_enabled: Some(false),
            profile_data: serde_json::Value::Null,
        };

        let response = UserResponse::from(user.clone());
        assert_eq!(response.id, user.id);
        assert_eq!(response.email, user.email);
        assert_eq!(response.username, user.username);
        assert_eq!(response.role, user.role);
        assert_eq!(response.status, user.status);
        assert_eq!(response.wallet_address, user.wallet_address);
    }
}
