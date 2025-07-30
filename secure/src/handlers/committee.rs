//! Committee management handlers
//!
//! This module provides HTTP handlers for managing committee members,
//! including listing, setting member status, and checking membership.

use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use ethers::types::Address;
use serde::{Deserialize, Serialize};
use std::{str::FromStr, sync::Arc};
use tracing::{info, instrument};

use crate::{
    models::{
        Create,
        blockchain_transaction::{CreateTransaction, EntityType},
        committee::{CommitteeMember, CreateCommitteeMemberRequest},
    },
    routes::AppState,
};
use avinapi::query::pagination::PaginationQuery;
use avinapi::response::JsonResult;

/// Request for setting committee member status
#[derive(Debug, Deserialize)]
pub struct SetCommitteeMemberRequest {
    /// Wallet address of the member
    pub member_wallet: String,
    /// Approval status
    pub is_approved: bool,
}

/// Response for setting committee member
#[derive(Debug, Serialize)]
pub struct SetCommitteeMemberResponse {
    /// Transaction hash
    pub tx_hash: String,
}

/// Query parameters for checking membership
#[derive(Debug, Deserialize)]
pub struct IsMemberQuery {
    /// Wallet address to check
    pub member_wallet: String,
}

/// Set committee member status (requires admin)
#[instrument(skip(state))]
pub async fn set_committee_member(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetCommitteeMemberRequest>,
) -> JsonResult<SetCommitteeMemberResponse> {
    // TODO: Check admin status from authentication context
    // For now, we'll skip this check in development

    // Validate wallet address
    let member_address = Address::from_str(&req.member_wallet)
        .map_err(|_| avinapi::error::AppError::Validation("Invalid wallet address".into()))?;

    // Upsert committee member in database
    let member = if let Some(existing) =
        CommitteeMember::get_by_wallet(state.db.pool(), &req.member_wallet).await?
    {
        // Update existing member
        sqlx::query!(
            "UPDATE committee_members SET is_approved = $1, updated_at = NOW() WHERE id = $2",
            req.is_approved,
            existing.id
        )
        .execute(state.db.pool())
        .await?;

        CommitteeMember {
            id: existing.id,
            member_wallet: existing.member_wallet,
            is_approved: req.is_approved,
            created_at: existing.created_at,
            updated_at: chrono::Utc::now(),
        }
    } else {
        // Create new member
        let create_req = CreateCommitteeMemberRequest {
            member_wallet: req.member_wallet.to_lowercase(),
            is_approved: req.is_approved,
        };

        CommitteeMember::create(state.db.pool(), create_req).await?
    };

    // Start database transaction
    let mut tx = state.db.pool().begin().await?;

    // Submit to blockchain
    let tx_receipt = if req.is_approved {
        state
            .contract_caller
            .add_committee_member(member_address)
            .await
            .map_err(|e| {
                avinapi::error::AppError::Internal(format!("Failed to submit to blockchain: {}", e))
            })?
    } else {
        state
            .contract_caller
            .remove_committee_member(member_address)
            .await
            .map_err(|e| {
                avinapi::error::AppError::Internal(format!("Failed to submit to blockchain: {}", e))
            })?
    };

    let tx_hash = format!("{:?}", tx_receipt.transaction_hash);

    info!(
        "Committee member {} with tx hash: {}",
        if req.is_approved { "added" } else { "removed" },
        tx_hash
    );

    // Create blockchain transaction record
    let create_tx = CreateTransaction {
        tx_hash: tx_hash.clone(),
        entity_id: member.id as i64,
        entity_type: EntityType::Committee,
    };

    CreateTransaction::create(&mut tx, create_tx).await?;

    // Commit transaction
    tx.commit().await?;

    avinapi::data!(SetCommitteeMemberResponse { tx_hash })
}

/// List confirmed committee members
#[instrument(skip(state))]
pub async fn list_committee_members(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PaginationQuery>,
) -> JsonResult<Vec<CommitteeMember>> {
    let page = query.page;
    let limit = query.per_page;

    let pagination = crate::models::PaginationParams::new(page, limit);
    let paginated_members =
        CommitteeMember::get_confirmed_members(state.db.pool(), pagination).await?;

    avinapi::data!(paginated_members.data)
}

/// Get committee member by ID
#[instrument(skip(state))]
pub async fn get_committee_member(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> JsonResult<CommitteeMember> {
    let member = CommitteeMember::get_confirmed_by_id(state.db.pool(), id)
        .await?
        .ok_or_else(|| avinapi::error::AppError::NotFound("Committee member not found".into()))?;

    avinapi::data!(member)
}

/// Check if wallet is a committee member
#[instrument(skip(state))]
pub async fn is_committee_member(
    State(state): State<Arc<AppState>>,
    Query(query): Query<IsMemberQuery>,
) -> JsonResult<bool> {
    let member = CommitteeMember::get_by_wallet(state.db.pool(), &query.member_wallet).await?;

    let is_member = member.map(|m| m.is_approved).unwrap_or(false);

    avinapi::data!(is_member)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::json;
    use tower::ServiceExt;

    use crate::{
        config::Config,
        infra::{
            contracts::{ContractAddresses, ContractCaller, ContractConfig},
            db::Database,
            tee::{KeyVault, TappdAdapter},
        },
        routes::AppState,
    };

    // Helper function to create test state
    async fn create_test_state() -> Arc<AppState> {
        // Load test configuration
        let config = Config::default();

        // Create database connection
        let db = Database::new(&config.database)
            .await
            .expect("Failed to connect to test database");

        // Run migrations (ignore errors if migrations were already applied)
        let _ = sqlx::migrate!("./migrations").run(db.pool()).await;

        // Create test IPFS client
        let ipfs_client = ipfs_api_backend_hyper::IpfsClient::default();

        // Create contract caller
        let contract_config = ContractConfig {
            http_url: config.chain.rpc_url.clone(),
            ws_url: config.chain.rpc_url.replace("http://", "ws://"),
            chain_id: config.chain.chain_id,
            addresses: ContractAddresses {
                data_contribution: config.chain.contract_address.parse().unwrap(),
                algorithm_review: config.chain.contract_address.parse().unwrap(),
            },
            funding_threshold_eth: 0.1,
            top_up_amount_eth: 1.0,
        };

        let key_vault = Arc::new(KeyVault::new(Box::new(TappdAdapter::new())));
        let contract_caller = ContractCaller::new(contract_config, key_vault, None)
            .await
            .expect("Failed to create contract caller");

        Arc::new(AppState::new(db, config, ipfs_client, contract_caller))
    }

    // Helper function to create test app
    fn create_test_app(state: Arc<AppState>) -> Router {
        Router::new()
            .route(
                "/api/v1/committee",
                axum::routing::get(list_committee_members),
            )
            .route(
                "/api/v1/committee",
                axum::routing::post(set_committee_member),
            )
            .route(
                "/api/v1/committee/{id}",
                axum::routing::get(get_committee_member),
            )
            .route(
                "/api/v1/committee/is-member",
                axum::routing::get(is_committee_member),
            )
            .with_state(state)
    }

    #[tokio::test]
    async fn test_list_members_empty() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/committee")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(json["data"].is_array());
        assert_eq!(json["data"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_set_member_invalid_address() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/committee")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "member_wallet": "invalid_address",
                            "is_approved": true
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["code"], "VALIDATION_ERROR");
        assert!(
            json["message"]
                .as_str()
                .unwrap()
                .contains("Invalid wallet address")
        );
    }

    #[tokio::test]
    async fn test_get_member_not_found() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/committee/999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn test_is_member_not_found() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/committee/is-member?member_wallet=0x742d35Cc6634C0532925a3b844Bc9e7595f8fBbe")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["data"], false);
    }

    #[tokio::test]
    async fn test_is_member_missing_param() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/committee/is-member")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
