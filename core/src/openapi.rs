//! OpenAPI documentation configuration
//!
//! This module configures OpenAPI/Swagger documentation for the Core service,
//! providing interactive API documentation through Scalar UI.

use utoipa::{
    Modify, OpenApi,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};
use utoipa_scalar::Scalar;

/// OpenAPI documentation structure
#[derive(OpenApi)]
#[openapi(
    info(
        title = "DeLong Protocol Core API",
        version = "0.2.0",
        description = "Core service API for DeLong Protocol, providing authentication \
                       and proxy services to the Secure TEE backend.",
        contact(
            name = "DeLong Protocol Team",
            email = "support@delong.protocol"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    servers(
        (url = "http://localhost:11020", description = "Local development server"),
        (url = "https://api.delong.protocol", description = "Production server")
    ),
    paths(
        // Health endpoints
        crate::handlers::health::health,

        // Auth endpoints
        crate::handlers::auth::register_user,
        crate::handlers::auth::login_user,
    ),
    components(
        schemas(
            // Health schemas
            crate::handlers::health::HealthResponse,

            // Auth schemas
            crate::handlers::auth::RegisterRequest,
            crate::handlers::auth::LoginRequest,
            crate::handlers::auth::AuthResponse,
            crate::handlers::auth::UserResponse,

            // Common schemas
            crate::models::user::User,
            crate::models::user::UserRole,
            crate::models::user::UserStatus,
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "System", description = "System health check endpoints"),
        (name = "Auth", description = "Authentication and user registration"),
    )
)]
pub struct ApiDoc;

/// Security configuration for OpenAPI
pub struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some("JWT Bearer token authentication"))
                        .build(),
                ),
            );
        }
    }
}

/// Generate OpenAPI specification JSON
pub fn generate_openapi_spec() -> String {
    ApiDoc::openapi().to_pretty_json().unwrap_or_default()
}

/// Configuration for API documentation
pub struct ApiDocConfig {
    /// Enable Scalar UI
    pub scalar_enabled: bool,
    /// Custom API title
    pub title: Option<String>,
    /// Custom API version
    pub version: Option<String>,
}

impl Default for ApiDocConfig {
    fn default() -> Self {
        Self {
            scalar_enabled: true,
            title: None,
            version: None,
        }
    }
}

/// Create API documentation routers based on configuration
pub fn create_api_docs(config: ApiDocConfig) -> axum::Router {
    use axum::Router;
    use axum::response::Html;
    use axum::routing::get;

    let mut router = Router::new();

    // Add OpenAPI spec endpoint
    router = router.route(
        "/api-docs/openapi.json",
        get(|| async { axum::Json(ApiDoc::openapi()) }),
    );

    // Add Scalar UI if enabled
    if config.scalar_enabled {
        router = router.route(
            "/scalar",
            get(|| async {
                let spec = ApiDoc::openapi();
                let html = Scalar::new(serde_json::to_value(spec).unwrap()).to_html();
                Html(html)
            }),
        );
    }

    router
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_spec_generation() {
        let spec = generate_openapi_spec();
        assert!(!spec.is_empty());
        assert!(spec.contains("DeLong Protocol Core API"));
    }

    #[test]
    fn test_api_doc_config_default() {
        let config = ApiDocConfig::default();
        assert!(config.scalar_enabled);
        assert!(config.title.is_none());
        assert!(config.version.is_none());
    }
}
