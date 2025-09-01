//! Proxy handler for forwarding requests to Secure service
//!
//! This module implements a generic proxy handler that forwards all requests
//! matching certain path prefixes to the Secure service, similar to how Nginx
//! handles proxy_pass directives.

use axum::{
    Json,
    body::{Body, Bytes},
    extract::{
        OriginalUri, Query, State,
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, Method, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::{
    AppState,
    infra::proxy::{ProxyClient, client::AuthContext as ProxyAuthContext},
    middleware::auth::{AuthMethod, AuthUser},
};
use avinapi::prelude::*;

/// Generic proxy handler that forwards all requests to Secure service
///
/// This handler acts like Nginx's proxy_pass, automatically forwarding
/// any request that matches the configured path prefix to the Secure service.
///
/// ## Example Usage
///
/// ```rust,ignore
/// let app = Router::new()
///     // Forward all /api/datasets/* requests
///     .nest("/api/datasets", proxy_routes())
///     // Forward all /api/algorithms/* requests
///     .nest("/api/algorithms", proxy_routes())
/// ```
pub async fn proxy_handler(
    State(state): State<AppState>,
    auth_user: AuthUser,
    OriginalUri(original_uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Response> {
    // Check if this is a WebSocket upgrade request
    if headers.get(header::UPGRADE).and_then(|v| v.to_str().ok()) == Some("websocket") {
        // WebSocket requests should be handled by the dedicated WebSocket handler
        return Err(AppError::Validation(
            "WebSocket upgrades should use the /ws endpoint".to_string(),
        ));
    }

    // Extract the proxy client
    let proxy_client = state
        .proxy_client
        .as_ref()
        .ok_or_else(|| AppError::Config("Proxy service not configured".to_string()))?;

    // OriginalUri contains the full original path including /api prefix
    let path = original_uri.path();

    info!(
        "Proxying {} request to path: {} for user: {} using {:?}",
        method,
        path,
        auth_user.user_id(),
        auth_user.auth_method
    );

    // Build authentication context
    let auth_context = ProxyAuthContext {
        user_id: auth_user.user_id().to_string(),
        email: auth_user.email().to_string(),
        auth_method: match auth_user.auth_method {
            AuthMethod::Jwt => "jwt".to_string(),
            AuthMethod::ApiKey => "api_key".to_string(),
        },
        tenant_id: None,
        scopes: if auth_user.user.email_verified.unwrap_or(false) {
            vec!["read".to_string(), "write".to_string()]
        } else {
            vec!["read".to_string()]
        },
        client_ip: headers
            .get("x-forwarded-for")
            .or_else(|| headers.get("x-real-ip"))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string()),
        request_id: headers
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
    };

    // Forward the request using the proxy client
    // Only include body for methods that typically have a body
    let request_body = match method {
        Method::GET | Method::HEAD | Method::DELETE | Method::OPTIONS => {
            // These methods should NOT have a body according to HTTP spec
            // Even if a body is present, we don't forward it to avoid content-length issues
            None
        }
        _ => {
            // POST, PUT, PATCH, etc. - include body even if empty
            Some(body)
        }
    };

    proxy_client
        .forward_request(method, path, headers, request_body, auth_context)
        .await
}

/// Extract authentication method from request
///
/// This middleware determines whether the request is authenticated via JWT or API key
/// and stores this information in the request extensions.
pub async fn extract_auth_method(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let auth_method = if req.headers().contains_key(header::AUTHORIZATION) {
        "jwt"
    } else if req.headers().contains_key("x-api-key") {
        "api_key"
    } else {
        "unknown"
    };

    let mut req = req;
    req.extensions_mut().insert(auth_method.to_string());

    Ok(next.run(req).await)
}

/// Generate Secure proxy routes
///
/// This creates a router that forwards authenticated requests to the Secure service.
/// The router handles both regular HTTP requests and WebSocket connections.
///
/// ## Architecture
///
/// ```text
/// Client Request
///       ↓
/// Core Auth Layer (JWT/API Key)
///       ↓
/// Proxy Handler
///       ↓
/// Internal JWT Generation
///       ↓
/// Secure Service
/// ```
///
/// ## Request Flow
///
/// 1. **Authentication**: Verify client JWT or API key
/// 2. **Context Building**: Create auth context with user info and permissions
/// 3. **Internal JWT**: Generate short-lived token for service-to-service auth
/// 4. **Request Forwarding**: Forward to Secure with internal JWT
/// 5. **Response Handling**: Return Secure's response to client
///
/// ## Security Features
///
/// - **Double Authentication**: Client auth + internal JWT
/// - **Request Signing**: SHA256 digest of request body
/// - **Short-lived Tokens**: Internal JWTs expire in 60 seconds
/// - **Permission Scoping**: Auth context includes user permissions
///
/// ## Example Usage
///
/// ```rust,ignore
/// let app = Router::new()
///     .nest("/api/datasets", proxy_routes())
///     .nest("/api/algorithms", proxy_routes())
///     .nest("/api/tasks", proxy_routes())
/// ```
pub fn proxy_routes() -> axum::Router<AppState> {
    use axum::routing::{Router, any};

    Router::new()
        // Catch all methods and paths for HTTP
        .route("/{*path}", any(proxy_handler))
        // Also handle the root path
        .route("/", any(proxy_handler))
}

/// WebSocket query parameters
#[derive(Debug, Deserialize)]
pub struct WsQuery {
    /// Task ID for this WebSocket connection
    pub task_id: String,
}

/// Handle WebSocket proxy connections
pub async fn websocket_proxy_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsQuery>,
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> impl IntoResponse {
    info!(
        "WebSocket proxy request for task_id: {} from user: {} via {:?}",
        params.task_id,
        auth_user.user_id(),
        auth_user.auth_method
    );

    // Check if proxy client is available
    let proxy_client = match &state.proxy_client {
        Some(client) => client.clone(),
        None => {
            error!("WebSocket proxy requested but proxy client not configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Proxy service not available",
            )
                .into_response();
        }
    };

    // Build proxy auth context from our auth context
    let proxy_auth_context = ProxyAuthContext {
        user_id: auth_user.user_id().to_string(),
        email: auth_user.email().to_string(),
        auth_method: match auth_user.auth_method {
            AuthMethod::Jwt => "jwt".to_string(),
            AuthMethod::ApiKey => "api_key".to_string(),
        },
        tenant_id: None,
        scopes: vec!["read".to_string(), "write".to_string()],
        client_ip: None,
        request_id: format!("ws-{}", Uuid::new_v4()),
    };

    // Accept the WebSocket upgrade
    ws.on_upgrade(move |socket| {
        handle_websocket_proxy(socket, params.task_id, proxy_client, proxy_auth_context)
    })
}

/// Handle the WebSocket proxy connection
async fn handle_websocket_proxy(
    client_socket: WebSocket,
    task_id: String,
    proxy_client: std::sync::Arc<ProxyClient>,
    _auth_context: ProxyAuthContext,
) {
    info!("Starting WebSocket proxy for task_id: {}", task_id);

    // Connect to the Secure service WebSocket
    let secure_base_url = &proxy_client.config.secure_service_url;
    let secure_ws_url = format!(
        "{}/ws?task_id={}",
        secure_base_url
            .replace("http://", "ws://")
            .replace("https://", "wss://"),
        task_id
    );

    // Create WebSocket client to Secure service
    let secure_ws = match tokio_tungstenite::connect_async(&secure_ws_url).await {
        Ok((ws_stream, _)) => ws_stream,
        Err(e) => {
            error!("Failed to connect to Secure WebSocket: {}", e);
            // Send close frame to client
            let mut client_socket = client_socket;
            let _ = client_socket
                .send(WsMessage::Close(Some(axum::extract::ws::CloseFrame {
                    code: 1011,
                    reason: "Failed to connect to backend service".into(),
                })))
                .await;
            return;
        }
    };

    // Split both WebSocket connections
    let (mut client_sender, mut client_receiver) = client_socket.split();
    let (mut secure_sender, mut secure_receiver) = secure_ws.split();

    // Forward messages from client to secure service
    let task_id_for_client = task_id.clone();
    let client_to_secure = tokio::spawn(async move {
        while let Some(msg) = client_receiver.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    let text_str = text.to_string();
                    debug!(
                        "Forwarding text message from client: {} bytes",
                        text_str.len()
                    );
                    if let Err(e) = secure_sender
                        .send(tokio_tungstenite::tungstenite::Message::Text(
                            text_str.into(),
                        ))
                        .await
                    {
                        error!("Failed to forward text to secure: {}", e);
                        break;
                    }
                }
                Ok(WsMessage::Binary(data)) => {
                    debug!(
                        "Forwarding binary message from client: {} bytes",
                        data.len()
                    );
                    if let Err(e) = secure_sender
                        .send(tokio_tungstenite::tungstenite::Message::Binary(
                            data.to_vec().into(),
                        ))
                        .await
                    {
                        error!("Failed to forward binary to secure: {}", e);
                        break;
                    }
                }
                Ok(WsMessage::Ping(data)) => {
                    if let Err(e) = secure_sender
                        .send(tokio_tungstenite::tungstenite::Message::Ping(
                            data.to_vec().into(),
                        ))
                        .await
                    {
                        error!("Failed to forward ping: {}", e);
                        break;
                    }
                }
                Ok(WsMessage::Pong(data)) => {
                    if let Err(e) = secure_sender
                        .send(tokio_tungstenite::tungstenite::Message::Pong(
                            data.to_vec().into(),
                        ))
                        .await
                    {
                        error!("Failed to forward pong: {}", e);
                        break;
                    }
                }
                Ok(WsMessage::Close(_)) => {
                    info!(
                        "Client closing WebSocket for task_id: {}",
                        task_id_for_client
                    );
                    let _ = secure_sender
                        .send(tokio_tungstenite::tungstenite::Message::Close(None))
                        .await;
                    break;
                }
                Err(e) => {
                    error!("Client WebSocket error: {}", e);
                    break;
                }
            }
        }
    });

    // Forward messages from secure service to client
    let task_id_clone = task_id.clone();
    let secure_to_client = tokio::spawn(async move {
        while let Some(msg) = secure_receiver.next().await {
            match msg {
                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                    debug!("Forwarding text message from secure: {} bytes", text.len());
                    if let Err(e) = client_sender
                        .send(WsMessage::Text(text.to_string().into()))
                        .await
                    {
                        error!("Failed to forward text to client: {}", e);
                        break;
                    }
                }
                Ok(tokio_tungstenite::tungstenite::Message::Binary(data)) => {
                    debug!(
                        "Forwarding binary message from secure: {} bytes",
                        data.len()
                    );
                    if let Err(e) = client_sender
                        .send(WsMessage::Binary(axum::body::Bytes::from(data)))
                        .await
                    {
                        error!("Failed to forward binary to client: {}", e);
                        break;
                    }
                }
                Ok(tokio_tungstenite::tungstenite::Message::Ping(data)) => {
                    if let Err(e) = client_sender
                        .send(WsMessage::Ping(axum::body::Bytes::from(data)))
                        .await
                    {
                        error!("Failed to forward ping: {}", e);
                        break;
                    }
                }
                Ok(tokio_tungstenite::tungstenite::Message::Pong(data)) => {
                    if let Err(e) = client_sender
                        .send(WsMessage::Pong(axum::body::Bytes::from(data)))
                        .await
                    {
                        error!("Failed to forward pong: {}", e);
                        break;
                    }
                }
                Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => {
                    info!(
                        "Secure service closing WebSocket for task_id: {}",
                        task_id_clone
                    );
                    let _ = client_sender.send(WsMessage::Close(None)).await;
                    break;
                }
                Ok(tokio_tungstenite::tungstenite::Message::Frame(_)) => {
                    // Frame messages are handled internally by tungstenite
                }
                Err(e) => {
                    error!("Secure WebSocket error: {}", e);
                    break;
                }
            }
        }
    });

    // Wait for either forwarding task to complete
    tokio::select! {
        _ = client_to_secure => {
            debug!("Client to secure forwarding completed for task_id: {}", task_id);
        }
        _ = secure_to_client => {
            debug!("Secure to client forwarding completed for task_id: {}", task_id);
        }
    }

    info!("WebSocket proxy closed for task_id: {}", task_id);
}

/// Health check endpoint for Secure service
pub async fn secure_health_check(
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let proxy_client = state
        .proxy_client
        .as_ref()
        .ok_or_else(|| AppError::Config("Proxy service not configured".to_string()))?;

    match proxy_client.health_check().await {
        Ok(true) => Ok(Json(json!({
            "status": "healthy",
            "service": "secure",
            "available": true
        }))),
        Ok(false) => Ok(Json(json!({
            "status": "unhealthy",
            "service": "secure",
            "available": false
        }))),
        Err(e) => Err(e),
    }
}
