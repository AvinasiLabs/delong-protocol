//! Simplified authentication middleware for Core service

use crate::{
    AppState,
    models::{api_key::ApiKey, user::User},
    utils::jwt::{extract_user_id, verify_token},
};
use avinapi::prelude::AppError;
use axum::{
    body::Body,
    extract::{FromRequestParts, State},
    http::{Request, request::Parts},
    middleware::Next,
    response::Response,
};
use tracing::{debug, error};

// ============================================================================
// Core Types
// ============================================================================

/// Authenticated user with authentication details
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user: User,
    pub auth_method: AuthMethod,
    pub token: Option<String>,   // JWT token if JWT auth
    pub api_key: Option<ApiKey>, // API key record if API key auth
}

/// Authentication method
#[derive(Debug, Clone, PartialEq)]
pub enum AuthMethod {
    Jwt,
    ApiKey,
}

impl AuthUser {
    /// Create from JWT authentication
    pub fn from_jwt(user: User, token: String) -> Self {
        Self {
            user,
            auth_method: AuthMethod::Jwt,
            token: Some(token),
            api_key: None,
        }
    }

    /// Create from API key authentication
    pub fn from_api_key(user: User, api_key: ApiKey) -> Self {
        Self {
            user,
            auth_method: AuthMethod::ApiKey,
            token: None,
            api_key: Some(api_key),
        }
    }

    /// Get user ID
    pub fn user_id(&self) -> i32 {
        self.user.id
    }

    /// Get user email
    pub fn email(&self) -> &str {
        &self.user.email
    }

    /// Check if user is admin
    pub fn is_admin(&self) -> bool {
        self.user.role == "admin"
    }

    /// Check if authenticated via JWT
    pub fn is_jwt_auth(&self) -> bool {
        self.auth_method == AuthMethod::Jwt
    }

    /// Check if authenticated via API key
    pub fn is_api_key_auth(&self) -> bool {
        self.auth_method == AuthMethod::ApiKey
    }
}

// ============================================================================
// Extractors
// ============================================================================

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or_else(|| AppError::Authentication("No authentication context found".into()))
    }
}

/// Extractor for admin users
pub struct AdminUser(pub AuthUser);

impl<S> FromRequestParts<S> for AdminUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        if !auth_user.is_admin() {
            return Err(AppError::Authorization("Admin access required".to_string()));
        }

        Ok(AdminUser(auth_user))
    }
}

// ============================================================================
// Middleware Functions
// ============================================================================

/// JWT-only authentication middleware
pub async fn jwt_only_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Check for API key (not allowed)
    if extract_api_key(&req).is_some() {
        return Err(AppError::Authentication(
            "This endpoint requires JWT authentication. API keys are not accepted.".into(),
        ));
    }

    // Extract and validate JWT
    let token = extract_jwt_token(&req)
        .ok_or_else(|| AppError::Authentication("JWT token required".into()))?;

    // Validate token and get user
    let user = validate_jwt_token(&state, &token).await?;

    // Store auth context
    req.extensions_mut().insert(AuthUser::from_jwt(user, token));

    Ok(next.run(req).await)
}

/// API key-only authentication middleware
pub async fn api_key_only_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Check for JWT (not allowed)
    if extract_jwt_token(&req).is_some() {
        return Err(AppError::Authentication(
            "This endpoint requires API key authentication. JWT tokens are not accepted.".into(),
        ));
    }

    // Extract and validate API key
    let api_key =
        extract_api_key(&req).ok_or_else(|| AppError::Authentication("API key required".into()))?;

    let (key_record, user) =
        validate_api_key(&state, &api_key, req.uri().path(), req.method().as_str()).await?;

    // Store auth context
    req.extensions_mut()
        .insert(AuthUser::from_api_key(user, key_record));

    Ok(next.run(req).await)
}

/// Flexible authentication middleware - accepts both JWT and API key
pub async fn flexible_auth_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Try API key first (takes precedence)
    if let Some(api_key) = extract_api_key(&req) {
        debug!("Using API key authentication");
        let (key_record, user) =
            validate_api_key(&state, &api_key, req.uri().path(), req.method().as_str()).await?;
        req.extensions_mut()
            .insert(AuthUser::from_api_key(user, key_record));
        return Ok(next.run(req).await);
    }

    // Try JWT
    if let Some(token) = extract_jwt_token(&req) {
        debug!("Using JWT authentication");
        let user = validate_jwt_token(&state, &token).await?;
        req.extensions_mut().insert(AuthUser::from_jwt(user, token));
        return Ok(next.run(req).await);
    }

    Err(AppError::Authentication(
        "Authentication required. Please provide either a JWT token or API key.".into(),
    ))
}

/// Admin-only middleware (JWT authentication required)
pub async fn admin_only_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Admin endpoints only support JWT
    let token = extract_jwt_token(&req).ok_or_else(|| {
        AppError::Authentication("Admin access requires JWT authentication".into())
    })?;

    // Validate token and get user
    let user = validate_jwt_token(&state, &token).await?;

    // Check admin role
    if user.role != "admin" {
        return Err(AppError::Authorization("Admin access required".to_string()));
    }

    // Store auth context
    req.extensions_mut().insert(AuthUser::from_jwt(user, token));

    Ok(next.run(req).await)
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract JWT token from request headers
fn extract_jwt_token(req: &Request<Body>) -> Option<String> {
    req.headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|auth| {
            if auth.starts_with("Bearer ") {
                Some(auth[7..].to_string())
            } else {
                None
            }
        })
}

/// Extract API key from request headers
fn extract_api_key(req: &Request<Body>) -> Option<String> {
    // Try X-API-Key header first
    if let Some(key) = req
        .headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
    {
        return Some(key);
    }

    // Try Authorization header with ApiKey prefix
    req.headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|auth| {
            if auth.starts_with("ApiKey ") {
                Some(auth[7..].to_string())
            } else {
                None
            }
        })
}

/// Validate JWT token and return user
async fn validate_jwt_token(state: &AppState, token: &str) -> Result<User, AppError> {
    // Validate the JWT token using the actual JWT service
    let _claims = verify_token(token, &state.jwt_config)?;

    // Extract user ID from the token
    let user_id = extract_user_id(token, &state.jwt_config)?;

    // Fetch the user from database
    User::find_by_id(&state.db, user_id)
        .await
        .map_err(|_| AppError::Authentication("Failed to fetch user".into()))?
        .ok_or_else(|| AppError::Authentication("User not found".into()))
}

/// Validate API key and return key record and user
async fn validate_api_key(
    state: &AppState,
    api_key: &str,
    path: &str,
    method: &str,
) -> Result<(ApiKey, User), AppError> {
    // Find and validate the API key
    let key_record = ApiKey::find_by_key(&state.db, api_key).await?;

    // Check if active
    if !key_record.is_active {
        return Err(AppError::Authentication("API key is not active".into()));
    }

    // Check expiration
    if let Some(expires_at) = key_record.expires_at {
        if expires_at < chrono::Utc::now() {
            return Err(AppError::Authentication("API key has expired".into()));
        }
    }

    // Check path permissions
    if !key_record.can_access_path(path, method) {
        error!(
            "API key {} denied access to {} {}",
            key_record.name, method, path
        );
        return Err(AppError::Authorization(format!(
            "API key '{}' does not have permission to access {} {}",
            key_record.name, method, path
        )));
    }

    // Get associated user
    let user = User::find_by_id(&state.db, key_record.user_id)
        .await?
        .ok_or_else(|| AppError::Authentication("User not found for API key".into()))?;

    Ok((key_record, user))
}
