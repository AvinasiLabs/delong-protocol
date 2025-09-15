//! Authentication handlers
//!
//! This module contains HTTP handlers for user authentication,
//! including registration, login, verification code management,
//! and Google OAuth integration.

use avinapi::prelude::{AppError, JsonResult, ValidatedJson, ValidatedQuery, data, empty};
use axum::extract::State;
use axum_extra::extract::CookieJar;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use regex::Regex;
use std::sync::LazyLock;
use tracing::{error, info, instrument, warn};
use utoipa::ToSchema;
use validator::Validate;

use crate::{
    infra::email::EmailService,
    middleware::AuthUser,
    models::{auth::VerificationType, user::User},
    routes::AppState,
    utils::jwt::{self},
};

// ===== Validation Regex =====

/// Regex for validating Ethereum wallet addresses
pub static WALLET_ADDRESS_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^0x[a-fA-F0-9]{40}$").expect("Invalid wallet address regex"));

/// Regex for validating usernames
pub static USERNAME_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9_]{3,20}$").expect("Invalid username regex"));

// ===== Request Structures =====

/// Login request
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct LoginRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    #[validate(length(min = 6, message = "Password must be at least 6 characters"))]
    pub password: String,
}

/// User registration request
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct RegisterRequest {
    #[validate(
        length(min = 3, max = 20, message = "Username must be 3-20 characters"),
        regex(
            path = "crate::handlers::auth::USERNAME_REGEX",
            message = "Username must contain only letters, numbers, and underscores"
        )
    )]
    pub username: String,
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    #[validate(length(min = 8, max = 128, message = "Password must be 8-128 characters"))]
    pub password: String,
    #[validate(length(min = 4, max = 6, message = "Verification code must be 4-6 characters"))]
    pub verification_code: Option<String>,
}

/// Send verification code request
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct SendVerificationCodeRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    pub verification_type: VerificationType,
    pub language: Option<String>,
}

// UpdateWalletRequest removed - use SIWE link-wallet instead
// SIWE provides cryptographic proof of wallet ownership

/// Google OAuth login request
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct GoogleLoginRequest {
    pub access_token: String,
    pub id_token: Option<String>,
}

// ===== Response Structures =====

/// User response model for API responses
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    pub id: i32,
    pub email: String,
    pub username: String,
    pub role: String,
    pub status: String,
    pub wallet_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub google_id: Option<String>,
    pub avatar_url: Option<String>,
    pub provider: String,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            username: user.username,
            role: user.role,
            status: user.status,
            wallet_address: user.wallet_address,
            created_at: user.created_at,
            last_login: user.last_login,
            google_id: user.google_id,
            avatar_url: user.avatar_url,
            provider: user.provider,
        }
    }
}

/// Authentication response
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: UserResponse,
    pub expires_at: DateTime<Utc>,
}

/// Google OAuth query parameters
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct GoogleAuthQuery {
    #[serde(rename = "returnTo")]
    pub return_to: Option<String>,
}

/// Google OAuth URL response
#[derive(Debug, Serialize, ToSchema)]
pub struct GoogleAuthUrlResponse {
    #[serde(rename = "authUrl")]
    pub auth_url: String,
    pub state: String,
}

/// Google OAuth callback query parameters
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct GoogleCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// Google callback request
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct GoogleCallbackRequest {
    pub code: String,
    pub state: String,
    pub error: Option<String>,
}

/// User login handler
/// Login user
/// POST /auth/login
#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "Auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login result", body = AuthResponse)
    )
)]
pub async fn login_user(
    State(state): State<AppState>,
    jar: CookieJar,
    ValidatedJson(payload): ValidatedJson<LoginRequest>,
) -> (CookieJar, JsonResult<AuthResponse>) {
    info!("User login request for email: {}", payload.email);

    // Find user by email
    let user = match User::find_by_email(&state.db, &payload.email).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            warn!("Login failed: User not found for email: {}", payload.email);
            return (
                jar,
                Err(AppError::Authentication("User not registered".to_string())),
            );
        }
        Err(e) => {
            error!("Database error during login: {:?}", e);
            return (jar, Err(e));
        }
    };

    // Verify password
    if !user.verify_password(&payload.password) {
        warn!(
            "Login failed: Invalid password for email: {}",
            payload.email
        );
        return (
            jar,
            Err(AppError::Authentication("Invalid credentials".to_string())),
        );
    }

    // Check if user is active
    if !user.is_active() {
        warn!(
            "Login failed: Account not active for email: {}",
            payload.email
        );
        return (
            jar,
            Err(AppError::Authentication(
                "Account is not active".to_string(),
            )),
        );
    }

    // Generate JWT tokens
    let access_token = match jwt::generate_token(&user, &state.jwt_config) {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to generate JWT token: {:?}", e);
            return (
                jar,
                Err(AppError::Internal("Failed to generate token".to_string())),
            );
        }
    };

    let refresh_token = match jwt::generate_refresh_token(&user, &state.jwt_config) {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to generate refresh token: {:?}", e);
            return (
                jar,
                Err(AppError::Internal(
                    "Failed to generate refresh token".to_string(),
                )),
            );
        }
    };

    // Set httpOnly cookies
    let jar = crate::middleware::auth::set_auth_cookies(jar, &access_token, &refresh_token);

    // Update last login
    if let Err(e) = User::update_last_login(&state.db, user.id).await {
        error!("Failed to update last login: {:?}", e);
        // Don't fail the login for this
    }

    let auth_response = AuthResponse {
        access_token: String::new(),  // Don't expose token in response body
        refresh_token: String::new(), // Don't expose token in response body
        user: UserResponse::from(user),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
    };

    info!("User logged in successfully: {}", payload.email);
    (jar, data!(auth_response))
}

/// User registration handler
/// Register a new user
/// POST /auth/register
#[utoipa::path(
    post,
    path = "/auth/register",
    tag = "Auth",
    request_body = RegisterRequest,
    responses(
        (status = 200, description = "Registration result", body = AuthResponse)
    )
)]
#[instrument(skip(state, payload, jar))]
pub async fn register_user(
    State(state): State<AppState>,
    jar: CookieJar,
    ValidatedJson(payload): ValidatedJson<RegisterRequest>,
) -> (CookieJar, JsonResult<AuthResponse>) {
    info!("User registration request for email: {}", payload.email);

    if payload.verification_code.is_none() {
        return (
            jar,
            Err(AppError::Validation(
                "Verification code is required".to_string(),
            )),
        );
    }

    // Verify verification code
    let verification_code = payload.verification_code.unwrap_or_default();
    match state
        .verification_store
        .verify(&payload.email, &verification_code)
        .await
    {
        Ok(()) => {
            info!("Email verified successfully: {}", payload.email);
        }
        Err(e) => {
            error!("Verification failed: {}", e);
            return (jar, Err(e));
        }
    }

    // Check if user already exists
    if let Ok(Some(_)) = User::find_by_email(&state.db, &payload.email).await {
        warn!(
            "Registration failed: User already exists for email: {}",
            payload.email
        );
        return (
            jar,
            Err(AppError::Conflict("Email already registered".to_string())),
        );
    }

    // Create user
    let user = match User::create(
        &state.db,
        &payload.username,
        &payload.email,
        Some(&payload.password),
        Some("scientist"),
        None,
    )
    .await
    {
        Ok(user) => user,
        Err(e) => {
            error!("Failed to create user: {:?}", e);
            return (jar, Err(e));
        }
    };

    // Generate JWT tokens
    let access_token = match jwt::generate_token(&user, &state.jwt_config) {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to generate JWT token: {:?}", e);
            return (
                jar,
                Err(AppError::Internal("Failed to generate token".to_string())),
            );
        }
    };

    let refresh_token = match jwt::generate_refresh_token(&user, &state.jwt_config) {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to generate refresh token: {:?}", e);
            return (
                jar,
                Err(AppError::Internal(
                    "Failed to generate refresh token".to_string(),
                )),
            );
        }
    };

    // Set httpOnly cookies
    let jar = crate::middleware::auth::set_auth_cookies(jar, &access_token, &refresh_token);

    // Remove verification code
    state.verification_store.clear(&payload.email).await.ok();

    let auth_response = AuthResponse {
        access_token: String::new(),  // Don't expose token in response body
        refresh_token: String::new(), // Don't expose token in response body
        user: UserResponse::from(user),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
    };

    info!("User registered successfully: {}", payload.email);
    (jar, data!(auth_response))
}

/// Send verification code handler
/// Send verification code
/// POST /auth/send-code
#[utoipa::path(
    post,
    path = "/auth/send-code",
    tag = "Auth",
    request_body = SendVerificationCodeRequest,
    responses(
        (status = 200, description = "Verification code operation result")
    )
)]
pub async fn send_verification_code(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<SendVerificationCodeRequest>,
) -> JsonResult<()> {
    info!(
        "Send verification code request for email: {}",
        payload.email
    );

    // Check if code already sent recently
    if state
        .verification_store
        .exists(&payload.email)
        .await
        .unwrap_or(false)
    {
        warn!(
            "Verification code already sent recently for: {}",
            payload.email
        );
        return Err(AppError::Conflict(
            "Verification code already sent. Please wait before requesting again".to_string(),
        ));
    }

    // Generate and store verification code
    let verification_code = match state
        .verification_store
        .store(&payload.email, payload.language.as_deref().unwrap_or("en"))
        .await
    {
        Ok(code) => code,
        Err(e) => {
            error!("Failed to store verification code: {:?}", e);
            return Err(e);
        }
    };

    // Send email with verification code
    if state.config.email.enabled {
        EmailService::send_verification_code(
            &state.config.email,
            &payload.email,
            &verification_code,
        )
        .await?
    } else {
        info!(
            "Email service disabled - verification code for {}: {}",
            payload.email, verification_code
        );
    }

    empty!()
}

/// Get Google OAuth URL handler
/// GET /auth/google
/// Generate Google OAuth URL
#[utoipa::path(
    post,
    path = "/auth/google",
    tag = "Auth",
    responses(
        (status = 200, description = "OAuth URL generation result", body = GoogleAuthUrlResponse)
    )
)]
pub async fn google_auth_url(
    State(state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<GoogleAuthQuery>,
) -> JsonResult<GoogleAuthUrlResponse> {
    info!("Google OAuth URL request");

    // Generate a secure state parameter
    let state_param = crate::infra::google_oauth::generate_state_parameter();

    // Store the state in Redis with return_to URL if provided
    if let Some(redis_pool) = state.config.redis_pool.as_ref() {
        crate::infra::google_oauth::OAuthStateStore::store_state(
            redis_pool,
            &state_param,
            query.return_to.clone(),
        )
        .await
        .map_err(|e| {
            error!("Failed to store OAuth state: {}", e);
            AppError::Internal("Failed to initialize OAuth flow".to_string())
        })?;
    }

    // Generate the Google OAuth URL
    let auth_url = state
        .google_oauth_service
        .generate_auth_url(&state_param, query.return_to.as_deref());

    let response = GoogleAuthUrlResponse {
        auth_url,
        state: state_param,
    };

    data!(response)
}

/// Google OAuth callback handler
/// GET /auth/google/callback
/// Handle Google OAuth callback
#[utoipa::path(
    get,
    path = "/auth/google/callback",
    tag = "Auth",
    responses(
        (status = 200, description = "OAuth authentication result", body = AuthResponse)
    )
)]
pub async fn google_auth_callback(
    State(state): State<AppState>,
    jar: CookieJar,
    ValidatedQuery(params): ValidatedQuery<GoogleCallbackQuery>,
) -> (CookieJar, JsonResult<AuthResponse>) {
    info!("Google OAuth callback request");

    // Check for error parameter
    if let Some(error) = params.error {
        warn!("Google OAuth error: {}", error);
        return (
            jar,
            Err(AppError::Authentication(format!("OAuth error: {}", error))),
        );
    }

    let code = match params.code {
        Some(c) => c,
        None => {
            warn!("Missing authorization code");
            return (
                jar,
                Err(AppError::Validation(
                    "Missing authorization code".to_string(),
                )),
            );
        }
    };

    let state_param = match params.state {
        Some(s) => s,
        None => {
            warn!("Missing state parameter");
            return (
                jar,
                Err(AppError::Validation("Missing state parameter".to_string())),
            );
        }
    };

    // Verify state parameter and retrieve return_to URL
    let _oauth_state = if let Some(redis_pool) = state.config.redis_pool.as_ref() {
        match crate::infra::google_oauth::OAuthStateStore::retrieve_state(redis_pool, &state_param)
            .await
        {
            Ok(state) => state,
            Err(e) => {
                warn!("Invalid OAuth state: {}", e);
                return (
                    jar,
                    Err(AppError::Authentication(
                        "Invalid or expired state parameter".to_string(),
                    )),
                );
            }
        }
    } else {
        // For testing without Redis, just create a mock state
        crate::infra::google_oauth::OAuthState {
            state: state_param.clone(),
            return_to: None,
            created_at: Utc::now(),
        }
    };

    // Exchange authorization code for access token
    let token_response = match state
        .google_oauth_service
        .exchange_code_for_token(&code)
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to exchange code for token: {}", e);
            return (
                jar,
                Err(AppError::Authentication(
                    "Failed to authenticate with Google".to_string(),
                )),
            );
        }
    };

    // Get user info from Google
    let google_user_info = match state
        .google_oauth_service
        .get_user_info(&token_response.access_token)
        .await
    {
        Ok(info) => info,
        Err(e) => {
            error!("Failed to get user info: {}", e);
            return (
                jar,
                Err(AppError::Authentication(
                    "Failed to retrieve user information".to_string(),
                )),
            );
        }
    };

    // Find or create user
    let mut user = match User::find_by_google_id(&state.db, &google_user_info.id).await {
        Ok(existing_user) => {
            info!("Existing user found for Google ID: {}", google_user_info.id);
            existing_user
        }
        Err(_) => {
            // Check if user exists with this email
            match User::find_by_email(&state.db, &google_user_info.email).await {
                Ok(Some(mut existing_user)) => {
                    info!(
                        "Linking existing user to Google account: {}",
                        google_user_info.email
                    );
                    // Link the Google account to existing user
                    existing_user.google_id = Some(google_user_info.id.clone());
                    existing_user.provider = "google".to_string();
                    existing_user.avatar_url = google_user_info.picture.clone();
                    existing_user.provider_data = Some(
                        serde_json::to_value(&google_user_info)
                            .unwrap_or_else(|_| serde_json::json!({})),
                    );
                    existing_user.email_verified = google_user_info.verified_email;
                    match existing_user.update(&state.db).await {
                        Ok(_) => existing_user,
                        Err(e) => return (jar, Err(e)),
                    }
                }
                Ok(None) => {
                    info!(
                        "Creating new user from Google account: {}",
                        google_user_info.email
                    );
                    // Create new user
                    let username = google_user_info
                        .email
                        .split('@')
                        .next()
                        .unwrap_or("user")
                        .to_string();

                    match User::create_from_google(&state.db, google_user_info.clone(), username)
                        .await
                    {
                        Ok(u) => u,
                        Err(e) => return (jar, Err(e)),
                    }
                }
                Err(e) => return (jar, Err(e)),
            }
        }
    };

    // Update last login
    user.last_login = Some(Utc::now());
    user.last_provider_sync = Some(Utc::now());
    if let Err(e) = user.update(&state.db).await {
        return (jar, Err(e));
    }
    info!("User logged in successfully via Google: {}", user.email);

    // Generate JWT tokens
    let access_token = match jwt::generate_token(&user, &state.jwt_config) {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to generate access token: {:?}", e);
            return (
                jar,
                Err(AppError::Internal("Failed to generate token".to_string())),
            );
        }
    };
    let refresh_token = match jwt::generate_refresh_token(&user, &state.jwt_config) {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to generate refresh token: {:?}", e);
            return (
                jar,
                Err(AppError::Internal(
                    "Failed to generate refresh token".to_string(),
                )),
            );
        }
    };

    // Set httpOnly cookies
    let jar = crate::middleware::auth::set_auth_cookies(jar, &access_token, &refresh_token);

    let auth_response = AuthResponse {
        access_token: String::new(),  // Don't expose token in response body
        refresh_token: String::new(), // Don't expose token in response body
        user: UserResponse::from(user),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
    };

    (jar, data!(auth_response))
}

/// Update wallet address handler
/// POST /api/user/update-wallet
/// Update wallet address
// update_wallet_address function removed - use SIWE link-wallet instead
// SIWE provides cryptographic proof of wallet ownership

/// Get current authenticated user
/// GET /api/user/me
/// Get current user information
#[utoipa::path(
    get,
    path = "/api/user/me",
    tag = "Auth",
    responses(
        (status = 200, description = "Current user information", body = UserResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_current_user(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> JsonResult<UserResponse> {
    info!("Get current user request");

    let user_id = auth_user.user_id();

    // Get user from database
    let user = match User::find_by_id(&state.db, user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            warn!("User not found for id: {}", user_id);
            return Err(AppError::NotFound("User not found".to_string()));
        }
        Err(e) => {
            error!("Database error retrieving user: {:?}", e);
            return Err(e);
        }
    };

    info!("Current user retrieved successfully");
    data!(UserResponse::from(user))
}

/// Logout handler
/// POST /auth/logout
/// Clear authentication cookies
#[utoipa::path(
    post,
    path = "/auth/logout",
    tag = "Auth",
    responses(
        (status = 200, description = "Logout successful")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn logout_user(jar: CookieJar) -> (CookieJar, JsonResult<()>) {
    info!("User logout request");

    // Clear httpOnly cookies
    let jar = crate::middleware::auth::clear_auth_cookies(jar);

    info!("User logged out successfully");
    (jar, empty!())
}

/// Refresh token handler
/// POST /auth/refresh
/// Refresh authentication tokens using refresh token from cookies
#[utoipa::path(
    post,
    path = "/auth/refresh",
    tag = "Auth",
    responses(
        (status = 200, description = "Token refreshed", body = AuthResponse)
    )
)]
pub async fn refresh_token(
    State(state): State<AppState>,
    jar: CookieJar,
) -> (CookieJar, JsonResult<AuthResponse>) {
    info!("Token refresh request");

    // Extract refresh token from cookies
    let refresh_token = match crate::middleware::auth::extract_refresh_token(&jar) {
        Some(token) => token,
        None => {
            return (
                jar,
                Err(AppError::Authentication("Refresh token not found".into())),
            );
        }
    };

    // Validate refresh token and extract claims
    let claims = match crate::utils::jwt::verify_refresh_token(&refresh_token, &state.jwt_config) {
        Ok(c) => c,
        Err(e) => return (jar, Err(e)),
    };

    // Extract user ID from claims
    let user_id = match claims.sub.parse::<i32>() {
        Ok(id) => id,
        Err(_) => {
            return (
                jar,
                Err(AppError::Parsing("Invalid user ID in token".to_string())),
            );
        }
    };

    // Get user from database
    let user = match User::find_by_id(&state.db, user_id).await {
        Ok(Some(u)) => u,
        Ok(None) => return (jar, Err(AppError::Authentication("User not found".into()))),
        Err(e) => return (jar, Err(e)),
    };

    // Generate new tokens
    let access_token = match jwt::generate_token(&user, &state.jwt_config) {
        Ok(token) => token,
        Err(e) => return (jar, Err(e)),
    };
    let new_refresh_token = match jwt::generate_refresh_token(&user, &state.jwt_config) {
        Ok(token) => token,
        Err(e) => return (jar, Err(e)),
    };

    // Set new cookies
    let jar = crate::middleware::auth::set_auth_cookies(jar, &access_token, &new_refresh_token);

    let auth_response = AuthResponse {
        access_token: String::new(),  // Don't expose token in response body
        refresh_token: String::new(), // Don't expose token in response body
        user: UserResponse::from(user),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
    };

    info!("Token refreshed successfully");
    (jar, data!(auth_response))
}
