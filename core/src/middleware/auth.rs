//! Simplified authentication middleware for Core service

use crate::models::api_key::ApiKey;
use crate::{
    AppState,
    models::user::User,
    utils::jwt::{extract_user_id, verify_token},
};
use avinapi::prelude::AppError;
use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use axum::{extract::FromRequestParts, http::request::Parts};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use cookie::time::Duration as TimeDuration;
use tracing::{debug, error};

// Cookie configuration constants
const ACCESS_TOKEN_COOKIE: &str = "access_token";
const REFRESH_TOKEN_COOKIE: &str = "refresh_token";
const ACCESS_TOKEN_DURATION_HOURS: i64 = 24;
const REFRESH_TOKEN_DURATION_DAYS: i64 = 30;

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

/// Flexible authentication middleware - accepts JWT (from header or cookie) and API key
pub async fn flexible_auth_middleware(
    State(state): State<AppState>,
    jar: CookieJar,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Try API key first (takes precedence)
    if let Some(api_key) = extract_api_key(&req) {
        debug!("Using API key authentication");
        let (key_record, user) =
            validate_apikey_and_get_user(&state, &api_key, req.uri().path(), req.method().as_str())
                .await?;
        req.extensions_mut()
            .insert(AuthUser::from_api_key(user, key_record));
        return Ok(next.run(req).await);
    }

    // Try JWT from httpOnly cookie
    if let Some(token) = extract_access_token(&jar) {
        debug!("Using JWT authentication from cookie");
        let user = validate_jwt_and_get_user(&state, &token).await?;
        req.extensions_mut().insert(AuthUser::from_jwt(user, token));
        return Ok(next.run(req).await);
    }

    Err(AppError::Authentication(
        "Authentication required. Please provide either a JWT token (in Authorization header or cookie) or API key.".into(),
    ))
}

// ============================================================================
// Helper Functions
// ============================================================================

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
async fn validate_jwt_and_get_user(state: &AppState, token: &str) -> Result<User, AppError> {
    // Validate the JWT token using the actual JWT service
    let _claims = verify_token(token, &state.jwt_config)?;

    // Extract user ID from the token
    let user_id = extract_user_id(token, &state.jwt_config)?;
    debug!("extracted user id: {}", user_id);

    // Fetch the user from database
    User::find_by_id(&state.db, user_id)
        .await
        .map_err(|_| AppError::Authentication("Failed to fetch user".into()))?
        .ok_or_else(|| AppError::Authentication("User not found".into()))
}

/// Validate API key and return key record and user
async fn validate_apikey_and_get_user(
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

// ============================================================================
// Cookie Helper Functions
// ============================================================================

/// Set authentication cookies with httpOnly flag
pub fn set_auth_cookies(jar: CookieJar, access_token: &str, refresh_token: &str) -> CookieJar {
    // Set access token cookie
    let access_cookie = Cookie::build((ACCESS_TOKEN_COOKIE, access_token.to_string()))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(TimeDuration::hours(ACCESS_TOKEN_DURATION_HOURS))
        .build();

    let jar = jar.add(access_cookie);

    // Set refresh token cookie
    let refresh_cookie = Cookie::build((REFRESH_TOKEN_COOKIE, refresh_token.to_string()))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(TimeDuration::days(REFRESH_TOKEN_DURATION_DAYS))
        .build();

    let jar = jar.add(refresh_cookie);

    debug!("Auth cookies set successfully");
    jar
}

/// Clear authentication cookies
pub fn clear_auth_cookies(jar: CookieJar) -> CookieJar {
    // Create expired access token cookie with same path and attributes
    let access_cookie = Cookie::build((ACCESS_TOKEN_COOKIE, ""))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(TimeDuration::seconds(0)) // Set to 0 to expire immediately
        .build();

    let jar = jar.add(access_cookie);

    // Create expired refresh token cookie with same path and attributes
    let refresh_cookie = Cookie::build((REFRESH_TOKEN_COOKIE, ""))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(TimeDuration::seconds(0)) // Set to 0 to expire immediately
        .build();

    let jar = jar.add(refresh_cookie);

    debug!("Auth cookies cleared");
    jar
}

/// Extract access token from cookies
pub fn extract_access_token(jar: &CookieJar) -> Option<String> {
    jar.get(ACCESS_TOKEN_COOKIE)
        .map(|cookie| cookie.value().to_string())
}

/// Extract refresh token from cookies
pub fn extract_refresh_token(jar: &CookieJar) -> Option<String> {
    jar.get(REFRESH_TOKEN_COOKIE)
        .map(|cookie| cookie.value().to_string())
}

// ============================================================================
// Middleware Functions
// ============================================================================

/// Cookie-based authentication middleware
/// Extracts JWT from httpOnly cookies and validates it
pub async fn cookie_auth_middleware(
    State(state): State<AppState>,
    jar: CookieJar,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Try to extract access token from cookies
    let token = extract_access_token(&jar).ok_or_else(|| {
        debug!("No access token found in cookies");
        AppError::Authentication("Authentication required".into())
    })?;

    // Validate token and get user
    match validate_jwt_and_get_user(&state, &token).await {
        Ok(user) => {
            // Store auth context in request extensions
            req.extensions_mut().insert(AuthUser::from_jwt(user, token));
            Ok(next.run(req).await)
        }
        Err(e) => {
            error!("Token validation failed: {:?}", e);
            Err(e)
        }
    }
}

/// Optional cookie authentication middleware
/// Tries to authenticate via cookies but doesn't fail if no cookies present
pub async fn optional_cookie_auth_middleware(
    State(state): State<AppState>,
    jar: CookieJar,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Try to extract access token from cookies
    if let Some(token) = extract_access_token(&jar) {
        // Try to validate token
        if let Ok(user) = validate_jwt_and_get_user(&state, &token).await {
            // Store auth context if successful
            req.extensions_mut().insert(AuthUser::from_jwt(user, token));
        }
    }

    // Continue regardless of authentication status
    Ok(next.run(req).await)
}

/// Admin-only cookie authentication middleware
pub async fn admin_cookie_auth_middleware(
    State(state): State<AppState>,
    jar: CookieJar,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Extract and validate token from cookies
    let token = extract_access_token(&jar)
        .ok_or_else(|| AppError::Authentication("Admin access requires authentication".into()))?;

    // Validate token and get user
    let user = validate_jwt_and_get_user(&state, &token).await?;

    // Check admin role
    if user.role != "admin" {
        return Err(AppError::Authorization("Admin access required".to_string()));
    }

    // Store auth context
    req.extensions_mut().insert(AuthUser::from_jwt(user, token));

    Ok(next.run(req).await)
}
