use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{Response, IntoResponse},
    Extension,
    Json,
};
use common::{ApiError, ApiResult, ApiResponse, AuthContext, JwtUtils, ResponseCode};
use tracing::{debug, warn};

/// JWT authentication middleware
#[derive(Clone)]
pub struct JwtMiddleware {
    jwt_utils: JwtUtils,
    enabled: bool,
}

impl JwtMiddleware {
    /// Create new JWT middleware
    pub fn new(jwt_secret: String, enabled: bool) -> Self {
        Self {
            jwt_utils: JwtUtils::new(jwt_secret),
            enabled,
        }
    }
    
    /// Middleware function for JWT authentication
    pub async fn auth_middleware(
        Extension(jwt_middleware): Extension<JwtMiddleware>,
        mut request: Request,
        next: Next,
    ) -> Response {
        // If JWT is disabled, allow all requests
        if !jwt_middleware.enabled {
            debug!("JWT authentication is disabled");
            return next.run(request).await;
        }
        
        debug!("JWT authentication is enabled");
        
        // Extract Authorization header
        let headers = request.headers();
        let auth_header = match headers.get("authorization").and_then(|h| h.to_str().ok()) {
            Some(header) => header,
            None => {
                warn!("Missing Authorization header");
                return create_auth_error_response("Missing Authorization header");
            }
        };
        
        // Check Bearer token format
        if !auth_header.starts_with("Bearer ") {
            warn!("Invalid Authorization header format. Expected 'Bearer <token>'");
            return create_auth_error_response("Invalid Authorization header format. Expected 'Bearer <token>'");
        }
        
        let token = &auth_header[7..]; // Remove "Bearer " prefix
        
        // Validate JWT token
        let claims = match jwt_middleware.jwt_utils.validate_token(token) {
            Ok(claims) => claims,
            Err(e) => {
                warn!("JWT validation failed: {}", e);
                return create_auth_error_response(&format!("Invalid JWT token: {}", e));
            }
        };
        
        // Check if token is expired
        if claims.is_expired() {
            warn!("JWT token is expired");
            return create_auth_error_response("JWT token has expired");
        }
        
        // Create auth context and add to request extensions
        let auth_context = AuthContext::from_claims(claims);
        request.extensions_mut().insert(auth_context);
        
        debug!("JWT authentication successful");
        next.run(request).await
    }
}

/// Create a standardized authentication error response
fn create_auth_error_response(_message: &str) -> Response {
    let error_response = ApiResponse::<()> {
        code: ResponseCode::Unauthorized,
        data: None,
        request_id: None,
    };
    
    (StatusCode::UNAUTHORIZED, Json(error_response)).into_response()
}

/// Extract authentication context from request
pub fn extract_auth_context(headers: &HeaderMap) -> Option<AuthContext> {
    headers.get("x-auth-context")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| serde_json::from_str(s).ok())
}

/// Check if user has admin role
pub fn require_admin(auth_context: &AuthContext) -> ApiResult<()> {
    if !auth_context.is_admin() {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

/// Extract user role from authentication context
pub fn get_user_role(auth_context: &AuthContext) -> String {
    auth_context.role.clone()
}

/// Extract user ID from authentication context
pub fn get_user_id(auth_context: &AuthContext) -> String {
    auth_context.user_id.clone()
}

/// Check if authentication is enabled
pub fn is_auth_enabled(jwt_middleware: &JwtMiddleware) -> bool {
    jwt_middleware.enabled
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, Method},
    };
    use common::Claims;
    
    #[tokio::test]
    async fn test_jwt_middleware_disabled() {
        let _middleware = JwtMiddleware::new("test_secret".to_string(), false);
        let _request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        
        // Should pass through without authentication when disabled
        // Note: In actual test, you'd need to set up a proper next handler
    }
    
    #[tokio::test]
    async fn test_jwt_middleware_enabled_missing_header() {
        let _middleware = JwtMiddleware::new("test_secret".to_string(), true);
        let _request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        
        // Should fail without Authorization header
        // Note: In actual test, you'd need to set up a proper next handler
    }
    
    #[test]
    fn test_require_admin() {
        let admin_claims = Claims::new("admin".to_string(), "user123".to_string(), 3600);
        let admin_context = AuthContext::from_claims(admin_claims);
        
        let user_claims = Claims::new("user".to_string(), "user456".to_string(), 3600);
        let user_context = AuthContext::from_claims(user_claims);
        
        assert!(require_admin(&admin_context).is_ok());
        assert!(require_admin(&user_context).is_err());
    }
    
    #[test]
    fn test_get_user_role() {
        let claims = Claims::new("admin".to_string(), "user123".to_string(), 3600);
        let context = AuthContext::from_claims(claims);
        
        assert_eq!(get_user_role(&context), "admin");
    }
    
    #[test]
    fn test_get_user_id() {
        let claims = Claims::new("admin".to_string(), "user123".to_string(), 3600);
        let context = AuthContext::from_claims(claims);
        
        assert_eq!(get_user_id(&context), "user123");
    }
} 