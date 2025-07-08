//! Core service middleware
//!
//! This module provides middleware specific to the core service, while reusing
//! common middleware from the shared common crate where possible.

use crate::{
    AppState,
    config::AppConfig,
    models::api_key::ApiKey,
    utils::jwt::{Claims, verify_token},
};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use common::middleware::{
    MiddlewareUtils, logging::logging_middleware as common_logging_middleware,
    request_id::request_id_middleware as common_request_id_middleware,
};

use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::{instrument, warn};

// Re-export common middleware for convenience
pub use common::middleware::{
    REQUEST_ID_HEADER,
    logging::{LoggingConfig, logging_middleware},
    request_id::{RequestIdConfig, RequestIdGenerator, request_id_middleware},
};

/// Authentication middleware for protected routes
#[instrument(skip(request, next, state), fields(
    method = %request.method(),
    path = %request.uri().path(),
    request_id
))]
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract authorization header
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Extract bearer token
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token and get user claims
    let claims = verify_token(token, &state.jwt_config).map_err(|e| {
        warn!("Token validation failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    // Add user information to request extensions
    request.extensions_mut().insert(claims);

    // Continue with the request
    Ok(next.run(request).await)
}

/// Admin middleware for admin-only routes
#[instrument(skip(request, next), fields(
    method = %request.method(),
    path = %request.uri().path(),
    request_id
))]
pub async fn admin_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    // Check if user claims are available (should be set by auth_middleware)
    let claims = request
        .extensions()
        .get::<Claims>()
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Check if user has admin role
    if claims.role != "admin" {
        warn!(
            "Non-admin user attempted to access admin endpoint: {}",
            claims.sub
        );
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(next.run(request).await)
}

/// API key middleware for API key authentication
#[instrument(skip(request, next, state), fields(
    method = %request.method(),
    path = %request.uri().path(),
    request_id
))]
pub async fn api_key_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract API key from header
    let api_key = request
        .headers()
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate API key
    let api_key_info = ApiKey::find_by_key(&state.db, api_key)
        .await
        .map_err(|e| {
            warn!("API key validation failed: {}", e);
            StatusCode::UNAUTHORIZED
        })?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Check if API key is active
    if !api_key_info.is_active {
        warn!("Inactive API key used: {}", api_key_info.id);
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Add API key information to request extensions
    request.extensions_mut().insert(api_key_info);

    // Continue with the request
    Ok(next.run(request).await)
}

/// CORS middleware configuration
pub fn cors_middleware(_config: &AppConfig) -> CorsLayer {
    let mut cors = CorsLayer::new()
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
            axum::http::header::HeaderName::from_static("x-api-key"),
            axum::http::header::HeaderName::from_static("x-request-id"),
        ]);

    // Configure origins - always allow any origin for now
    cors = cors.allow_origin(Any);

    cors
}

/// Rate limiting middleware
#[instrument(skip(request, next), fields(
    method = %request.method(),
    path = %request.uri().path(),
    request_id
))]
pub async fn rate_limit_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    let headers = request.headers();
    let path = request.uri().path();

    // Extract client identifier using common utilities
    let client_id = MiddlewareUtils::extract_client_ip_with_fallback(headers, None)
        .unwrap_or_else(|| "unknown".to_string());

    // Determine rate limit for this request
    let rate_limit = determine_rate_limit(path, headers).await;

    // Check rate limit
    if !check_rate_limit(&client_id, &rate_limit).await {
        warn!("Rate limit exceeded for client: {}", client_id);
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Process request
    let mut response = next.run(request).await;

    // Add rate limit headers
    add_rate_limit_headers(&mut response, &rate_limit).await;

    Ok(response)
}

/// Security headers middleware
#[instrument(skip(request, next), fields(
    method = %request.method(),
    path = %request.uri().path(),
    request_id
))]
pub async fn security_headers_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract HTTPS information before moving the request
    let is_https = should_add_hsts_header(&request);

    let mut response = next.run(request).await;

    // Add security headers
    let headers = response.headers_mut();

    // Content Security Policy
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'",
        ),
    );

    // X-Frame-Options
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));

    // X-Content-Type-Options
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );

    // X-XSS-Protection
    headers.insert(
        "x-xss-protection",
        HeaderValue::from_static("1; mode=block"),
    );

    // Referrer Policy
    headers.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );

    // Strict Transport Security (HTTPS only)
    if is_https {
        headers.insert(
            "strict-transport-security",
            HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
        );
    }

    Ok(response)
}

/// Check if request should get HSTS header
fn should_add_hsts_header(request: &Request) -> bool {
    // Check if request is over HTTPS
    request.uri().scheme_str() == Some("https")
        || request
            .headers()
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            == Some("https")
}

/// Rate limit configuration
#[derive(Debug, Clone)]
struct RateLimit {
    requests: u32,
    window_seconds: u64,
}

/// Determine rate limit based on request path and headers
async fn determine_rate_limit(path: &str, headers: &HeaderMap) -> RateLimit {
    // Check if this is an API key request
    if headers.contains_key("x-api-key") {
        return RateLimit {
            requests: 1000,
            window_seconds: 60,
        };
    }

    // Different limits for different endpoints
    match path {
        p if p.starts_with("/auth/") => RateLimit {
            requests: 10,
            window_seconds: 60,
        },
        p if p.starts_with("/api/") => RateLimit {
            requests: 100,
            window_seconds: 60,
        },
        p if p.starts_with("/admin/") => RateLimit {
            requests: 50,
            window_seconds: 60,
        },
        _ => RateLimit {
            requests: 200,
            window_seconds: 60,
        },
    }
}

/// Check if client has exceeded rate limit
async fn check_rate_limit(_client_id: &str, _rate_limit: &RateLimit) -> bool {
    // TODO: Implement actual rate limiting with Redis or in-memory store
    // For now, always allow requests
    true
}

/// Add rate limit headers to response
async fn add_rate_limit_headers(response: &mut Response, rate_limit: &RateLimit) {
    let headers = response.headers_mut();

    // Add rate limit headers
    headers.insert(
        "x-ratelimit-limit",
        HeaderValue::from_str(&rate_limit.requests.to_string()).unwrap(),
    );
    headers.insert(
        "x-ratelimit-remaining",
        HeaderValue::from_str(&(rate_limit.requests - 1).to_string()).unwrap(),
    );
    headers.insert(
        "x-ratelimit-reset",
        HeaderValue::from_str(
            &(chrono::Utc::now().timestamp() + rate_limit.window_seconds as i64).to_string(),
        )
        .unwrap(),
    );
}

/// Middleware stack configuration
pub struct MiddlewareStack {
    config: Arc<AppConfig>,
}

impl MiddlewareStack {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self { config }
    }

    /// Build middleware stack for development
    pub fn build(&self) -> axum::Router {
        use tower::ServiceBuilder;
        use tower_http::{compression::CompressionLayer, trace::TraceLayer};

        axum::Router::new().layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(cors_middleware(&self.config))
                .layer(axum::middleware::from_fn(common_request_id_middleware))
                .layer(axum::middleware::from_fn(common_logging_middleware))
                .layer(axum::middleware::from_fn(security_headers_middleware)),
        )
    }

    /// Build middleware stack for production with rate limiting
    pub fn build_production(&self) -> axum::Router {
        use tower::ServiceBuilder;
        use tower_http::{compression::CompressionLayer, trace::TraceLayer};

        axum::Router::new().layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(cors_middleware(&self.config))
                .layer(axum::middleware::from_fn(common_request_id_middleware))
                .layer(axum::middleware::from_fn(common_logging_middleware))
                .layer(axum::middleware::from_fn(rate_limit_middleware))
                .layer(axum::middleware::from_fn(security_headers_middleware)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn test_should_add_hsts_header() {
        let request = Request::builder()
            .uri("https://example.com/test")
            .body(axum::body::Body::empty())
            .unwrap();

        assert!(should_add_hsts_header(&request));

        let mut request2 = Request::builder()
            .uri("http://example.com/test")
            .body(axum::body::Body::empty())
            .unwrap();

        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        *request2.headers_mut() = headers;

        assert!(should_add_hsts_header(&request2));
    }

    #[tokio::test]
    async fn test_determine_rate_limit() {
        let mut headers = HeaderMap::new();
        let rate_limit = determine_rate_limit("/api/test", &headers).await;
        assert_eq!(rate_limit.requests, 100);
        assert_eq!(rate_limit.window_seconds, 60);

        headers.insert("x-api-key", HeaderValue::from_static("test-key"));
        let rate_limit = determine_rate_limit("/api/test", &headers).await;
        assert_eq!(rate_limit.requests, 1000);
        assert_eq!(rate_limit.window_seconds, 60);
    }

    #[tokio::test]
    async fn test_check_rate_limit() {
        let rate_limit = RateLimit {
            requests: 10,
            window_seconds: 60,
        };

        // Currently always returns true
        assert!(check_rate_limit("test-client", &rate_limit).await);
    }
}
