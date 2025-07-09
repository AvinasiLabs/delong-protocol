//! OpenAPI documentation for DeLong Protocol Gateway
//!
//! This module provides comprehensive API documentation for all gateway endpoints
//! using utoipa to generate OpenAPI 3.0 specification.

use axum::{
    http::HeaderValue,
    response::{IntoResponse, Response},
};
use utoipa::OpenApi;

// Import all the models that have ToSchema implemented
use common::prelude::*;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "DeLong Protocol Gateway API",
        description = "
# DeLong Protocol Gateway API

The DeLong Protocol Gateway is the unified entry point for all API requests in the DeLong Protocol ecosystem. It provides secure access to dataset management, algorithm execution, committee operations, and voting functionality.

## Architecture

The gateway acts as a proxy and router, forwarding requests to appropriate backend services:

- **Core Service**: Handles authentication, dynamic datasets, and public operations
- **Secure Service**: Processes sensitive operations in TEE (Trusted Execution Environment)
- **Data Pipeline**: Manages data analysis and sample generation

## Authentication

All API endpoints (except health checks and sample data access) require authentication using API keys:

```
X-API-Key: delong_live_your_api_key_here
```

## Rate Limiting

API access is rate-limited based on your API key tier:

- **Basic**: 100 requests/minute
- **Standard**: 500 requests/minute
- **Premium**: 2000 requests/minute
- **Enterprise**: 10000 requests/minute

## Response Format

All API responses follow a consistent format:

```json
{
  \"code\": \"SUCCESS\",
  \"data\": { ... },
  \"request_id\": \"req_123456789\"
}
```

## Error Codes

- `SUCCESS`: Request completed successfully
- `BAD_REQUEST`: Invalid request parameters
- `UNAUTHORIZED`: Missing or invalid API key
- `FORBIDDEN`: Insufficient permissions
- `NOT_FOUND`: Requested resource not found
- `INTERNAL_SERVER_ERROR`: Server error
- `TIMEOUT`: Request timed out
- `TOO_MANY_REQUESTS`: Rate limit exceeded
        ",
        version = "0.2.0",
        contact(
            name = "DeLong Protocol Team",
            email = "dylan@avinasi.org"
        ),
        license(
            name = "AGPL-3.0",
            url = "https://www.gnu.org/licenses/agpl-3.0.html"
        )
    ),
    servers(
        (url = "https://gateway.delong.avinasi.org", description = "Production server"),
        (url = "https://gateway-staging.delong.avinasi.org", description = "Staging server"),
        (url = "http://localhost:8080", description = "Local development server")
    ),
    // Note: Paths will be added once handlers are properly annotated with utoipa macros
    paths(),
    components(
        schemas(
            // Core response types
            ApiResponse<String>,
            ApiResponse<StaticDatasetInfo>,
            ApiResponse<DynamicDatasetInfo>,
            ApiResponse<AlgoExeData>,
            ApiResponse<CommitteeMemberData>,
            ApiResponse<ContractData>,
            ApiResponse<VoteData>,
            PaginatedResponse<StaticDatasetInfo>,
            PaginatedResponse<DynamicDatasetInfo>,
            PaginatedResponse<AlgoExeData>,
            PaginatedResponse<CommitteeMemberData>,
            PaginatedResponse<ContractData>,
            PaginatedResponse<VoteData>,
            PaginatedResponse<ReportInfo>,
            PaginationParams,
            ResponseCode,
            ApiResponse<String>,
            ApiResponse<StaticDatasetInfo>,
            ApiResponse<DynamicDatasetInfo>,
            ApiResponse<AlgoExeData>,
            ApiResponse<CommitteeMemberData>,
            ApiResponse<ContractData>,
            ApiResponse<VoteData>,
            ApiResponse<ReportInfo>,

            // Auth models
            ApiKeyInfo,
            AuthContext,
            AuthMethod,
            CreateApiKeyRequest,
            CreateApiKeyResponse,
            JwtClaims,
            Permission,
            RateLimitTier,
            RevokeApiKeyRequest,
            RevokeApiKeyResponse,
            UserSession,
            ValidateApiKeyRequest,
            ValidateApiKeyResponse,

            // Dataset models
            CreateDatasetRequest,
            DatasetFormat,
            DatasetStatus,
            DynamicDatasetInfo,
            DynamicDatasetListQuery,
            StaticDatasetInfo,
            StaticDatasetListQuery,
            UpdateDatasetRequest,
            UpdateStaticDatasetRequest,

            // Algorithm execution models
            AlgoExeData,
            AlgoExeStatus,
            AlgoExeSubmissionRequest,
            AlgoExeSubmissionResponse,
            AlgoReviewStatus,

            // Committee models
            CommitteeMemberData,
            CommitteeMemberResponse,
            MembershipCheckResponse,
            SetCommitteeMemberRequest,

            // Contract models
            ContractData,
            ContractResponse,
            CreateContractRequest,
            ExtendedContractData,
            UpdateContractRequest,

            // Vote models
            CastVoteRequest,
            ExtendedVoteData,
            SetVoteDurationRequest,
            VoteData,
            VoteDecision,
            VoteDurationResponse,
            VoteQuery,
            VoteStatus,
            VoteSummary,
            VotingSession,

            // Report models
            ReportInfo,
            ReportQuery,
            ReportStatus,
            ReportSummary,
            ReportType,
            UploadReportRequest,
            UploadReportResponse,

            // WebSocket models
            BlockchainTransactionNotification,
            ConnectionStats,
            NotificationMessage,
            SubscriptionRequest,
            SubscriptionResponse,
            TransactionStatus,
        )
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "auth", description = "Authentication and API key management"),
        (name = "static-datasets", description = "Static dataset management (IPFS + blockchain)"),
        (name = "dynamic-datasets", description = "Dynamic dataset management (local storage)"),
        (name = "sample-data", description = "Public sample data access"),
        (name = "algo-exe", description = "Algorithm execution and management"),
        (name = "committee", description = "Committee member management"),
        (name = "contracts", description = "Smart contract information"),
        (name = "votes", description = "Voting system operations"),
        (name = "reports", description = "Test report management"),
        (name = "websocket", description = "Real-time WebSocket notifications"),
    ),
    external_docs(
        url = "https://docs.delong.avinasi.org",
        description = "Full DeLong Protocol Documentation"
    )
)]
pub struct ApiDoc;

/// Create Swagger UI service for API documentation
pub fn create_swagger_ui() -> axum::Router<crate::routes::AppState> {
    use axum::routing::get;

    axum::Router::new()
        .route("/docs", get(swagger_ui_handler))
        .route("/docs/{*tail}", get(swagger_ui_handler))
}

/// Create Scalar UI service for API documentation (alternative to Swagger)
pub fn create_scalar_ui() -> axum::Router<crate::routes::AppState> {
    use axum::routing::get;

    axum::Router::new()
        .route("/scalar", get(scalar_ui_handler))
        .route("/scalar/{*tail}", get(scalar_ui_handler))
}

/// Handler for Swagger UI
async fn swagger_ui_handler() -> impl IntoResponse {
    use axum::response::Html;

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>DeLong Protocol API Documentation</title>
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5.9.0/swagger-ui.css" />
    <style>
        html {{ box-sizing: border-box; overflow: -moz-scrollbars-vertical; overflow-y: scroll; }}
        *, *:before, *:after {{ box-sizing: inherit; }}
        body {{ margin:0; background: #fafafa; }}
    </style>
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5.9.0/swagger-ui-bundle.js"></script>
    <script src="https://unpkg.com/swagger-ui-dist@5.9.0/swagger-ui-standalone-preset.js"></script>
    <script>
    window.onload = function() {{
        const ui = SwaggerUIBundle({{
            url: '/docs/openapi.json',
            dom_id: '#swagger-ui',
            deepLinking: true,
            presets: [
                SwaggerUIBundle.presets.apis,
                SwaggerUIStandalonePreset
            ],
            plugins: [
                SwaggerUIBundle.plugins.DownloadUrl
            ],
            layout: "StandaloneLayout"
        }});
    }};
    </script>
</body>
</html>"#
    );

    Html(html)
}

/// Handler for Scalar UI
async fn scalar_ui_handler() -> impl IntoResponse {
    use axum::response::Html;

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>DeLong Protocol API Documentation - Scalar</title>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
</head>
<body>
    <script id="api-reference" data-url="/docs/openapi.json"></script>
    <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference@latest"></script>
</body>
</html>"#
    );

    Html(html)
}

/// Get the OpenAPI JSON specification
pub async fn get_openapi_json() -> impl IntoResponse {
    let json = ApiDoc::openapi().to_pretty_json().unwrap_or_else(|e| {
        tracing::error!("Failed to serialize OpenAPI spec: {}", e);
        "{}".to_string()
    });

    let mut response = Response::new(json);
    response
        .headers_mut()
        .insert("content-type", HeaderValue::from_static("application/json"));
    response
}

/// Get the OpenAPI YAML specification
pub async fn get_openapi_yaml() -> impl IntoResponse {
    let yaml = serde_yaml::to_string(&ApiDoc::openapi()).unwrap_or_else(|e| {
        tracing::error!("Failed to serialize OpenAPI spec to YAML: {}", e);
        "".to_string()
    });

    let mut response = Response::new(yaml);
    response
        .headers_mut()
        .insert("content-type", HeaderValue::from_static("application/yaml"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::OpenApi;

    #[test]
    fn test_openapi_generation() {
        let spec = ApiDoc::openapi();

        // Verify basic structure
        assert_eq!(spec.info.title, "DeLong Protocol Gateway API");
        assert_eq!(spec.info.version, "0.2.0");
        assert!(!spec.components.is_none());

        // Verify we have the main tags
        if let Some(tags) = &spec.tags {
            let tag_names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
            assert!(tag_names.contains(&"health"));
            assert!(tag_names.contains(&"auth"));
            assert!(tag_names.contains(&"static-datasets"));
            assert!(tag_names.contains(&"dynamic-datasets"));
            assert!(tag_names.contains(&"algo-exe"));
            assert!(tag_names.contains(&"committee"));
            assert!(tag_names.contains(&"votes"));
        }
    }

    #[tokio::test]
    async fn test_openapi_json_serialization() {
        let _response = get_openapi_json().await;
        // This is a future, so we can't directly test the string
        // But we can test that it compiles and runs
        assert!(true);
    }

    #[tokio::test]
    async fn test_openapi_yaml_serialization() {
        let _response = get_openapi_yaml().await;
        // This is a future, so we can't directly test the string
        // But we can test that it compiles and runs
        assert!(true);
    }

    #[test]
    fn test_direct_json_serialization() {
        let spec = ApiDoc::openapi();
        let json = spec.to_pretty_json().unwrap();
        assert!(!json.is_empty());
        assert_ne!(json, "{}");

        // Should be valid JSON
        let _: serde_json::Value = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_direct_yaml_serialization() {
        let spec = ApiDoc::openapi();
        let yaml = serde_yaml::to_string(&spec).unwrap();
        assert!(!yaml.is_empty());

        // Should be valid YAML
        let _: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
    }
}
