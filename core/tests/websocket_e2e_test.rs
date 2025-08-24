//! End-to-End WebSocket Proxy Tests
//!
//! These tests verify the complete WebSocket proxy functionality
//! by connecting through the Core service to a real Secure service.

#[cfg(test)]
mod tests {
    use delong_core::{Config, create_app_state, routes::create_router};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::json;
    use std::time::Duration;
    use tokio::time::timeout;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tracing::info;

    /// Initialize test logging
    fn init_logging() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("info")
            .with_test_writer()
            .try_init();
    }

    /// Helper to create test configuration
    fn create_test_config() -> Config {
        // Load configuration directly from .env file
        Config::load().expect("Failed to load test configuration")
    }

    /// Create a test user and get a real JWT token
    async fn create_test_user_and_login(base_url: &str) -> String {
        use serde_json::json;

        // Generate unique test email
        let test_id = uuid::Uuid::new_v4();
        let email = format!("test-{}@example.com", test_id);
        let username = format!("test_{}", &test_id.to_string()[..8]);
        let password = "TestPassword123!";

        let client = reqwest::Client::new();

        // Add delay to avoid rate limiting
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Step 1: Send verification code (in test mode, this generates code "1234")
        let send_code_response = client
            .post(format!("{}/auth/send-code", base_url))
            .json(&json!({
                "email": email,
                "verification_type": "email",
                "language": "en"
            }))
            .send()
            .await
            .expect("Failed to send verification code");

        if !send_code_response.status().is_success() {
            panic!(
                "Failed to send verification code: {:?}",
                send_code_response.text().await
            );
        }

        // Delay between verification and registration to avoid rate limiting
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Step 2: Register with the fixed test verification code "1234"
        let register_response = client
            .post(format!("{}/auth/register", base_url))
            .json(&json!({
                "email": email,
                "username": username,
                "password": password,
                "verification_code": "1234"  // Fixed code in test mode
            }))
            .send()
            .await
            .expect("Failed to register user");

        if !register_response.status().is_success() {
            panic!(
                "Failed to register user: {:?}",
                register_response.text().await
            );
        }

        // Delay before login to avoid rate limiting
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Step 3: Login to get JWT token
        let login_response = client
            .post(format!("{}/auth/login", base_url))
            .json(&json!({
                "email": email,
                "password": password
            }))
            .send()
            .await
            .expect("Failed to login");

        if !login_response.status().is_success() {
            panic!("Failed to login: {:?}", login_response.text().await);
        }

        let login_data: serde_json::Value = login_response
            .json()
            .await
            .expect("Failed to parse login response");

        // Debug: Print the actual login response structure
        eprintln!(
            "Login response: {}",
            serde_json::to_string_pretty(&login_data).unwrap()
        );

        // The response might be wrapped in a success envelope
        // Try to get access_token from the root or from a data/payload field
        let access_token = login_data["access_token"]
            .as_str()
            .or_else(|| login_data["data"]["access_token"].as_str())
            .or_else(|| login_data["payload"]["access_token"].as_str())
            .expect("No access token in response")
            .to_string();

        eprintln!("Got access token: {}", access_token);
        access_token
    }

    /// Check if Secure service is available
    async fn check_secure_service() -> bool {
        // Try to connect to the Secure service port
        match tokio::net::TcpStream::connect("127.0.0.1:11000").await {
            Ok(_) => {
                // Port is open, service is likely running
                // Give it a moment to fully initialize
                tokio::time::sleep(Duration::from_millis(500)).await;
                true
            }
            Err(_) => false,
        }
    }

    /// Start Core service for testing
    async fn start_test_server() -> (tokio::task::JoinHandle<()>, u16) {
        let config = create_test_config();

        let app_state = create_app_state(config)
            .await
            .expect("Failed to create app state");

        let app = create_router(app_state);

        // Bind to port 0 to get an available port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind to port");

        // Get the actual port that was bound
        let port = listener.local_addr().unwrap().port();

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("Core service failed");
        });

        // Give the service time to start
        tokio::time::sleep(Duration::from_millis(500)).await;

        (handle, port)
    }

    #[tokio::test]
    #[ignore = "Requires Secure service to be running"]
    async fn test_websocket_proxy_basic() {
        init_logging();

        // Check if Secure service is available
        if !check_secure_service().await {
            eprintln!(
                "Secure service not available at localhost:11000. Please start it first with: cd secure && cargo run"
            );
            panic!("Secure service is required for this test");
        }

        // Start Core service
        let (_handle, port) = start_test_server().await;

        // Create a real user and get JWT token
        let base_url = format!("http://localhost:{}", port);
        let token = create_test_user_and_login(&base_url).await;

        // Connect to WebSocket through Core proxy
        let ws_url = format!("ws://localhost:{}/api/ws?task_id=test-123", port);
        let mut request = ws_url.into_client_request().unwrap();
        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {}", token).parse().unwrap(),
        );

        // Connect
        let (ws_stream, _) = timeout(Duration::from_secs(5), connect_async(request))
            .await
            .expect("Connection timeout")
            .expect("Failed to connect");

        let (mut write, mut read) = ws_stream.split();

        info!("WebSocket connection established");

        // Send a test message
        let test_msg = json!({
            "type": "test",
            "data": "Hello from test"
        });

        write
            .send(tokio_tungstenite::tungstenite::Message::Text(
                test_msg.to_string().into(),
            ))
            .await
            .expect("Failed to send message");

        // Wait for any response (connection confirmation from Secure)
        if let Ok(Some(Ok(msg))) = timeout(Duration::from_secs(2), read.next()).await {
            info!("Received: {:?}", msg);
        }

        // Close connection
        write
            .send(tokio_tungstenite::tungstenite::Message::Close(None))
            .await
            .ok();
    }

    #[tokio::test]
    #[ignore = "Requires Secure service to be running"]
    async fn test_websocket_proxy_binary() {
        // Add delay between tests to avoid rate limiting
        tokio::time::sleep(Duration::from_secs(3)).await;

        init_logging();

        if !check_secure_service().await {
            eprintln!(
                "Secure service not available at localhost:11000. Please start it first with: cd secure && cargo run"
            );
            panic!("Secure service is required for this test");
        }

        let (_handle, port) = start_test_server().await;
        let base_url = format!("http://localhost:{}", port);
        let token = create_test_user_and_login(&base_url).await;

        // Connect
        let ws_url = format!("ws://localhost:{}/api/ws?task_id=test-binary", port);
        let mut request = ws_url.into_client_request().unwrap();
        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {}", token).parse().unwrap(),
        );

        let (ws_stream, _) = connect_async(request).await.expect("Failed to connect");

        let (mut write, _read) = ws_stream.split();

        // Send binary data
        let binary_data = vec![1, 2, 3, 4, 5];
        write
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                binary_data.into(),
            ))
            .await
            .expect("Failed to send binary");

        // Close
        write
            .send(tokio_tungstenite::tungstenite::Message::Close(None))
            .await
            .ok();
    }

    #[tokio::test]
    #[ignore = "Requires Secure service to be running"]
    async fn test_websocket_requires_auth() {
        // Add delay between tests to avoid rate limiting
        tokio::time::sleep(Duration::from_secs(3)).await;

        init_logging();

        if !check_secure_service().await {
            eprintln!(
                "Secure service not available at localhost:11000. Please start it first with: cd secure && cargo run"
            );
            panic!("Secure service is required for this test");
        }

        let (_handle, port) = start_test_server().await;

        // Try to connect WITHOUT authentication
        let ws_url = format!("ws://localhost:{}/api/ws?task_id=test-noauth", port);
        let request = ws_url.into_client_request().unwrap();

        // Should fail
        let result = timeout(Duration::from_secs(2), connect_async(request)).await;

        match result {
            Ok(Ok(_)) => panic!("Should not connect without auth"),
            Ok(Err(e)) => info!("Correctly rejected: {}", e),
            Err(_) => info!("Connection timed out (expected)"),
        }
    }

    #[tokio::test]
    #[ignore = "Requires Secure service to be running"]
    async fn test_websocket_multiple_connections() {
        // Add delay between tests to avoid rate limiting
        tokio::time::sleep(Duration::from_secs(3)).await;

        init_logging();

        if !check_secure_service().await {
            eprintln!(
                "Secure service not available at localhost:11000. Please start it first with: cd secure && cargo run"
            );
            panic!("Secure service is required for this test");
        }

        let (_handle, port) = start_test_server().await;
        let base_url = format!("http://localhost:{}", port);
        let token = create_test_user_and_login(&base_url).await;

        // Connect multiple clients
        let mut handles = Vec::new();

        for i in 0..3 {
            let token = token.clone();
            let task = tokio::spawn(async move {
                let ws_url = format!("ws://localhost:{}/api/ws?task_id=test-multi-{}", port, i);
                let mut request = ws_url.into_client_request().unwrap();
                request.headers_mut().insert(
                    "Authorization",
                    format!("Bearer {}", token).parse().unwrap(),
                );

                let (ws_stream, _) = connect_async(request).await.expect("Failed to connect");

                let (mut write, _read) = ws_stream.split();

                // Send a message
                let msg = json!({
                    "client": i,
                    "message": format!("Hello from client {}", i)
                });

                write
                    .send(tokio_tungstenite::tungstenite::Message::Text(
                        msg.to_string().into(),
                    ))
                    .await
                    .expect("Failed to send");

                // Close
                write
                    .send(tokio_tungstenite::tungstenite::Message::Close(None))
                    .await
                    .ok();

                info!("Client {} completed", i);
            });

            handles.push(task);
        }

        // Wait for all clients to complete
        for handle in handles {
            handle.await.expect("Client task failed");
        }

        info!("All clients completed successfully");
    }
}
