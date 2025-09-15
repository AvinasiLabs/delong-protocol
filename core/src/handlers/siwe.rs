//! SIWE (Sign-In with Ethereum) handlers
//!
//! This module handles wallet linking functionality using SIWE for authenticated users.
//! Wallets are linked to existing accounts, not used as a standalone authentication method.

use avinapi::prelude::{AppError, JsonResult, data, empty};
use axum::extract::State;
use axum_extra::extract::CookieJar;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use siwe::{Message as SiweMessage, VerificationOpts};
use tracing::{error, info, instrument, warn};
use utoipa::ToSchema;
use validator::Validate;

use crate::{middleware::AuthUser, models::user::User, routes::AppState, utils::jwt};

// ===== Request/Response Structures =====

/// Response containing a nonce for SIWE message signing
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NonceResponse {
    pub nonce: String,
    pub expires_at: String,
}

/// Request to link a wallet using SIWE
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct LinkWalletRequest {
    /// The SIWE message as a string
    pub message: String,
    /// The signature of the message
    pub signature: String,
}

/// Response after successfully linking a wallet
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LinkWalletResponse {
    pub wallet_address: String,
    pub linked_at: String,
    pub user: crate::handlers::auth::UserResponse,
}

/// Wallet status response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WalletStatusResponse {
    pub is_linked: bool,
    pub wallet_address: Option<String>,
    pub linked_at: Option<String>,
    pub verified: bool,
}

/// SIWE verification request (for testing/verification without linking)
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct VerifySignatureRequest {
    pub message: String,
    pub signature: String,
}

/// SIWE verification response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VerifySignatureResponse {
    pub valid: bool,
    pub address: Option<String>,
}

// ===== Nonce Management =====

/// Structure stored in Redis for nonce tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SiweNonce {
    pub nonce: String,
    pub expires_at: chrono::DateTime<Utc>,
    pub used: bool,
}

/// Generate a random nonce string
fn generate_nonce() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();

    (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

// ===== Handlers =====

/// Get a nonce for SIWE message signing
///
/// # Details
/// This endpoint generates a cryptographically secure random nonce that must be included
/// in the SIWE message. The nonce expires after 10 minutes and can only be used once.
#[utoipa::path(
    get,
    path = "/auth/siwe/nonce",
    responses(
        (status = 200, description = "Nonce generated successfully", body = NonceResponse),
        (status = 500, description = "Internal server error")
    ),
    tags = ["Authentication", "SIWE"]
)]
#[instrument(skip(state))]
pub async fn get_siwe_nonce(State(state): State<AppState>) -> JsonResult<NonceResponse> {
    let nonce = generate_nonce();
    let expires_at = Utc::now() + Duration::minutes(10);

    // Store nonce in Redis with expiration
    if let Some(ref redis_pool) = state.config.redis_pool {
        let key = format!("siwe:nonce:{}", nonce);
        let nonce_data = SiweNonce {
            nonce: nonce.clone(),
            expires_at,
            used: false,
        };

        match redis_pool.get().await {
            Ok(mut conn) => {
                let value = serde_json::to_string(&nonce_data)
                    .map_err(|e| AppError::Internal(format!("Failed to serialize nonce: {}", e)))?;

                let _: () = deadpool_redis::redis::cmd("SET")
                    .arg(&key)
                    .arg(&value)
                    .arg("EX")
                    .arg(600) // 10 minutes in seconds
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| {
                        error!("Failed to store nonce in Redis: {}", e);
                        AppError::Internal("Failed to generate nonce".to_string())
                    })?;

                info!("Generated nonce: {} (expires at {})", nonce, expires_at);
            }
            Err(e) => {
                error!("Failed to get Redis connection: {}", e);
                return Err(AppError::Internal("Redis connection failed".to_string()));
            }
        }
    } else {
        warn!("Redis not configured, nonce storage skipped (insecure for production)");
    }

    data!(NonceResponse {
        nonce,
        expires_at: expires_at.to_rfc3339(),
    })
}

/// Verify a SIWE signature without linking (for testing/verification)
///
/// # Details
/// This endpoint verifies a SIWE message signature without linking it to any account.
/// Useful for testing wallet signatures or verification flows.
#[utoipa::path(
    post,
    path = "/auth/siwe/verify",
    request_body = VerifySignatureRequest,
    responses(
        (status = 200, description = "Signature verification result", body = VerifySignatureResponse),
    ),
    tags = ["Authentication", "SIWE"]
)]
#[instrument(skip(_state))]
pub async fn verify_signature(
    State(_state): State<AppState>,
    axum::Json(payload): axum::Json<VerifySignatureRequest>,
) -> JsonResult<VerifySignatureResponse> {
    // Parse SIWE message
    let siwe_message = match payload.message.parse::<SiweMessage>() {
        Ok(msg) => msg,
        Err(_e) => {
            return data!(VerifySignatureResponse {
                valid: false,
                address: None,
            });
        }
    };

    // Decode signature (strip 0x prefix if present)
    let signature_str =
        if payload.signature.starts_with("0x") || payload.signature.starts_with("0X") {
            &payload.signature[2..]
        } else {
            &payload.signature
        };
    let signature = match hex::decode(signature_str) {
        Ok(sig) => sig,
        Err(_) => {
            return data!(VerifySignatureResponse {
                valid: false,
                address: None,
            });
        }
    };

    // Verify without strict domain/nonce checking (for testing)
    let verification_opts = VerificationOpts {
        domain: None,
        nonce: None,
        timestamp: Some(time::OffsetDateTime::now_utc()),
    };

    let valid = siwe_message
        .verify(&signature, &verification_opts)
        .await
        .is_ok();
    let address = if valid {
        info!("Signature verified successfully");
        Some(format!("0x{}", hex::encode(siwe_message.address)))
    } else {
        warn!("Signature verification failed");
        None
    };

    data!(VerifySignatureResponse { valid, address })
}

/// Link a wallet to the authenticated user's account
///
/// # Details
/// This endpoint verifies a SIWE message signature and links the wallet address
/// to the currently authenticated user. The user must be logged in via Google OAuth
/// or another provider before linking a wallet.
#[utoipa::path(
    post,
    path = "/api/wallet/link-wallet",
    request_body = LinkWalletRequest,
    responses(
        (status = 200, description = "Wallet linked successfully", body = LinkWalletResponse),
    ),
    security(
        ("bearer_auth" = [])
    ),
    tags = ["Authentication", "SIWE"]
)]
#[instrument(skip(state, cookies))]
pub async fn link_wallet(
    State(state): State<AppState>,
    auth_user: AuthUser,
    cookies: CookieJar,
    axum::Json(payload): axum::Json<LinkWalletRequest>,
) -> JsonResult<LinkWalletResponse> {
    // Parse SIWE message
    let siwe_message = payload
        .message
        .parse::<SiweMessage>()
        .map_err(|e| AppError::Validation(format!("Invalid SIWE message: {}", e)))?;

    // Verify domain matches (extract hostname without port for comparison)
    let expected_domain = std::env::var("SIWE_DOMAIN").unwrap_or_else(|_| "localhost".to_string());

    // Extract hostname from both domains (remove port if present)
    let expected_hostname = expected_domain
        .split(':')
        .next()
        .unwrap_or(&expected_domain);

    // Convert Authority to string and extract hostname
    let actual_domain_str = siwe_message.domain.to_string();
    let actual_hostname = actual_domain_str
        .split(':')
        .next()
        .unwrap_or(&actual_domain_str);

    if actual_hostname != expected_hostname {
        return Err(AppError::Validation(format!(
            "Invalid domain. Expected: {}, got: {}",
            expected_hostname, actual_hostname
        )));
    }

    // Check nonce validity in Redis
    if let Some(ref redis_pool) = state.config.redis_pool {
        let key = format!("siwe:nonce:{}", siwe_message.nonce);

        match redis_pool.get().await {
            Ok(mut conn) => {
                // Get nonce data
                let nonce_str: Option<String> = deadpool_redis::redis::cmd("GET")
                    .arg(&key)
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| {
                        error!("Failed to get nonce from Redis: {}", e);
                        AppError::Internal("Failed to verify nonce".to_string())
                    })?;

                match nonce_str {
                    Some(data_str) => {
                        let nonce_data: SiweNonce =
                            serde_json::from_str(&data_str).map_err(|e| {
                                AppError::Internal(format!("Failed to parse nonce data: {}", e))
                            })?;

                        if nonce_data.used {
                            return Err(AppError::Validation("Nonce already used".to_string()));
                        }

                        if nonce_data.expires_at < Utc::now() {
                            return Err(AppError::Validation("Nonce expired".to_string()));
                        }

                        // Delete nonce to prevent reuse
                        let _: () = deadpool_redis::redis::cmd("DEL")
                            .arg(&key)
                            .query_async(&mut conn)
                            .await
                            .map_err(|e| {
                                error!("Failed to delete nonce: {}", e);
                                AppError::Internal("Failed to invalidate nonce".to_string())
                            })?;
                    }
                    None => {
                        return Err(AppError::Validation("Invalid or expired nonce".to_string()));
                    }
                }
            }
            Err(e) => {
                error!("Failed to get Redis connection: {}", e);
                return Err(AppError::Internal("Redis connection failed".to_string()));
            }
        }
    }

    // Decode and verify signature (strip 0x prefix if present)
    let signature_str =
        if payload.signature.starts_with("0x") || payload.signature.starts_with("0X") {
            &payload.signature[2..]
        } else {
            &payload.signature
        };
    let signature = hex::decode(signature_str)
        .map_err(|e| AppError::Validation(format!("Invalid signature format: {}", e)))?;

    // Verify the signature (use the actual domain from the message for verification)
    let verification_opts = VerificationOpts {
        domain: Some(siwe_message.domain.clone()),
        nonce: Some(siwe_message.nonce.clone()),
        timestamp: Some(time::OffsetDateTime::now_utc()),
    };

    siwe_message
        .verify(&signature, &verification_opts)
        .await
        .map_err(|e| {
            error!("SIWE verification failed: {}", e);
            AppError::Validation("Invalid signature".to_string())
        })?;

    let wallet_address = format!("0x{}", hex::encode(siwe_message.address));

    // Check if wallet is already linked to another account
    if let Ok(Some(existing_user)) = User::find_by_wallet(&state.db, &wallet_address).await {
        if existing_user.id != auth_user.user.id {
            return Err(AppError::Validation(
                "This wallet is already linked to another account".to_string(),
            ));
        }
    }

    // Update user's wallet address
    let updated_user =
        User::update_wallet_address(&state.db, auth_user.user.id, Some(&wallet_address))
            .await
            .map_err(|e| {
                error!("Failed to update wallet address: {}", e);
                AppError::Internal("Failed to link wallet".to_string())
            })?;

    info!(
        "Wallet {} linked to user {}",
        wallet_address, auth_user.user.id
    );

    // Generate new JWT with wallet address
    let new_token = jwt::generate_token(&updated_user, &state.jwt_config).map_err(|e| {
        error!("Failed to generate new token: {}", e);
        AppError::Internal("Failed to generate authentication token".to_string())
    })?;

    // Update cookie with new token
    let _updated_cookies = crate::middleware::auth::set_auth_cookies(cookies, &new_token, "");

    data!(LinkWalletResponse {
        wallet_address,
        linked_at: Utc::now().to_rfc3339(),
        user: updated_user.into(),
    })
}

/// Unlink wallet from the authenticated user's account
///
/// # Details
/// This endpoint removes the wallet address from the user's account.
/// The user must be authenticated to unlink their wallet.
#[utoipa::path(
    post,
    path = "/api/wallet/unlink-wallet",
    responses(
        (status = 200, description = "Wallet unlinked successfully"),
    ),
    security(
        ("bearer_auth" = [])
    ),
    tags = ["Authentication", "SIWE"]
)]
#[instrument(skip(state))]
pub async fn unlink_wallet(State(state): State<AppState>, auth_user: AuthUser) -> JsonResult<()> {
    // Clear user's wallet address
    User::update_wallet_address(&state.db, auth_user.user.id, None)
        .await
        .map_err(|e| {
            error!("Failed to unlink wallet: {}", e);
            AppError::Internal("Failed to unlink wallet".to_string())
        })?;

    info!("Wallet unlinked from user {}", auth_user.user.id);

    empty!()
}

/// Get wallet linking status for the authenticated user
///
/// # Details
/// Returns information about whether the user has a linked wallet,
/// the wallet address if linked, and when it was linked.
#[utoipa::path(
    get,
    path = "/api/wallet/wallet-status",
    responses(
        (status = 200, description = "Wallet status retrieved", body = WalletStatusResponse),
    ),
    security(
        ("bearer_auth" = [])
    ),
    tags = ["Authentication", "SIWE"]
)]
#[instrument(skip(_state))]
pub async fn get_wallet_status(
    State(_state): State<AppState>,
    auth_user: AuthUser,
) -> JsonResult<WalletStatusResponse> {
    data!(WalletStatusResponse {
        is_linked: auth_user.user.wallet_address.is_some(),
        wallet_address: auth_user.user.wallet_address.clone(),
        linked_at: auth_user.user.wallet_connected_at.map(|dt| dt.to_rfc3339()),
        verified: auth_user.user.wallet_address.is_some(), // If we have an address, it's verified
    })
}
