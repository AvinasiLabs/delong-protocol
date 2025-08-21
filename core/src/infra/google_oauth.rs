//! Google OAuth service
//!
//! This module handles Google OAuth authentication flow,
//! including URL generation, token exchange, and user info retrieval.

use avinapi::prelude::AppError;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, warn};

/// Google OAuth service
#[derive(Clone)]
pub struct GoogleOAuthService {
    client: Client,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

/// Google OAuth token response
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: String,
    pub id_token: Option<String>,
}

/// Google user info response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleUserInfo {
    pub id: String,
    pub email: String,
    pub verified_email: bool,
    pub name: String,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub picture: Option<String>,
    pub locale: Option<String>,
}

/// OAuth state data stored temporarily
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthState {
    pub state: String,
    pub return_to: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl GoogleOAuthService {
    /// Create a new Google OAuth service
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            client: Client::new(),
            client_id,
            client_secret,
            redirect_uri,
        }
    }

    /// Generate Google OAuth authorization URL
    pub fn generate_auth_url(&self, state: &str, return_to: Option<&str>) -> String {
        let params = vec![
            ("client_id", self.client_id.as_str()),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", "openid email profile"),
            ("state", state),
            ("access_type", "offline"),
            ("prompt", "select_account"),
        ];

        // Add return_to to state if provided
        if let Some(_return_to) = return_to {
            // In production, you might want to encode return_to in the state parameter
            // For now, we'll handle it separately
        }

        let query_string: String = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        format!(
            "https://accounts.google.com/o/oauth2/v2/auth?{}",
            query_string
        )
    }

    /// Exchange authorization code for access token
    pub async fn exchange_code_for_token(&self, code: &str) -> Result<TokenResponse, AppError> {
        info!("Exchanging authorization code for token");

        let params = [
            ("code", code),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("redirect_uri", &self.redirect_uri),
            ("grant_type", "authorization_code"),
        ];

        let response = self
            .client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send token request: {}", e);
                AppError::Internal(format!("Failed to exchange code: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "Token exchange failed with status {}: {}",
                status, error_text
            );
            return Err(AppError::Internal(format!(
                "Token exchange failed: {}",
                error_text
            )));
        }

        let token_response = response.json::<TokenResponse>().await.map_err(|e| {
            error!("Failed to parse token response: {}", e);
            AppError::Internal(format!("Invalid token response: {}", e))
        })?;

        info!("Successfully exchanged code for token");
        Ok(token_response)
    }

    /// Get user info from Google using access token
    pub async fn get_user_info(&self, access_token: &str) -> Result<GoogleUserInfo, AppError> {
        info!("Fetching user info from Google");

        let response = self
            .client
            .get("https://www.googleapis.com/oauth2/v2/userinfo")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to fetch user info: {}", e);
                AppError::Internal(format!("Failed to fetch user info: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "User info request failed with status {}: {}",
                status, error_text
            );
            return Err(AppError::Internal(format!(
                "Failed to get user info: {}",
                error_text
            )));
        }

        let user_info = response.json::<GoogleUserInfo>().await.map_err(|e| {
            error!("Failed to parse user info: {}", e);
            AppError::Internal(format!("Invalid user info response: {}", e))
        })?;

        info!("Successfully fetched user info for: {}", user_info.email);
        Ok(user_info)
    }

    /// Verify ID token (optional, for additional security)
    pub async fn verify_id_token(
        &self,
        id_token: &str,
    ) -> Result<HashMap<String, serde_json::Value>, AppError> {
        // This is a simplified version. In production, you should properly verify the JWT
        // by checking the signature, issuer, audience, and expiry

        // For now, we'll just decode the payload (middle part of JWT)
        let parts: Vec<&str> = id_token.split('.').collect();
        if parts.len() != 3 {
            return Err(AppError::Validation("Invalid ID token format".to_string()));
        }

        let payload = parts[1];
        use base64::{Engine as _, engine::general_purpose};
        let decoded = general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|e| {
                error!("Failed to decode ID token: {}", e);
                AppError::Validation(format!("Invalid ID token: {}", e))
            })?;

        let claims: HashMap<String, serde_json::Value> =
            serde_json::from_slice(&decoded).map_err(|e| {
                error!("Failed to parse ID token claims: {}", e);
                AppError::Validation(format!("Invalid token claims: {}", e))
            })?;

        // Verify basic claims
        if let Some(aud) = claims.get("aud").and_then(|v| v.as_str()) {
            if aud != self.client_id {
                warn!("ID token audience mismatch");
                return Err(AppError::Authentication(
                    "Invalid token audience".to_string(),
                ));
            }
        }

        if let Some(iss) = claims.get("iss").and_then(|v| v.as_str()) {
            if !["accounts.google.com", "https://accounts.google.com"].contains(&iss) {
                warn!("ID token issuer mismatch");
                return Err(AppError::Authentication("Invalid token issuer".to_string()));
            }
        }

        Ok(claims)
    }
}

/// Generate a secure state parameter for OAuth
pub fn generate_state_parameter() -> String {
    use rand::{Rng, distributions::Alphanumeric};

    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

/// Store OAuth state in Redis or memory cache
pub struct OAuthStateStore {
    // In production, this should use Redis
    // For now, we'll use a simple in-memory store via the app state
}

impl OAuthStateStore {
    /// Store OAuth state
    pub async fn store_state(
        redis_pool: &deadpool_redis::Pool,
        state: &str,
        return_to: Option<String>,
    ) -> Result<(), AppError> {
        let mut conn = redis_pool.get().await.map_err(|e| {
            error!("Failed to get Redis connection: {}", e);
            AppError::Internal(format!("Redis connection error: {}", e))
        })?;

        let state_data = OAuthState {
            state: state.to_string(),
            return_to,
            created_at: Utc::now(),
        };

        let key = format!("oauth:state:{}", state);
        let value = serde_json::to_string(&state_data).map_err(|e| {
            error!("Failed to serialize state data: {}", e);
            AppError::Internal(format!("Serialization error: {}", e))
        })?;

        // Store with 10 minute expiry
        use deadpool_redis::redis::AsyncCommands;
        let _: () = conn.set_ex(&key, &value, 600).await.map_err(|e| {
            error!("Failed to store OAuth state: {}", e);
            AppError::Internal(format!("Failed to store state: {}", e))
        })?;

        Ok(())
    }

    /// Retrieve and delete OAuth state
    pub async fn retrieve_state(
        redis_pool: &deadpool_redis::Pool,
        state: &str,
    ) -> Result<OAuthState, AppError> {
        let mut conn = redis_pool.get().await.map_err(|e| {
            error!("Failed to get Redis connection: {}", e);
            AppError::Internal(format!("Redis connection error: {}", e))
        })?;

        let key = format!("oauth:state:{}", state);

        // Get and delete in one operation
        use deadpool_redis::redis::AsyncCommands;
        let value: Option<String> = conn.get_del(&key).await.map_err(|e| {
            error!("Failed to retrieve OAuth state: {}", e);
            AppError::Internal(format!("Failed to retrieve state: {}", e))
        })?;

        let value = value.ok_or_else(|| {
            warn!("OAuth state not found or expired: {}", state);
            AppError::Authentication("Invalid or expired state parameter".to_string())
        })?;

        let state_data: OAuthState = serde_json::from_str(&value).map_err(|e| {
            error!("Failed to deserialize state data: {}", e);
            AppError::Internal(format!("Invalid state data: {}", e))
        })?;

        // Check if state is not too old (additional safety check)
        let age = Utc::now().signed_duration_since(state_data.created_at);
        if age.num_minutes() > 10 {
            warn!("OAuth state too old: {} minutes", age.num_minutes());
            return Err(AppError::Authentication(
                "State parameter expired".to_string(),
            ));
        }

        Ok(state_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_auth_url() {
        let service = GoogleOAuthService::new(
            "test_client_id".to_string(),
            "test_secret".to_string(),
            "http://localhost:3000/auth/google/callback".to_string(),
        );

        let url = service.generate_auth_url("test_state", Some("/dashboard"));

        assert!(url.contains("client_id=test_client_id"));
        assert!(url.contains("redirect_uri=http"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("state=test_state"));
        assert!(url.contains("scope="));
    }

    #[test]
    fn test_generate_state_parameter() {
        let state1 = generate_state_parameter();
        let state2 = generate_state_parameter();

        assert_eq!(state1.len(), 32);
        assert_eq!(state2.len(), 32);
        assert_ne!(state1, state2); // Should be different each time
    }
}
