//! JWT utilities for authentication
//!
//! This module provides JWT token generation and verification utilities
//! for the DeLong Protocol authentication system.

use crate::models::user::User;
use crate::{AppError, AppResult};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// JWT claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // User ID
    pub email: String,
    pub username: String,
    pub iat: i64, // Issued at
    pub exp: i64, // Expiration time
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

/// JWT configuration
pub struct JwtConfig {
    pub secret: String,
    pub expiration_hours: i64,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: "default_secret_change_in_production".to_string(),
            expiration_hours: 24,
        }
    }
}

/// JWT Service for token management
pub struct JwtService {
    config: JwtConfig,
}

impl JwtService {
    /// Create a new JWT service
    pub fn new(config: Arc<crate::Config>) -> Self {
        Self {
            config: JwtConfig {
                secret: config.jwt_secret.clone(),
                expiration_hours: 24,
            },
        }
    }

    /// Extract token from Authorization header
    pub fn extract_token_from_header<'a>(&self, header: &'a str) -> AppResult<&'a str> {
        header.strip_prefix("Bearer ").ok_or_else(|| {
            AppError::Authentication("Invalid authorization header format".to_string())
        })
    }

    /// Validate access token
    pub fn validate_access_token(&self, token: &str) -> AppResult<Claims> {
        verify_token(token, &self.config)
    }

    /// Generate access token for a user
    pub fn generate_access_token(&self, user: &User) -> AppResult<String> {
        generate_token(user, &self.config)
    }
}

/// Generate JWT token for a user
pub fn generate_token(user: &User, config: &JwtConfig) -> AppResult<String> {
    let now = Utc::now();
    let exp = now + Duration::hours(config.expiration_hours);

    let claims = Claims {
        sub: user.id.to_string(),
        email: user.email.clone(),
        username: user.username.clone(),
        iat: now.timestamp(),
        exp: exp.timestamp(),
        roles: vec![user.role.clone()],
        permissions: vec![],
    };

    let header = Header::default();
    let encoding_key = EncodingKey::from_secret(config.secret.as_ref());

    encode(&header, &claims, &encoding_key)
        .map_err(|e| AppError::Config(format!("JWT encoding error: {}", e)))
}

/// Generate refresh token for a user with longer expiration
pub fn generate_refresh_token(user: &User, config: &JwtConfig) -> AppResult<String> {
    let now = Utc::now();
    // Refresh token expires in 30 days
    let exp = now + Duration::days(30);

    let claims = Claims {
        sub: user.id.to_string(),
        email: user.email.clone(),
        username: user.username.clone(),
        iat: now.timestamp(),
        exp: exp.timestamp(),
        roles: vec![user.role.clone()],
        permissions: vec![],
    };

    let header = Header::default();
    let encoding_key = EncodingKey::from_secret(config.secret.as_ref());

    encode(&header, &claims, &encoding_key)
        .map_err(|e| AppError::Config(format!("JWT encoding error: {}", e)))
}

/// Verify JWT token and extract claims
pub fn verify_token(token: &str, config: &JwtConfig) -> AppResult<Claims> {
    let decoding_key = DecodingKey::from_secret(config.secret.as_ref());
    let validation = Validation::default();

    decode::<Claims>(token, &decoding_key, &validation)
        .map(|data| data.claims)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                AppError::Authentication("Token expired".to_string())
            }
            jsonwebtoken::errors::ErrorKind::InvalidToken => {
                AppError::Authentication("Invalid token".to_string())
            }
            jsonwebtoken::errors::ErrorKind::InvalidSignature => {
                AppError::Authentication("Invalid signature".to_string())
            }
            _ => AppError::Authentication("Token verification failed".to_string()),
        })
}

/// Verify refresh token and extract claims
/// Refresh tokens have relaxed validation rules compared to access tokens
pub fn verify_refresh_token(token: &str, config: &JwtConfig) -> AppResult<Claims> {
    let decoding_key = DecodingKey::from_secret(config.secret.as_ref());
    let mut validation = Validation::default();
    // Refresh tokens can have longer expiration, but we still validate it
    validation.leeway = 60; // Allow 60 seconds of leeway for clock skew

    decode::<Claims>(token, &decoding_key, &validation)
        .map(|data| data.claims)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                AppError::Authentication("Refresh token expired".to_string())
            }
            jsonwebtoken::errors::ErrorKind::InvalidToken => {
                AppError::Authentication("Invalid refresh token".to_string())
            }
            jsonwebtoken::errors::ErrorKind::InvalidSignature => {
                AppError::Authentication("Invalid refresh token signature".to_string())
            }
            _ => AppError::Authentication("Refresh token verification failed".to_string()),
        })
}

/// Extract user ID from JWT token
pub fn extract_user_id(token: &str, config: &JwtConfig) -> AppResult<i32> {
    let claims = verify_token(token, config)?;
    claims
        .sub
        .parse::<i32>()
        .map_err(|_| AppError::Parsing("Invalid user ID in token".to_string()))
}

/// Extract user email from JWT token
pub fn extract_user_email(token: &str, config: &JwtConfig) -> AppResult<String> {
    let claims = verify_token(token, config)?;
    Ok(claims.email)
}

/// Extract user role from JWT token
pub fn extract_user_role(token: &str, config: &JwtConfig) -> AppResult<String> {
    let claims = verify_token(token, config)?;
    Ok(claims
        .roles
        .first()
        .cloned()
        .unwrap_or_else(|| String::new()))
}

/// Check if token is expired
pub fn is_token_expired(token: &str, config: &JwtConfig) -> bool {
    match verify_token(token, config) {
        Ok(_) => false,
        Err(AppError::Authentication(msg)) if msg.contains("expired") => true,
        Err(_) => true, // Treat invalid tokens as expired
    }
}

/// Refresh JWT token using a refresh token
pub fn refresh_token(refresh_token_str: &str, config: &JwtConfig) -> AppResult<String> {
    // Verify the refresh token (must be valid and not expired)
    let claims = verify_refresh_token(refresh_token_str, config)?;

    // Generate new access token with same user info but new expiration
    let now = Utc::now();
    let exp = now + Duration::hours(config.expiration_hours);

    let new_claims = Claims {
        sub: claims.sub,
        email: claims.email,
        username: claims.username,
        iat: now.timestamp(),
        exp: exp.timestamp(),
        roles: claims.roles,
        permissions: claims.permissions,
    };

    let header = Header::default();
    let encoding_key = EncodingKey::from_secret(config.secret.as_ref());

    encode(&header, &new_claims, &encoding_key)
        .map_err(|e| AppError::Config(format!("JWT encoding error: {}", e)))
}

/// Create JWT config from environment or default values
pub fn create_jwt_config() -> JwtConfig {
    JwtConfig {
        secret: std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "default_secret_change_in_production".to_string()),
        expiration_hours: std::env::var("JWT_EXPIRATION_HOURS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(24),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_user() -> User {
        User {
            id: 1,
            email: "test@example.com".to_string(),
            username: "testuser".to_string(),
            password_hash: Some("hash".to_string()),
            role: "scientist".to_string(),
            status: "active".to_string(),
            wallet_address: None,
            google_id: None,
            avatar_url: None,
            provider: "email".to_string(),
            provider_data: serde_json::json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login: None,
            last_provider_sync: None,
            email_verified: Some(true),
            two_factor_enabled: Some(false),
            profile_data: serde_json::json!({}),
        }
    }

    #[test]
    fn test_generate_and_verify_token() {
        let config = JwtConfig::default();
        let user = create_test_user();

        let token = generate_token(&user, &config).unwrap();
        let claims = verify_token(&token, &config).unwrap();

        assert_eq!(claims.sub, "1");
        assert_eq!(claims.email, "test@example.com");
        assert_eq!(
            claims
                .roles
                .first()
                .cloned()
                .unwrap_or_else(|| String::new()),
            "scientist"
        );
    }

    #[test]
    fn test_extract_user_info() {
        let config = JwtConfig::default();
        let user = create_test_user();

        let token = generate_token(&user, &config).unwrap();

        assert_eq!(extract_user_id(&token, &config).unwrap(), 1);
        assert_eq!(
            extract_user_email(&token, &config).unwrap(),
            "test@example.com"
        );
        assert_eq!(extract_user_role(&token, &config).unwrap(), "scientist");
    }

    #[test]
    fn test_refresh_token() {
        let config = JwtConfig::default();
        let user = create_test_user();

        let original_token = generate_token(&user, &config).unwrap();
        let refreshed_token = refresh_token(&original_token, &config).unwrap();

        // Both tokens should be valid and contain same user info
        let original_claims = verify_token(&original_token, &config).unwrap();
        let refreshed_claims = verify_token(&refreshed_token, &config).unwrap();

        assert_eq!(original_claims.sub, refreshed_claims.sub);
        assert_eq!(original_claims.email, refreshed_claims.email);
        assert_eq!(original_claims.roles, refreshed_claims.roles);

        // Refreshed token should have later expiration
        println!(
            "Original exp: {}, Refreshed exp: {}",
            original_claims.exp, refreshed_claims.exp
        );
        // Since we're generating both tokens rapidly, they might have the same timestamp
        // Let's just check they're not less
        assert!(refreshed_claims.exp >= original_claims.exp);
    }

    #[test]
    fn test_invalid_token() {
        let config = JwtConfig::default();

        assert!(verify_token("invalid.token.here", &config).is_err());
        assert!(extract_user_id("invalid.token.here", &config).is_err());
    }

    #[test]
    fn test_is_token_expired() {
        let config = JwtConfig::default();
        let user = create_test_user();

        let token = generate_token(&user, &config).unwrap();

        // Fresh token should not be expired
        assert!(!is_token_expired(&token, &config));

        // Invalid token should be treated as expired
        assert!(is_token_expired("invalid.token.here", &config));
    }
}
