//! Vote handlers
//!
//! This module provides HTTP handlers for querying votes and setting voting duration.

use axum::{
    extract::{Query, State},
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};

use crate::{models::vote::Vote, routes::AppState};
use avinapi::response::JsonResult;

/// Query parameters for listing votes
#[derive(Debug, Deserialize)]
pub struct ListVotesQuery {
    /// Algorithm CID to filter votes
    pub algo_cid: String,
}

/// Request for setting voting duration
#[derive(Debug, Deserialize)]
pub struct SetVotingDurationRequest {
    /// Duration in seconds
    pub duration: u64,
}

/// Response for setting voting duration
#[derive(Debug, Serialize)]
pub struct SetVotingDurationResponse {
    /// Transaction hash
    pub tx_hash: String,
}

/// List votes by algorithm CID
#[instrument(skip(state))]
pub async fn list_votes(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListVotesQuery>,
) -> JsonResult<Vec<Vote>> {
    let votes = Vote::find_by_algo_cid(state.db.pool(), &query.algo_cid).await?;

    avinapi::data!(votes)
}

/// Set voting duration (requires admin)
#[instrument(skip(state))]
pub async fn set_voting_duration(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetVotingDurationRequest>,
) -> JsonResult<SetVotingDurationResponse> {
    // TODO: Check admin status from authentication context
    // For now, we'll skip this check in development

    if req.duration == 0 {
        return Err(avinapi::error::AppError::Validation(
            "Vote duration should be greater than 0".into(),
        ));
    }

    // Submit to blockchain
    let tx_receipt = state
        .contract_caller
        .set_voting_duration(req.duration)
        .await
        .map_err(|e| {
            avinapi::error::AppError::Internal(format!("Failed to set voting duration: {}", e))
        })?;

    let tx_hash = format!("{:?}", tx_receipt.transaction_hash);

    info!(
        "Voting duration set to {} seconds with tx hash: {}",
        req.duration, tx_hash
    );

    avinapi::data!(SetVotingDurationResponse { tx_hash })
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
            .route("/api/v1/votes", axum::routing::get(list_votes))
            .route(
                "/api/v1/votes/set-voting-duration",
                axum::routing::post(set_voting_duration),
            )
            .with_state(state)
    }

    #[tokio::test]
    async fn test_list_votes_empty() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/votes?algo_cid=QmTest")
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
    async fn test_list_votes_missing_param() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/votes")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_set_voting_duration_zero() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/votes/set-voting-duration")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "duration": 0
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
        assert!(json["message"].as_str().unwrap().contains("greater than 0"));
    }

    #[tokio::test]
    #[ignore = "Requires blockchain connection"]
    async fn test_set_voting_duration_success() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/votes/set-voting-duration")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "duration": 3600
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

        assert!(json["data"]["tx_hash"].is_string());
    }
}
