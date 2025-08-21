//! Proxy handler for forwarding requests to Secure service
//!
//! This module implements a generic proxy handler that forwards all requests
//! matching certain path prefixes to the Secure service, similar to how Nginx
//! handles proxy_pass directives.

use axum::{
    Extension, Json,
    body::{Body, Bytes},
    extract::{OriginalUri, State},
    http::{HeaderMap, Method, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::{error, info};

use crate::{AppState, infra::proxy::AuthContext, models::user::User};
use avinapi::prelude::*;

/// Generic proxy handler that forwards all requests to Secure service
///
/// This handler acts like Nginx's proxy_pass, automatically forwarding
/// any request that matches the configured path prefix to the Secure service.
///
/// ## Usage in router:
/// ```rust
/// // Forward all /api/datasets/* requests
/// .nest("/api/datasets", proxy_routes())
/// // Forward all /api/algorithms/* requests
/// .nest("/api/algorithms", proxy_routes())
/// ```
pub async fn proxy_handler(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Extension(auth_method): Extension<String>,
    OriginalUri(original_uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Response> {
    // Extract the proxy client
    let proxy_client = state
        .proxy_client
        .as_ref()
        .ok_or_else(|| AppError::Config("Proxy client not configured".to_string()))?;

    // Build auth context from the authenticated user
    let auth_context = build_auth_context(&user, &auth_method, &headers);

    // Log the proxy request
    info!(
        "[Proxy] {} {} -> Secure (user: {}, auth: {})",
        method,
        original_uri.path(),
        user.email,
        auth_method
    );

    // Forward the request
    proxy_client
        .forward_request(
            method,
            original_uri.path(),
            headers,
            Some(body),
            auth_context,
        )
        .await
}

/// Middleware to extract authentication method
///
/// This middleware runs after authentication and stores the auth method
/// (jwt or api_key) in the request extensions for the proxy handler to use.
pub async fn extract_auth_method(
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Check which authentication method was used
    let auth_method = if req.headers().get(header::AUTHORIZATION).is_some() {
        "jwt"
    } else if req.headers().get("x-api-key").is_some() {
        "api_key"
    } else {
        "unknown"
    };

    // Store in extensions
    req.extensions_mut().insert(auth_method.to_string());

    Ok(next.run(req).await)
}

/// Build authentication context from user and request information
fn build_auth_context(user: &User, auth_method: &str, headers: &HeaderMap) -> AuthContext {
    AuthContext {
        user_id: user.id.to_string(),
        email: user.email.clone(),
        auth_method: auth_method.to_string(),
        tenant_id: None, // Can be extended for multi-tenancy
        scopes: get_user_scopes(user),
        client_ip: extract_client_ip(headers),
        request_id: extract_request_id(headers),
    }
}

/// Get user permission scopes
fn get_user_scopes(user: &User) -> Vec<String> {
    let mut scopes = vec!["read".to_string()];

    if user.email_verified.unwrap_or(false) {
        scopes.push("write".to_string());
    }

    // Add more scopes based on user roles/permissions
    // if user.is_admin {
    //     scopes.push("admin".to_string());
    // }

    scopes
}

/// Extract client IP from headers
fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
    // Try X-Forwarded-For first (standard proxy header)
    if let Some(forwarded_for) = headers.get("x-forwarded-for") {
        if let Ok(value) = forwarded_for.to_str() {
            // Take the first IP if there are multiple
            return Some(value.split(',').next().unwrap_or(value).trim().to_string());
        }
    }

    // Try X-Real-IP (common alternative)
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(value) = real_ip.to_str() {
            return Some(value.to_string());
        }
    }

    None
}

/// Extract request ID from headers
fn extract_request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string()
}

/// Create router for proxied paths
///
/// This creates a catch-all router that forwards all requests to the proxy handler.
/// Use this with `.nest()` to forward specific path prefixes.
///
/// ## Example:
/// ```rust
/// let app = Router::new()
///     // These paths are handled locally
///     .route("/auth/login", post(login))
///     .route("/auth/register", post(register))
///
///     // These paths are forwarded to Secure
///     .nest("/api/datasets", proxy_routes())
///     .nest("/api/algorithms", proxy_routes())
///     .nest("/api/tasks", proxy_routes())
///
///     // Apply authentication middleware
///     .layer(middleware::from_fn(auth_middleware));
/// ```
pub fn proxy_routes() -> axum::Router<AppState> {
    use axum::routing::{Router, any};

    Router::new()
        // Catch all methods and paths
        .route("/{*path}", any(proxy_handler))
        // Also handle the root path
        .route("/", any(proxy_handler))
}

/// Health check endpoint for Secure service
///
/// This endpoint checks if the Secure service is reachable.
/// Check health status of Secure TEE service
#[utoipa::path(
    get,
    path = "/api/secure/health",
    tag = "Proxy",
    responses(
        (status = 200, description = "Secure service health status")
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    )
)]
pub async fn secure_health_check(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let proxy_client = state
        .proxy_client
        .as_ref()
        .ok_or_else(|| AppError::Config("Proxy client not configured".to_string()))?;

    match proxy_client.health_check().await {
        Ok(true) => Ok((
            StatusCode::OK,
            Json(json!({
                "status": "healthy",
                "service": "secure",
                "message": "Secure service is reachable"
            })),
        )),
        Ok(false) => Ok((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "unhealthy",
                "service": "secure",
                "message": "Secure service is not responding"
            })),
        )),
        Err(e) => {
            error!("Health check failed: {}", e);
            Ok((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "status": "error",
                    "service": "secure",
                    "message": format!("Health check failed: {}", e)
                })),
            ))
        }
    }
}

/// Proxy configuration middleware
///
/// This middleware can be used to add additional headers or modify requests
/// before they are forwarded to the Secure service.
pub async fn proxy_config_middleware(
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Add custom headers for proxied requests
    req.headers_mut()
        .insert("x-forwarded-by", "delong-core".parse().unwrap());

    // Add forwarded proto header
    let scheme = req.uri().scheme_str().unwrap_or("http").to_string();
    req.headers_mut()
        .insert("x-forwarded-proto", scheme.parse().unwrap());

    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_client_ip() {
        let mut headers = HeaderMap::new();

        // Test X-Forwarded-For with multiple IPs
        headers.insert("x-forwarded-for", "192.168.1.1, 10.0.0.1".parse().unwrap());
        assert_eq!(extract_client_ip(&headers), Some("192.168.1.1".to_string()));

        // Test X-Real-IP
        headers.clear();
        headers.insert("x-real-ip", "192.168.1.2".parse().unwrap());
        assert_eq!(extract_client_ip(&headers), Some("192.168.1.2".to_string()));

        // Test no IP headers
        headers.clear();
        assert_eq!(extract_client_ip(&headers), None);
    }

    #[test]
    fn test_get_user_scopes() {
        let mut user = User {
            id: 1,
            email: "test@example.com".to_string(),
            username: "test".to_string(),
            password_hash: Some("hash".to_string()),
            role: "scientist".to_string(),
            status: "active".to_string(),
            wallet_address: None,
            google_id: None,
            avatar_url: None,
            provider: "local".to_string(),
            provider_data: serde_json::json!({}),
            email_verified: Some(false),
            two_factor_enabled: Some(false),
            last_login: None,
            last_provider_sync: None,
            profile_data: serde_json::json!({}),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        // Unverified user only gets read scope
        assert_eq!(get_user_scopes(&user), vec!["read".to_string()]);

        // Verified user gets read and write scopes
        user.email_verified = Some(true);
        assert_eq!(
            get_user_scopes(&user),
            vec!["read".to_string(), "write".to_string()]
        );
    }
}
