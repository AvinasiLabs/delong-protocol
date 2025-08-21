//! API Key authentication middleware
//!
//! This middleware handles API key authentication for programmatic access
//! to the Core service, which then forwards authenticated requests to Secure.

use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use sqlx::{PgPool, Row};
use tracing::{debug, warn};

use crate::{
    AppState,
    models::{
        api_key::{ApiKey, RateLimitTier},
        user::User,
    },
};
use avinapi::transport::response::{ResponseCode, fail};

/// Extract API key from request headers
fn extract_api_key(req: &Request<Body>) -> Option<String> {
    // Check X-API-Key header (standard)
    if let Some(api_key) = req.headers().get("x-api-key") {
        if let Ok(key) = api_key.to_str() {
            return Some(key.to_string());
        }
    }

    // Check Authorization header with ApiKey scheme
    if let Some(auth_header) = req.headers().get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("ApiKey ") {
                return Some(auth_str[7..].to_string());
            }
        }
    }

    None
}

/// API Key authentication middleware
///
/// This middleware validates API keys for programmatic access.
/// It should be applied to routes that support both JWT and API key authentication.
pub async fn api_key_auth_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract API key from headers
    let api_key = match extract_api_key(&req) {
        Some(key) => key,
        None => {
            // No API key found, maybe JWT auth will handle it
            debug!("No API key found in request");
            return Ok(next.run(req).await);
        }
    };

    // Validate and load API key from database
    match validate_api_key(&state.db, &api_key).await {
        Ok(Some((api_key_record, user))) => {
            debug!(
                "API key authenticated for user: {} (key: {})",
                user.email, api_key_record.name
            );

            // Store authenticated user and auth method in extensions
            req.extensions_mut().insert(user);
            req.extensions_mut().insert("api_key".to_string()); // Auth method
            req.extensions_mut().insert(api_key_record);

            Ok(next.run(req).await)
        }
        Ok(None) => {
            warn!("Invalid API key attempted: {}", mask_api_key(&api_key));
            Ok(fail::<()>(ResponseCode::AuthenticationError, "Invalid API key").into_response())
        }
        Err(e) => {
            warn!("Error validating API key: {}", e);
            Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                fail::<()>(ResponseCode::AuthenticationError, "Authentication error"),
            )
                .into_response())
        }
    }
}

/// Combined authentication middleware that tries both JWT and API key
///
/// This middleware first checks for API key, then falls back to JWT.
/// This allows endpoints to support both authentication methods.
pub async fn combined_auth_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // First, check for API key
    if let Some(api_key) = extract_api_key(&req) {
        // Validate API key
        match validate_api_key(&state.db, &api_key).await {
            Ok(Some((api_key_record, user))) => {
                debug!(
                    "API key authenticated for user: {} (key: {})",
                    user.email, api_key_record.name
                );

                // Store authenticated user and auth method
                req.extensions_mut().insert(user);
                req.extensions_mut().insert("api_key".to_string());
                req.extensions_mut().insert(api_key_record);

                return Ok(next.run(req).await);
            }
            Ok(None) => {
                warn!("Invalid API key attempted: {}", mask_api_key(&api_key));
                return Ok(
                    fail::<()>(ResponseCode::AuthenticationError, "Invalid API key")
                        .into_response(),
                );
            }
            Err(e) => {
                warn!("Error validating API key: {}", e);
                return Ok((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    fail::<()>(ResponseCode::AuthenticationError, "Authentication error"),
                )
                    .into_response());
            }
        }
    }

    // If no API key, check for JWT
    if let Some(auth_header) = req.headers().get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                // JWT auth will be handled by jwt_auth_middleware
                // Just mark that we're using JWT
                req.extensions_mut().insert("jwt".to_string());
                return Ok(next.run(req).await);
            }
        }
    }

    // No authentication provided
    Ok(fail::<()>(ResponseCode::AuthenticationError, "Authentication required").into_response())
}

/// Validate API key and return the key record and associated user
async fn validate_api_key(
    db: &PgPool,
    api_key: &str,
) -> Result<Option<(ApiKey, User)>, sqlx::Error> {
    // Query for the API key directly
    // Query for the API key with manual type mapping
    let row = sqlx::query!(
        r#"
        SELECT
            id,
            api_key,
            name,
            user_id,
            permissions,
            rate_limit_tier as "rate_limit_tier: RateLimitTier",
            is_active,
            expires_at,
            last_used_at,
            created_at,
            updated_at
        FROM api_keys
        WHERE api_key = $1
        AND is_active = true
        AND (expires_at IS NULL OR expires_at > NOW())
        "#,
        api_key
    )
    .fetch_optional(db)
    .await?;

    let result = if let Some(r) = row {
        let permissions = if let Ok(perms) = serde_json::from_value::<Vec<String>>(r.permissions) {
            perms
        } else {
            vec![]
        };

        Some(ApiKey {
            id: r.id,
            api_key: r.api_key,
            name: r.name,
            user_id: r.user_id,
            permissions,
            rate_limit_tier: r.rate_limit_tier,
            is_active: r.is_active,
            expires_at: r.expires_at,
            last_used_at: r.last_used_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    } else {
        None
    };

    if let Some(api_key_record) = result {
        // Update last used timestamp
        sqlx::query!(
            r#"
            UPDATE api_keys
            SET last_used_at = NOW()
            WHERE id = $1
            "#,
            api_key_record.id
        )
        .execute(db)
        .await?;

        // Fetch the associated user
        let user_row = sqlx::query(
            r#"
            SELECT
                u.id,
                u.email,
                u.username,
                u.password_hash,
                u.role,
                u.status,
                u.wallet_address,
                u.google_id,
                u.avatar_url,
                u.provider,
                u.provider_data,
                u.created_at,
                u.updated_at,
                u.last_login,
                u.last_provider_sync,
                u.email_verified,
                u.two_factor_enabled,
                u.profile_data
            FROM users u
            WHERE u.id = $1
            "#,
        )
        .bind(api_key_record.user_id)
        .fetch_optional(db)
        .await?;

        let user = if let Some(row) = user_row {
            User {
                id: row.try_get("id")?,
                email: row.try_get("email").unwrap_or_else(|_| String::new()),
                username: row.try_get("username").unwrap_or_else(|_| String::new()),
                password_hash: row.try_get("password_hash")?,
                role: row
                    .try_get("role")
                    .unwrap_or_else(|_| "scientist".to_string()),
                status: row
                    .try_get("status")
                    .unwrap_or_else(|_| "active".to_string()),
                wallet_address: row.try_get("wallet_address")?,
                google_id: row.try_get("google_id")?,
                avatar_url: row.try_get("avatar_url")?,
                provider: row
                    .try_get("provider")
                    .unwrap_or_else(|_| "local".to_string()),
                provider_data: row
                    .try_get("provider_data")
                    .unwrap_or_else(|_| serde_json::json!({})),
                created_at: row
                    .try_get("created_at")
                    .unwrap_or_else(|_| chrono::Utc::now()),
                updated_at: row
                    .try_get("updated_at")
                    .unwrap_or_else(|_| chrono::Utc::now()),
                last_login: row.try_get("last_login")?,
                last_provider_sync: row.try_get("last_provider_sync")?,
                email_verified: row.try_get("email_verified")?,
                two_factor_enabled: row.try_get("two_factor_enabled")?,
                profile_data: row
                    .try_get("profile_data")
                    .unwrap_or_else(|_| serde_json::json!({})),
            }
        } else {
            return Ok(None);
        };

        Ok(Some((api_key_record, user)))
    } else {
        Ok(None)
    }
}

/// Mask API key for logging (show only first 8 chars)
fn mask_api_key(key: &str) -> String {
    if key.len() > 8 {
        format!("{}...", &key[..8])
    } else {
        "***".to_string()
    }
}

/// Extension extractor for authenticated user via API key
pub struct ApiKeyUser(pub User, pub ApiKey);

impl<S> axum::extract::FromRequestParts<S> for ApiKeyUser
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let user = parts
            .extensions
            .get::<User>()
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let api_key = parts
            .extensions
            .get::<ApiKey>()
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // Check if auth method is api_key
        let auth_method = parts
            .extensions
            .get::<String>()
            .map(|s| s.as_str())
            .unwrap_or("");

        if auth_method != "api_key" {
            return Err(StatusCode::UNAUTHORIZED);
        }

        Ok(ApiKeyUser(user.clone(), api_key.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    #[test]
    fn test_extract_api_key_from_header() {
        // Test X-API-Key header
        let req = Request::builder()
            .header("x-api-key", "test-key-123")
            .body(Body::empty())
            .unwrap();

        assert_eq!(extract_api_key(&req), Some("test-key-123".to_string()));

        // Test Authorization header with ApiKey scheme
        let req = Request::builder()
            .header("authorization", "ApiKey test-key-456")
            .body(Body::empty())
            .unwrap();

        assert_eq!(extract_api_key(&req), Some("test-key-456".to_string()));

        // Test no API key
        let req = Request::builder().body(Body::empty()).unwrap();

        assert_eq!(extract_api_key(&req), None);
    }

    #[test]
    fn test_mask_api_key() {
        assert_eq!(mask_api_key("dlp_abcd1234efgh5678"), "dlp_abcd...");
        assert_eq!(mask_api_key("short"), "***");
        assert_eq!(mask_api_key("12345678"), "***");
        assert_eq!(mask_api_key("123456789"), "12345678...");
    }
}
