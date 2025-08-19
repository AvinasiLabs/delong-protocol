use crate::AppError;
use crate::AppState;
use crate::utils::jwt::Claims;
use crate::utils::jwt::JwtService;
use axum::extract::State;
use axum::{
    extract::{FromRequestParts, Request},
    http::{header::AUTHORIZATION, request::Parts},
    middleware::Next,
    response::Response,
};

/// User information extracted from JWT
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: i32,
    pub email: String,
    pub username: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

impl TryFrom<Claims> for AuthUser {
    type Error = String;

    fn try_from(claims: Claims) -> Result<Self, Self::Error> {
        let user_id = claims
            .sub
            .parse::<i32>()
            .map_err(|e| format!("Failed to parse user ID: {}", e))?;

        Ok(Self {
            id: user_id,
            email: claims.email,
            username: claims.username,
            roles: claims.roles,
            permissions: claims.permissions,
        })
    }
}

impl AuthUser {
    /// Get the user ID
    pub fn user_id(&self) -> Result<i32, AppError> {
        Ok(self.id)
    }

    /// Check if user has a specific role
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.contains(&role.to_string())
    }
}

/// Admin user wrapper (requires admin role)
#[derive(Debug, Clone)]
pub struct AdminUser(pub AuthUser);

/// JWT authentication middleware
pub async fn jwt_auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    // Get JWT service
    let jwt_service = JwtService::new(state.config.clone());

    // Extract Authorization header
    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .ok_or_else(|| {
            tracing::error!("Missing or invalid Authorization header");
            AppError::Authentication("Missing or invalid Authorization header".to_string())
        })?;

    // Extract token from header
    let token = jwt_service
        .extract_token_from_header(auth_header)
        .map_err(|e| {
            tracing::error!("Failed to extract token: {}", e);
            AppError::Authentication(format!("Failed to extract token: {}", e))
        })?;

    // Validate token
    let claims = jwt_service.validate_access_token(token).map_err(|e| {
        tracing::error!("Token validation failed: {}", e);
        AppError::Authentication(format!("Invalid token: {}", e))
    })?;

    // Create AuthUser from claims
    let auth_user = AuthUser::try_from(claims).map_err(|e| {
        tracing::error!("Failed to create AuthUser from claims: {}", e);
        AppError::Authentication(format!("Failed to parse claims: {}", e))
    })?;

    // Add user to request extensions
    req.extensions_mut().insert(auth_user);

    // Continue with the request
    Ok(next.run(req).await)
}

/// Extractor for authenticated user
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or(AppError::Authentication(
                "User not authenticated".to_string(),
            ))
    }
}

/// Extractor for admin user
impl<S> FromRequestParts<S> for AdminUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // First extract as regular auth user
        let auth_user =
            parts
                .extensions
                .get::<AuthUser>()
                .cloned()
                .ok_or(AppError::Authentication(
                    "User not authenticated".to_string(),
                ))?;

        // Check if user has admin role
        if !auth_user.has_role("admin") {
            return Err(AppError::Authorization(
                "Admin privileges required".to_string(),
            ));
        }

        Ok(AdminUser(auth_user))
    }
}
