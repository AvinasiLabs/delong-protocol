//! OpenAPI documentation configuration
//!
//! This module configures OpenAPI/Swagger documentation for the Core service,
//! providing interactive API documentation through Scalar UI.

use utoipa::{
    Modify, OpenApi,
    openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme},
};
use utoipa_scalar::Scalar;

/// OpenAPI documentation structure
#[derive(OpenApi)]
#[openapi(
    info(
        title = "DeLong Protocol Core API",
        version = "0.2.0",
        description = "Core service API for DeLong Protocol, providing authentication, \
                       API key management, AI audit services, and proxy to the Secure TEE backend.\n\n\
                       ## Authentication\n\
                       This API supports two authentication methods:\n\
                       - **JWT Bearer Token**: Used for user authentication\n\
                       - **API Key**: Used for programmatic access\n\n\
                       ## Rate Limiting\n\
                       All endpoints are rate-limited. Default limits:\n\
                       - 100 requests per minute for authenticated users\n\
                       - 10 requests per minute for unauthenticated users",
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
        crate::handlers::auth::send_verification_code,
        crate::handlers::auth::google_auth_url,
        crate::handlers::auth::google_auth_callback,
        crate::handlers::auth::update_wallet_address,
        crate::handlers::auth::get_current_user,

        // API Key endpoints
        crate::handlers::api_key::create_api_key,
        crate::handlers::api_key::list_user_api_keys,
        crate::handlers::api_key::get_api_key,
        crate::handlers::api_key::update_api_key,
        crate::handlers::api_key::revoke_api_key,
        crate::handlers::api_key::get_user_api_key_stats,

        // AI Audit endpoints
        crate::handlers::ai_audit::create_ai_audit,
        crate::handlers::ai_audit::get_ai_audit_reports,

        // Admin endpoints
        crate::handlers::admin::get_users,
        crate::handlers::admin::create_user_admin,
        crate::handlers::admin::get_user_by_id,
        crate::handlers::admin::update_user,
        crate::handlers::admin::get_roles,
        crate::handlers::admin::get_permissions,
        crate::handlers::admin::check_permission,

        // Proxy endpoints
        // Note: secure_health_check is not included as it doesn't have utoipa annotations
    ),
    components(
        schemas(
            // Health schemas
            crate::handlers::health::HealthResponse,

            // Auth schemas
            crate::handlers::auth::RegisterRequest,
            crate::handlers::auth::LoginRequest,
            crate::handlers::auth::SendVerificationCodeRequest,
            crate::handlers::auth::AuthResponse,
            crate::handlers::auth::UserResponse,
            crate::handlers::auth::GoogleAuthQuery,
            crate::handlers::auth::GoogleCallbackQuery,
            crate::handlers::auth::GoogleAuthUrlResponse,
            crate::handlers::auth::UpdateWalletRequest,

            // API Key schemas
            crate::handlers::api_key::CreateApiKeyRequest,
            crate::handlers::api_key::UpdateApiKeyRequest,
            crate::handlers::api_key::ApiKeyResponse,
            crate::handlers::api_key::ApiKeyListResponse,
            crate::handlers::api_key::ApiKeyStats,
            crate::handlers::api_key::ApiKeyFilterQuery,
            avinapi::prelude::PaginationQuery,

            // AI Audit schemas
            crate::handlers::ai_audit::AiAuditRequest,
            crate::handlers::ai_audit::AiAuditResponse,
            crate::handlers::ai_audit::AiAuditReportQuery,
            crate::handlers::ai_audit::AiAuditReport,
            crate::handlers::ai_audit::AiAuditResult,

            // Admin schemas
            crate::handlers::admin::CreateUserRequest,
            crate::handlers::admin::UpdateUserRequest,
            crate::handlers::admin::AdminUserQuery,
            crate::handlers::admin::UserListResponse,
            crate::handlers::admin::RolesPermissionsResponse,
            crate::handlers::admin::RoleInfo,
            crate::handlers::admin::PermissionInfo,
            crate::handlers::admin::PermissionCheckResponse,

            // Common schemas
            crate::models::user::User,
            crate::models::user::UserRole,
            crate::models::user::UserStatus,
            crate::models::api_key::ApiKey,
            crate::handlers::ai_audit::AiAudit,
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "System", description = "System health check and status endpoints"),
        (name = "Auth", description = "User authentication and registration"),
        (name = "User", description = "User profile and settings management"),
        (name = "API Keys", description = "API key management for programmatic access"),
        (name = "AI Audit", description = "AI algorithm audit services"),
        (name = "Admin", description = "Administrative functions (requires admin role)"),
        (name = "Proxy", description = "Proxy endpoints to Secure TEE service"),
    ),
    external_docs(
        url = "https://docs.delong.protocol",
        description = "Full documentation for DeLong Protocol"
    )
)]
pub struct ApiDoc;

/// Security configuration for OpenAPI
pub struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            // JWT Bearer authentication
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some(
                            "JWT Bearer token authentication. \
                             Obtain token via /auth/login or /auth/register endpoints.",
                        ))
                        .build(),
                ),
            );

            // API Key authentication
            components.add_security_scheme(
                "api_key",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("X-API-Key"))),
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
    /// Custom server URL
    pub server_url: Option<String>,
}

impl Default for ApiDocConfig {
    fn default() -> Self {
        Self {
            scalar_enabled: true,
            title: None,
            version: None,
            server_url: None,
        }
    }
}

/// Create API documentation routers based on configuration
pub fn create_api_docs(config: ApiDocConfig) -> axum::Router {
    use axum::Router;
    use axum::response::Html;
    use axum::routing::get;

    let mut router = Router::new();

    // Customize OpenAPI spec if needed
    let mut openapi = ApiDoc::openapi();

    if let Some(title) = config.title {
        openapi.info.title = title.into();
    }

    if let Some(version) = config.version {
        openapi.info.version = version.into();
    }

    if let Some(server_url) = config.server_url {
        openapi.servers = Some(vec![utoipa::openapi::Server::new(server_url)]);
    }

    // Add OpenAPI spec endpoint
    let spec = openapi.clone();
    router = router.route(
        "/api-docs/openapi.json",
        get(move || async move { axum::Json(spec.clone()) }),
    );

    // Add Scalar UI if enabled
    if config.scalar_enabled {
        let spec_for_scalar = openapi.clone();
        router = router.route(
            "/scalar",
            get(move || async move {
                let html =
                    Scalar::new(serde_json::to_value(spec_for_scalar.clone()).unwrap()).to_html();
                Html(html)
            }),
        );
    }

    // Add a documentation index page
    router = router.route(
        "/docs",
        get(|| async {
            Html(r#"<!DOCTYPE html>
            <html lang="en">
            <head>
                <meta charset="UTF-8">
                <meta name="viewport" content="width=device-width, initial-scale=1.0">
                <title>DeLong Protocol API Documentation</title>
                <style>
                    body {
                        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
                        max-width: 800px;
                        margin: 50px auto;
                        padding: 20px;
                        background: #f5f5f5;
                    }
                    .container {
                        background: white;
                        border-radius: 8px;
                        padding: 30px;
                        box-shadow: 0 2px 10px rgba(0,0,0,0.1);
                    }
                    h1 {
                        color: #333;
                        border-bottom: 2px solid #4a90e2;
                        padding-bottom: 10px;
                    }
                    .links {
                        margin-top: 30px;
                    }
                    .link-card {
                        display: block;
                        padding: 20px;
                        margin: 15px 0;
                        background: #f8f9fa;
                        border-radius: 6px;
                        text-decoration: none;
                        color: #333;
                        border: 1px solid #dee2e6;
                        transition: all 0.3s;
                    }
                    .link-card:hover {
                        background: #e9ecef;
                        border-color: #4a90e2;
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(0,0,0,0.1);
                    }
                    .link-card h3 {
                        margin: 0 0 8px 0;
                        color: #4a90e2;
                    }
                    .link-card p {
                        margin: 0;
                        color: #666;
                        font-size: 14px;
                    }
                    .quickstart {
                        margin-top: 40px;
                        padding-top: 20px;
                        border-top: 1px solid #dee2e6;
                    }
                    .quickstart h2 {
                        color: #333;
                        font-size: 1.3em;
                    }
                    code {
                        background: #f1f3f4;
                        padding: 2px 6px;
                        border-radius: 3px;
                        font-family: 'Monaco', 'Menlo', monospace;
                        font-size: 0.9em;
                    }
                </style>
            </head>
            <body>
                <div class="container">
                    <h1>🚀 DeLong Protocol API Documentation</h1>
                    <p>Welcome to the DeLong Protocol Core API documentation. Choose your preferred documentation interface:</p>

                    <div class="links">
                        <a href="/scalar" class="link-card">
                            <h3>📘 Scalar UI</h3>
                            <p>Modern, interactive API documentation with a clean interface</p>
                        </a>

                        <a href="/api-docs/openapi.json" class="link-card">
                            <h3>📄 OpenAPI Specification</h3>
                            <p>Raw OpenAPI 3.0 specification in JSON format</p>
                        </a>
                    </div>

                    <div class="quickstart">
                        <h2>Quick Start</h2>
                        <p>To get started with the API:</p>
                        <ol>
                            <li>Register a new account via <code>POST /auth/register</code></li>
                            <li>Login to get a JWT token via <code>POST /auth/login</code></li>
                            <li>Include the token in the <code>Authorization: Bearer &lt;token&gt;</code> header</li>
                            <li>Or create an API key and use it in the <code>X-API-Key</code> header</li>
                        </ol>
                    </div>
                </div>
            </body>
            </html>"#)
        }),
    );

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
        assert!(config.server_url.is_none());
    }

    #[test]
    fn test_openapi_has_security_schemes() {
        let openapi = ApiDoc::openapi();
        assert!(openapi.components.is_some());

        if let Some(components) = openapi.components {
            let schemes = &components.security_schemes;
            assert!(schemes.contains_key("bearer_auth"));
            assert!(schemes.contains_key("api_key"));
        }
    }
}
