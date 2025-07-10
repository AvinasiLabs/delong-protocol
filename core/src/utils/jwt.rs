//! JWT utilities for authentication
//!
//! This module provides JWT token generation and verification utilities
//! for the DeLong Protocol authentication system.

use crate::models::user::User;
use chrono::{Duration, Utc};
use common::{ApiError, ApiResult};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

/// JWT Claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,   // Subject (user ID)
    pub email: String, // User email
    pub role: String,  // User role
    pub exp: usize,    // Expiration time
    pub iat: usize,    // Issued at
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

/// Generate JWT token for a user
pub fn generate_token(user: &User, config: &JwtConfig) -> ApiResult<String> {
    let now = Utc::now();
    let exp = now + Duration::hours(config.expiration_hours);

    let claims = Claims {
        sub: user.id.to_string(),
        email: user.email.clone(),
        role: user.role.clone(),
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    let header = Header::default();
    let encoding_key = EncodingKey::from_secret(config.secret.as_ref());

    encode(&header, &claims, &encoding_key)
        .map_err(|e| ApiError::ConfigurationError(format!("JWT encoding error: {}", e)))
}

/// Verify JWT token and extract claims
pub fn verify_token(token: &str, config: &JwtConfig) -> ApiResult<Claims> {
    let decoding_key = DecodingKey::from_secret(config.secret.as_ref());
    let validation = Validation::default();

    decode::<Claims>(token, &decoding_key, &validation)
        .map(|data| data.claims)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => ApiError::TokenExpired,
            jsonwebtoken::errors::ErrorKind::InvalidToken => ApiError::InvalidToken,
            jsonwebtoken::errors::ErrorKind::InvalidSignature => ApiError::InvalidToken,
            _ => ApiError::InvalidToken,
        })
}

/// Extract user ID from JWT token
pub fn extract_user_id(token: &str, config: &JwtConfig) -> ApiResult<i32> {
    let claims = verify_token(token, config)?;
    claims
        .sub
        .parse::<i32>()
        .map_err(|_| ApiError::InvalidToken)
}

/// Extract user email from JWT token
pub fn extract_user_email(token: &str, config: &JwtConfig) -> ApiResult<String> {
    let claims = verify_token(token, config)?;
    Ok(claims.email)
}

/// Extract user role from JWT token
pub fn extract_user_role(token: &str, config: &JwtConfig) -> ApiResult<String> {
    let claims = verify_token(token, config)?;
    Ok(claims.role)
}

/// Check if token is expired
pub fn is_token_expired(token: &str, config: &JwtConfig) -> bool {
    match verify_token(token, config) {
        Ok(_) => false,
        Err(ApiError::TokenExpired) => true,
        Err(_) => true, // Treat invalid tokens as expired
    }
}

/// Refresh JWT token
pub fn refresh_token(old_token: &str, config: &JwtConfig) -> ApiResult<String> {
    // Verify the old token (allow expired tokens for refresh)
    let decoding_key = DecodingKey::from_secret(config.secret.as_ref());
    let mut validation = Validation::default();
    validation.validate_exp = false; // Don't validate expiration for refresh

    let old_claims = decode::<Claims>(old_token, &decoding_key, &validation)
        .map(|data| data.claims)
        .map_err(|_| ApiError::InvalidToken)?;

    // Generate new token with same user info but new expiration
    let now = Utc::now();
    let exp = now + Duration::hours(config.expiration_hours);

    let new_claims = Claims {
        sub: old_claims.sub,
        email: old_claims.email,
        role: old_claims.role,
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    let header = Header::default();
    let encoding_key = EncodingKey::from_secret(config.secret.as_ref());

    encode(&header, &new_claims, &encoding_key)
        .map_err(|e| ApiError::ConfigurationError(format!("JWT encoding error: {}", e)))
}

/// Create JWT config from an AppConfig
pub fn create_jwt_config(config: &crate::config::AppConfig) -> JwtConfig {
    JwtConfig {
        secret: config.jwt_secret.clone(),
        expiration_hours: config.jwt_expiration,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
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

    fn create_test_jwt_config() -> JwtConfig {
        let config = AppConfig {
            jwt_secret: "test_secret".to_string(),
            jwt_expiration: 1,
            ..Default::default()
        };
        create_jwt_config(&config)
    }

    #[test]
    fn test_generate_and_verify_token() {
        let config = create_test_jwt_config();
        let user = create_test_user();

        let token = generate_token(&user, &config).unwrap();
        let claims = verify_token(&token, &config).unwrap();

        assert_eq!(claims.sub, "1");
        assert_eq!(claims.email, "test@example.com");
        assert_eq!(claims.role, "scientist");
    }

    #[test]
    fn test_extract_user_info() {
        let config = create_test_jwt_config();
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
        let config = create_test_jwt_config();
        let user = create_test_user();

        let original_token = generate_token(&user, &config).unwrap();
        let refreshed_token = refresh_token(&original_token, &config).unwrap();

        // Both tokens should be valid and contain same user info
        let original_claims = verify_token(&original_token, &config).unwrap();
        let refreshed_claims = verify_token(&refreshed_token, &config).unwrap();

        assert_eq!(original_claims.sub, refreshed_claims.sub);
        assert_eq!(original_claims.email, refreshed_claims.email);
        assert_eq!(original_claims.role, refreshed_claims.role);

        // Refreshed token should have later expiration
        assert!(refreshed_claims.exp > original_claims.exp);
    }

    #[test]
    fn test_invalid_token() {
        let config = create_test_jwt_config();

        assert!(verify_token("invalid.token.here", &config).is_err());
        assert!(extract_user_id("invalid.token.here", &config).is_err());
    }

    #[test]
    fn test_is_token_expired() {
        let config = create_test_jwt_config();
        let user = create_test_user();

        let token = generate_token(&user, &config).unwrap();

        // Fresh token should not be expired
        assert!(!is_token_expired(&token, &config));

        // Invalid token should be treated as expired
        assert!(is_token_expired("invalid.token.here", &config));
    }
}
