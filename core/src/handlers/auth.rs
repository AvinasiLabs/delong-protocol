//! Authentication handlers
//!
//! This module contains HTTP handlers for user authentication,
//! including registration, login, verification code management,
//! and Google OAuth integration.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use common::{ApiError, ApiResponse, ResponseCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info, warn};
use validator::Validate;

use crate::{
    AppState,
    models::{
        auth::{AuthResponse, SendVerificationCodeRequest, UpdateWalletRequest},
        user::{CreateUserRequest, LoginRequest, RegisterRequest, User, UserResponse},
    },
    utils::jwt::{self},
};

/// Google auth URL query parameters
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
pub struct GoogleCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// Google callback request body
#[derive(Debug, Deserialize)]
pub struct GoogleCallbackRequest {
    pub code: String,
    pub state: String,
}

/// Refresh token request
#[derive(Debug, Deserialize, Validate)]
pub struct RefreshTokenRequest {
    #[validate(length(min = 1))]
    pub refresh_token: String,
}

/// User login handler
/// POST /auth/login
pub async fn login_user(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<ApiResponse<AuthResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("User login request for email: {}", payload.email);

    // Validate input
    if let Err(validation_errors) = payload.validate() {
        warn!("Login validation failed: {:?}", validation_errors);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    if payload.email.is_empty() || payload.password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // Find user by email
    let user = match User::find_by_email(&state.db, &payload.email).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            warn!("Login failed: User not found for email: {}", payload.email);
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::error(ResponseCode::NotFound)),
            ));
        }
        Err(e) => {
            error!("Database error during login: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            ));
        }
    };

    // Verify password
    if !user.verify_password(&payload.password) {
        warn!(
            "Login failed: Invalid password for email: {}",
            payload.email
        );
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::error(ResponseCode::Unauthorized)),
        ));
    }

    // Check if user is active
    if !user.is_active() {
        warn!(
            "Login failed: Account not active for email: {}",
            payload.email
        );
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiResponse::error(ResponseCode::Forbidden)),
        ));
    }

    // Generate JWT token
    let access_token = match jwt::generate_token(&user, &state.jwt_config) {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to generate JWT token: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            ));
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
    Ok(Json(ApiResponse::success(auth_response)))
}

/// User registration handler
/// POST /auth/register
pub async fn register_user(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<ApiResponse<AuthResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("User registration request for email: {}", payload.email);

    // Validate input
    if let Err(validation_errors) = payload.validate() {
        warn!("Registration validation failed: {:?}", validation_errors);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // Validate required fields
    if payload.username.is_empty() || payload.email.is_empty() || payload.password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    if payload.verification_code.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // Validate email format
    let email_regex = regex::Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap();
    if !email_regex.is_match(&payload.email) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // Validate password strength (6-10 characters as per frontend logic)
    if payload.password.len() < 6 || payload.password.len() > 10 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // Validate username format
    let username_regex = regex::Regex::new(r"^[a-zA-Z0-9_]{3,20}$").unwrap();
    if !username_regex.is_match(&payload.username) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
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
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(ResponseCode::BadRequest)),
            ));
        }
        Err(e) => {
            error!("Error verifying verification code: {:?}", e);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(ResponseCode::BadRequest)),
            ));
        }
    }

    // Check if user already exists
    if let Ok(Some(_)) = User::find_by_email(&state.db, &payload.email).await {
        warn!(
            "Registration failed: User already exists for email: {}",
            payload.email
        );
        return Err((
            StatusCode::CONFLICT,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // Create user
    let create_request = CreateUserRequest {
        username: payload.username,
        email: payload.email.clone(),
        password: Some(payload.password),
        role: Some("scientist".to_string()), // Default role
        wallet_address: None,
    };

    let user = match User::create(&state.db, create_request).await {
        Ok(user) => user,
        Err(e) => {
            error!("Failed to create user: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            ));
        }
    };

    // Generate JWT token
    let access_token = match jwt::generate_token(&user, &state.jwt_config) {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to generate JWT token: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            ));
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
    Ok(Json(ApiResponse::success(auth_response)))
}

/// Send verification code handler
/// POST /auth/send-code
pub async fn send_verification_code(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SendVerificationCodeRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!(
        "Send verification code request for email: {}",
        payload.email
    );

    // Validate input
    if let Err(validation_errors) = payload.validate() {
        warn!("Send code validation failed: {:?}", validation_errors);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    if payload.email.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // Validate email format
    let email_regex = regex::Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap();
    if !email_regex.is_match(&payload.email) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
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
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(ApiResponse::error(ResponseCode::TooManyRequests)),
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
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(ResponseCode::InternalServerError)),
        ));
    }

    // In development mode, include the code in response
    if state.config.development_mode {
        info!(
            "Development mode: Verification code for {}: {}",
            payload.email, verification_code
        );
        return Ok(Json(ApiResponse::success(json!({
            "message": "Verification code sent successfully",
            "dev_mode": true,
            "dev_code": verification_code
        }))));
    }

    // TODO: Send email via email service
    // For now, just log the code
    info!(
        "Verification code for {}: {}",
        payload.email, verification_code
    );

    Ok(Json(ApiResponse::success(json!({
        "message": "Verification code sent successfully"
    }))))
}

/// Get Google OAuth URL handler
/// GET /auth/google
pub async fn google_auth_url(
    State(_state): State<Arc<AppState>>,
    Query(_params): Query<GoogleAuthQuery>,
) -> Result<Json<ApiResponse<GoogleAuthUrlResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
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

    Ok(Json(ApiResponse::success(mock_response)))
}

/// Google OAuth callback handler
/// GET /auth/google/callback
pub async fn google_auth_callback(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GoogleCallbackQuery>,
) -> Result<Json<ApiResponse<AuthResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Google OAuth callback request");

    // Check for error parameter
    if let Some(error) = params.error {
        warn!("Google OAuth error: {}", error);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    let _code = params.code.ok_or_else(|| {
        warn!("Missing authorization code");
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        )
    })?;

    let _state_param = params.state.ok_or_else(|| {
        warn!("Missing state parameter");
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        )
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

    Ok(Json(ApiResponse::success(auth_response)))
}

/// Update wallet address handler
/// POST /auth/update-wallet
pub async fn update_wallet_address(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateWalletRequest>,
) -> Result<Json<ApiResponse<UserResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Update wallet address request");

    // Validate input
    if let Err(validation_errors) = payload.validate() {
        warn!("Update wallet validation failed: {:?}", validation_errors);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // Validate wallet address format
    let wallet_regex = regex::Regex::new(r"^0x[a-fA-F0-9]{40}$").unwrap();
    if !wallet_regex.is_match(&payload.wallet_address) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // TODO: Extract user ID from JWT token in authentication middleware
    // For now, we'll use a mock user ID
    let user_id = 1;

    // Update wallet address
    if let Err(e) =
        User::update_wallet_address(&state.db, user_id, Some(payload.wallet_address)).await
    {
        error!("Failed to update wallet address: {:?}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(ResponseCode::InternalServerError)),
        ));
    }

    // Get updated user
    let user = match User::find_by_id(&state.db, user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            warn!("User not found after wallet update");
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::error(ResponseCode::NotFound)),
            ));
        }
        Err(e) => {
            error!("Database error retrieving updated user: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            ));
        }
    };

    info!("Wallet address updated successfully");
    Ok(Json(ApiResponse::success(UserResponse::from(user))))
}

/// Get current user handler
/// GET /auth/me
pub async fn get_current_user(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<UserResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Get current user request");

    // TODO: Extract user ID from JWT token in authentication middleware
    let user_id = 1;

    match User::find_by_id(&state.db, user_id).await {
        Ok(Some(user)) => {
            info!("Current user retrieved successfully");
            Ok(Json(ApiResponse::success(UserResponse::from(user))))
        }
        Ok(None) => {
            warn!("Current user not found");
            Err((
                StatusCode::NOT_FOUND,
                Json(ApiResponse::error(ResponseCode::NotFound)),
            ))
        }
        Err(e) => {
            error!("Database error retrieving current user: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            ))
        }
    }
}

/// Refresh token handler
/// POST /auth/refresh
pub async fn refresh_token(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RefreshTokenRequest>,
) -> Result<Json<ApiResponse<AuthResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Refresh token request");

    // Validate input
    if let Err(validation_errors) = payload.validate() {
        warn!("Refresh token validation failed: {:?}", validation_errors);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ResponseCode::BadRequest)),
        ));
    }

    // Refresh the token
    let new_token = match jwt::refresh_token(&payload.refresh_token, &state.jwt_config) {
        Ok(token) => token,
        Err(ApiError::InvalidToken) => {
            warn!("Invalid refresh token");
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::error(ResponseCode::Unauthorized)),
            ));
        }
        Err(e) => {
            error!("Failed to refresh token: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            ));
        }
    };

    // Extract user info from the new token to create response
    let claims = match jwt::verify_token(&new_token, &state.jwt_config) {
        Ok(claims) => claims,
        Err(e) => {
            error!("Failed to verify refreshed token: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ResponseCode::InternalServerError)),
            ));
        }
    };

    let auth_response = AuthResponse {
        access_token: new_token.clone(),
        refresh_token: new_token,
        user: UserResponse {
            id: claims.sub.parse().unwrap_or(0),
            email: claims.email,
            username: "".to_string(), // We don't have username in JWT claims
            role: claims.role,
            status: "active".to_string(),
            wallet_address: None,
            created_at: chrono::Utc::now(),
            last_login: None,
            google_id: None,
            avatar_url: None,
            provider: "jwt".to_string(),
        },
        expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
    };

    info!("Token refreshed successfully");
    Ok(Json(ApiResponse::success(auth_response)))
}

/// Logout user handler
/// POST /auth/logout
pub async fn logout_user(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("User logout request");

    // TODO: Implement token blacklisting or other logout logic
    // For stateless JWT tokens, logout is typically handled client-side
    // by removing the token from storage

    Ok(Json(ApiResponse::success(json!({
        "message": "Logged out successfully"
    }))))
}

/// Health check for auth endpoints
/// GET /auth/health
pub async fn health_check() -> Json<ApiResponse<serde_json::Value>> {
    Json(ApiResponse::success(json!({
        "status": "healthy",
        "service": "auth",
        "timestamp": chrono::Utc::now()
    })))
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
