//! Internal JWT verification middleware for Secure service
//!
//! This middleware validates internal JWT tokens sent by the Core service
//! when forwarding authenticated requests to the Secure service.

use avinapi::prelude::AppError;
use axum::{
    body::Body,
    extract::{OriginalUri, Request},
    http::{HeaderMap, Method},
    middleware::Next,
    response::Response,
};
use bytes::Bytes;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

/// Authentication context from Core service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    /// User ID from JWT or API key
    pub user_id: String,
    /// User email
    pub email: String,
    /// Authentication method (jwt or api_key)
    pub auth_method: String,
    /// Tenant/organization ID if applicable
    pub tenant_id: Option<String>,
    /// Permission scopes
    pub scopes: Vec<String>,
    /// Original client IP
    pub client_ip: Option<String>,
    /// Request ID for tracing
    pub request_id: String,
}

/// Internal JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
struct InternalJwtClaims {
    /// Subject (user ID)
    sub: String,
    /// Issued at timestamp
    iat: u64,
    /// Expiration timestamp
    exp: u64,
    /// Issuer (should be "delong-core")
    iss: String,
    /// Audience (should be "delong-secure")
    aud: String,
    /// Authentication context
    context: AuthContext,
    /// Request signature for integrity
    request_signature: RequestSignature,
}

/// Request signature for integrity verification
#[derive(Debug, Serialize, Deserialize)]
struct RequestSignature {
    /// HTTP method
    method: String,
    /// Request path
    path: String,
    /// SHA256 hash of request body (if present)
    body_digest: Option<String>,
    /// Timestamp
    timestamp: u64,
}

/// Middleware to verify internal JWT from Core service
pub async fn internal_jwt_middleware(
    OriginalUri(original_uri): OriginalUri,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Skip authentication in test mode
    if std::env::var("TEST_MODE").unwrap_or_default() == "true"
        || std::env::var("SKIP_AUTH").unwrap_or_default() == "true"
    {
        debug!("Skipping JWT authentication in test mode");

        // Create a mock auth context for testing
        let mock_context = AuthContext {
            user_id: "test-user-001".to_string(),
            email: "test@example.com".to_string(),
            auth_method: "test".to_string(),
            tenant_id: None,
            scopes: vec!["read".to_string(), "write".to_string()],
            client_ip: Some("127.0.0.1".to_string()),
            request_id: "test-request-id".to_string(),
        };

        // Add the mock context to request extensions
        req.extensions_mut().insert(AuthenticatedUser {
            user_id: mock_context.user_id.clone(),
            email: mock_context.email.clone(),
            auth_method: mock_context.auth_method.clone(),
            scopes: mock_context.scopes.clone(),
        });
        req.extensions_mut().insert(mock_context);

        return Ok(next.run(req).await);
    }

    // Extract the internal JWT from header
    let token = extract_internal_jwt(req.headers()).ok_or_else(|| {
        warn!("Missing X-Internal-JWT header");
        AppError::Authentication("Missing internal authentication token".into())
    })?;

    // Get JWT secret from environment
    let jwt_secret = std::env::var("INTERNAL_JWT_SECRET").map_err(|_| {
        error!("INTERNAL_JWT_SECRET not configured");
        AppError::Config("INTERNAL_JWT_SECRET not configured".to_string())
    })?;

    // Decode and validate the JWT
    let claims = decode_and_validate_jwt(&token, &jwt_secret).map_err(|e| {
        warn!("JWT validation failed: {}", e);
        AppError::Authentication(format!("Invalid authentication token: {}", e))
    })?;

    // Verify request integrity
    let method = req.method().clone();
    let path = if let Some(query) = original_uri.query() {
        format!("{}?{}", original_uri.path(), query)
    } else {
        original_uri.path().to_string()
    };

    info!(
        "SECURE JWT: Verifying request - method: {}, path from OriginalUri: '{}'",
        method, path
    );

    // Read and replace the body for digest verification
    let body_bytes = read_body_bytes(&mut req).await.map_err(|e| {
        error!("Failed to read request body: {}", e);
        AppError::Internal(format!("Failed to process request: {}", e))
    })?;

    // Verify request signature
    info!(
        "SECURE JWT: Signature contains - method: {}, path: '{}'",
        claims.request_signature.method, claims.request_signature.path
    );
    if !verify_request_signature(&claims.request_signature, &method, &path, &body_bytes) {
        warn!("Request signature verification failed");
        return Err(AppError::Authentication(
            "Request integrity check failed".into(),
        ));
    }

    // Log the authenticated request
    debug!(
        "Authenticated request from Core: user={}, method={}, path={}, auth_method={}",
        claims.context.user_id, method, path, claims.context.auth_method
    );

    // Store auth context in request extensions
    req.extensions_mut().insert(claims.context.clone());
    req.extensions_mut().insert(AuthenticatedUser {
        user_id: claims.context.user_id.clone(),
        email: claims.context.email.clone(),
        auth_method: claims.context.auth_method.clone(),
        scopes: claims.context.scopes.clone(),
    });

    // Continue with the request
    Ok(next.run(req).await)
}

/// Extract internal JWT from headers
fn extract_internal_jwt(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-internal-jwt")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Decode and validate JWT
fn decode_and_validate_jwt(token: &str, secret: &str) -> Result<InternalJwtClaims, String> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&["delong-core"]);
    validation.set_audience(&["delong-secure"]);

    // Allow some clock skew (5 seconds)
    validation.leeway = 5;

    let token_data = decode::<InternalJwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| format!("JWT decode error: {}", e))?;

    // Additional validation: check if token is not too old
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("Time error: {}", e))?
        .as_secs();

    // Internal JWTs should be short-lived (max 120 seconds old)
    if now > token_data.claims.iat + 120 {
        return Err("Token is too old".to_string());
    }

    Ok(token_data.claims)
}

/// Verify request signature
fn verify_request_signature(
    signature: &RequestSignature,
    method: &Method,
    path: &str,
    body: &Option<Bytes>,
) -> bool {
    // Verify method and path match
    if signature.method != method.to_string() || signature.path != path {
        warn!(
            "Request signature mismatch: signature has method='{}', path='{}', but request has method='{}', path='{}'",
            signature.method, signature.path, method, path
        );
        return false;
    }

    // Verify body digest if present
    if let Some(body_bytes) = body {
        let mut hasher = Sha256::new();
        hasher.update(body_bytes);
        let computed_digest = format!("{:x}", hasher.finalize());

        if let Some(expected_digest) = &signature.body_digest {
            if &computed_digest != expected_digest {
                warn!("Body digest mismatch");
                return false;
            }
        } else {
            // Signature doesn't include body digest but we have a body
            warn!("Missing body digest in signature");
            return false;
        }
    } else if signature.body_digest.is_some() {
        // Signature includes body digest but we don't have a body
        warn!("Unexpected body digest in signature");
        return false;
    }

    // Verify timestamp is recent (within 60 seconds)
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if (now as i64 - signature.timestamp as i64).abs() > 60 {
        warn!("Request timestamp is too old or in the future");
        return false;
    }

    true
}

/// Read body bytes and replace in request
async fn read_body_bytes(req: &mut Request<Body>) -> Result<Option<Bytes>, String> {
    use axum::body::to_bytes;

    // Check if there's a body
    let content_length = req
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    if content_length == 0 {
        return Ok(None);
    }

    // Extract the body
    let body = std::mem::replace(req.body_mut(), Body::empty());

    // Read the bytes
    let bytes = to_bytes(body, content_length)
        .await
        .map_err(|e| format!("Failed to read body: {}", e))?;

    // Create a new body from the bytes and put it back
    *req.body_mut() = Body::from(bytes.clone());

    Ok(Some(bytes))
}

/// Authenticated user information extracted from internal JWT
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub email: String,
    pub auth_method: String,
    pub scopes: Vec<String>,
}

/// Extension trait for extracting authenticated user from request
pub trait AuthExt {
    fn auth_context(&self) -> Option<&AuthContext>;
    fn authenticated_user(&self) -> Option<&AuthenticatedUser>;
    fn has_scope(&self, scope: &str) -> bool;
}

impl AuthExt for Request<Body> {
    fn auth_context(&self) -> Option<&AuthContext> {
        self.extensions().get::<AuthContext>()
    }

    fn authenticated_user(&self) -> Option<&AuthenticatedUser> {
        self.extensions().get::<AuthenticatedUser>()
    }

    fn has_scope(&self, scope: &str) -> bool {
        self.authenticated_user()
            .map(|user| user.scopes.contains(&scope.to_string()))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    #[test]
    fn test_verify_request_signature() {
        let signature = RequestSignature {
            method: "GET".to_string(),
            path: "/api/datasets".to_string(),
            body_digest: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        assert!(verify_request_signature(
            &signature,
            &Method::GET,
            "/api/datasets",
            &None
        ));

        // Test with wrong method
        assert!(!verify_request_signature(
            &signature,
            &Method::POST,
            "/api/datasets",
            &None
        ));

        // Test with wrong path
        assert!(!verify_request_signature(
            &signature,
            &Method::GET,
            "/api/algorithms",
            &None
        ));
    }

    #[test]
    fn test_jwt_validation() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = InternalJwtClaims {
            sub: "user123".to_string(),
            iat: now,
            exp: now + 60,
            iss: "delong-core".to_string(),
            aud: "delong-secure".to_string(),
            context: AuthContext {
                user_id: "user123".to_string(),
                email: "test@example.com".to_string(),
                auth_method: "jwt".to_string(),
                tenant_id: None,
                scopes: vec!["read".to_string(), "write".to_string()],
                client_ip: Some("192.168.1.1".to_string()),
                request_id: "req123".to_string(),
            },
            request_signature: RequestSignature {
                method: "GET".to_string(),
                path: "/api/datasets".to_string(),
                body_digest: None,
                timestamp: now,
            },
        };

        let secret = "test-secret";
        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap();

        let decoded = decode_and_validate_jwt(&token, secret).unwrap();
        assert_eq!(decoded.sub, "user123");
        assert_eq!(decoded.context.email, "test@example.com");
    }
}
