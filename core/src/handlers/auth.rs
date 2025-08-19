//! Authentication handlers
//!
//! This module contains HTTP handlers for user authentication,
//! including registration, login, verification code management,
//! and Google OAuth integration.

use avinapi::prelude::{AppError, JsonResult, ValidatedJson, data};
use axum::extract::{Query, State};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

use regex::Regex;
use std::sync::LazyLock;
use tracing::{error, info, warn};
use validator::Validate;

use crate::{
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
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    #[validate(length(min = 6, message = "Password must be at least 6 characters"))]
    pub password: String,
}

/// User registration request
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
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
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SendVerificationCodeRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    pub verification_type: VerificationType,
    pub language: Option<String>,
}

/// Update wallet address request
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateWalletRequest {
    #[validate(
        length(
            min = 42,
            max = 42,
            message = "Wallet address must be exactly 42 characters"
        ),
        regex(
            path = "crate::handlers::auth::WALLET_ADDRESS_REGEX",
            message = "Invalid wallet address format (must be 0x followed by 40 hex characters)"
        )
    )]
    pub wallet_address: String,
}

/// Google OAuth login request
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct GoogleLoginRequest {
    pub access_token: String,
    pub id_token: Option<String>,
}

// ===== Response Structures =====

/// User response model for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: UserResponse,
    pub expires_at: DateTime<Utc>,
}

/// Google auth URL query parameters
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct GoogleAuthQuery {
    #[serde(rename = "returnTo")]
    pub return_to: Option<String>,
}

/// Google auth URL response
#[derive(Debug, Serialize)]
pub struct GoogleAuthUrlResponse {
    #[serde(rename = "authUrl")]
    pub auth_url: String,
    pub state: String,
}

/// Google callback query parameters
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
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
pub async fn login_user(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<LoginRequest>,
) -> JsonResult<AuthResponse> {
    info!("User login request for email: {}", payload.email);

    // Find user by email
    let user = match User::find_by_email(&state.db, &payload.email).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            warn!("Login failed: User not found for email: {}", payload.email);
            return Err(AppError::Authentication("Invalid credentials".to_string()));
        }
        Err(e) => {
            error!("Database error during login: {:?}", e);
            return Err(e);
        }
    };

    // Verify password
    if !user.verify_password(&payload.password) {
        warn!(
            "Login failed: Invalid password for email: {}",
            payload.email
        );
        return Err(AppError::Authentication("Invalid credentials".to_string()));
    }

    // Check if user is active
    if !user.is_active() {
        warn!(
            "Login failed: Account not active for email: {}",
            payload.email
        );
        return Err(AppError::Authentication(
            "Account is not active".to_string(),
        ));
    }

    // Generate JWT token
    let access_token = match jwt::generate_token(&user, &state.jwt_config) {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to generate JWT token: {:?}", e);
            return Err(AppError::Internal("Failed to generate token".to_string()));
        }
    };

    // Update last login
    if let Err(e) = User::update_last_login(&state.db, user.id).await {
        error!("Failed to update last login: {:?}", e);
        // Don't fail the login for this
    }

    let auth_response = AuthResponse {
        access_token: access_token.clone(),
        refresh_token: access_token, // For now, using same token
        user: UserResponse::from(user),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
    };

    info!("User logged in successfully: {}", payload.email);
    data!(auth_response)
}

/// User registration handler
/// Register a new user
/// POST /auth/register
pub async fn register_user(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<RegisterRequest>,
) -> JsonResult<AuthResponse> {
    info!("User registration request for email: {}", payload.email);

    if payload.verification_code.is_none() {
        return Err(AppError::Validation(
            "Verification code is required".to_string(),
        ));
    }

    // Verify verification code
    let verification_code = payload.verification_code.unwrap_or_default();
    match state
        .verification_store
        .verify_code(&payload.email, &verification_code)
        .await
    {
        Ok(true) => {
            info!("Verification code verified for email: {}", payload.email);
        }
        Ok(false) => {
            warn!("Invalid verification code for email: {}", payload.email);
            return Err(AppError::Validation(
                "Invalid or expired verification code".to_string(),
            ));
        }
        Err(e) => {
            error!("Error verifying verification code: {:?}", e);
            return Err(e);
        }
    }

    // Check if user already exists
    if let Ok(Some(_)) = User::find_by_email(&state.db, &payload.email).await {
        warn!(
            "Registration failed: User already exists for email: {}",
            payload.email
        );
        return Err(AppError::Conflict("Email already registered".to_string()));
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
            return Err(e);
        }
    };

    // Generate JWT token
    let access_token = match jwt::generate_token(&user, &state.jwt_config) {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to generate JWT token: {:?}", e);
            return Err(AppError::Internal("Failed to generate token".to_string()));
        }
    };

    // Remove verification code
    state.verification_store.remove_code(&payload.email).await;

    let auth_response = AuthResponse {
        access_token: access_token.clone(),
        refresh_token: access_token, // For now, using same token
        user: UserResponse::from(user),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
    };

    info!("User registered successfully: {}", payload.email);
    data!(auth_response)
}

/// Send verification code handler
/// Send verification code
/// POST /auth/send-code
pub async fn send_verification_code(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<SendVerificationCodeRequest>,
) -> JsonResult<serde_json::Value> {
    info!(
        "Send verification code request for email: {}",
        payload.email
    );

    if payload.email.is_empty() {
        return Err(AppError::Validation("Email is required".to_string()));
    }

    // Validate email format
    let email_regex = regex::Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap();
    if !email_regex.is_match(&payload.email) {
        return Err(AppError::Validation("Invalid email format".to_string()));
    }

    // Check if code already sent recently
    if state
        .verification_store
        .get_code(&payload.email)
        .await
        .is_some()
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
    let verification_code = generate_verification_code();
    if let Err(e) = state
        .verification_store
        .store_code(
            &payload.email,
            &verification_code,
            payload.language.as_deref(),
        )
        .await
    {
        error!("Failed to store verification code: {:?}", e);
        return Err(e);
    }

    // In development mode, include the code in response
    if state.config.development_mode {
        info!(
            "Development mode: Verification code for {}: {}",
            payload.email, verification_code
        );
        return data!(json!({
            "message": "Verification code sent successfully",
            "dev_mode": true,
            "dev_code": verification_code
        }));
    }

    // TODO: Send email via email service
    // For now, just log the code
    info!(
        "Verification code for {}: {}",
        payload.email, verification_code
    );

    data!(json!({
        "message": "Verification code sent successfully"
    }))
}

/// Get Google OAuth URL handler
/// GET /auth/google
pub async fn google_auth_url(
    State(_state): State<AppState>,
    Query(_query): Query<GoogleAuthQuery>,
) -> JsonResult<GoogleAuthUrlResponse> {
    info!("Google OAuth URL request");

    // TODO: Implement Google OAuth URL generation
    // This would involve generating a state parameter and constructing the Google OAuth URL

    let mock_response = GoogleAuthUrlResponse {
        auth_url: format!(
            "https://accounts.google.com/oauth/authorize?client_id=mock&redirect_uri=mock&response_type=code&scope=email%20profile&state={}",
            generate_state_parameter()
        ),
        state: generate_state_parameter(),
    };

    data!(mock_response)
}

/// Google OAuth callback handler
/// GET /auth/google/callback
pub async fn google_auth_callback(
    State(state): State<AppState>,
    Query(params): Query<GoogleCallbackQuery>,
) -> JsonResult<AuthResponse> {
    info!("Google OAuth callback request");

    // Check for error parameter
    if let Some(error) = params.error {
        warn!("Google OAuth error: {}", error);
        return Err(AppError::Authentication(format!("OAuth error: {}", error)));
    }

    let _code = params.code.ok_or_else(|| {
        warn!("Missing authorization code");
        AppError::Validation("Missing authorization code".to_string())
    })?;

    let _state_param = params.state.ok_or_else(|| {
        warn!("Missing state parameter");
        AppError::Validation("Missing state parameter".to_string())
    })?;

    // TODO: Implement Google OAuth callback
    // 1. Verify state parameter
    // 2. Exchange code for access token
    // 3. Get user info from Google
    // 4. Find or create user
    // 5. Generate JWT token

    // Mock response for now
    let mock_user = User {
        id: 1,
        email: "test@example.com".to_string(),
        username: "testuser".to_string(),
        password_hash: None,
        role: "scientist".to_string(),
        status: "active".to_string(),
        wallet_address: None,
        google_id: Some("mock_google_id".to_string()),
        avatar_url: Some("https://example.com/avatar.jpg".to_string()),
        provider: "google".to_string(),
        provider_data: serde_json::json!({}),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        last_login: None,
        last_provider_sync: None,
        email_verified: Some(true),
        two_factor_enabled: Some(false),
        profile_data: serde_json::json!({}),
    };

    let access_token = jwt::generate_token(&mock_user, &state.jwt_config).unwrap();

    let auth_response = AuthResponse {
        access_token: access_token.clone(),
        refresh_token: access_token,
        user: UserResponse::from(mock_user),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
    };

    data!(auth_response)
}

/// Update wallet address handler
/// POST /auth/update-wallet
/// Update wallet address
pub async fn update_wallet_address(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ValidatedJson(payload): ValidatedJson<UpdateWalletRequest>,
) -> JsonResult<UserResponse> {
    info!("Update wallet address request");

    // Validate wallet address format
    let wallet_regex = regex::Regex::new(r"^0x[a-fA-F0-9]{40}$").unwrap();
    if !wallet_regex.is_match(&payload.wallet_address) {
        return Err(AppError::Validation(
            "Invalid wallet address format".to_string(),
        ));
    }

    let user_id = auth_user
        .user_id()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Update wallet address
    if let Err(e) =
        User::update_wallet_address(&state.db, user_id, Some(payload.wallet_address)).await
    {
        error!("Failed to update wallet address: {:?}", e);
        return Err(e);
    }

    // Get updated user
    let user = match User::find_by_id(&state.db, user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            warn!("User not found after wallet update");
            return Err(AppError::NotFound("User not found".to_string()));
        }
        Err(e) => {
            error!("Database error retrieving updated user: {:?}", e);
            return Err(e);
        }
    };

    info!("Wallet address updated successfully");
    data!(UserResponse::from(user))
}

/// Generate a 6-digit verification code
fn generate_verification_code() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(100000..999999))
}

/// Generate a random state parameter for OAuth
fn generate_state_parameter() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")
}
