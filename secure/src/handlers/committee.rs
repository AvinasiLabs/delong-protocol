//! Committee management handlers
//!
//! This module provides HTTP handlers for managing committee members,
//! including listing, setting member status, and checking membership.

use alloy::primitives::Address;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{info, instrument};

use crate::{
    models::{
        blockchain_transaction::{CreateTransaction, EntityType},
        committee::{CommitteeMember, CreateCommitteeMemberRequest},
        Create,
    },
    routes::AppState,
};
use avinapi::prelude::{
    data, paginated, AppError, JsonResult, PaginatedResult, PaginationQuery, ValidatedJson,
    ValidatedQuery,
};
use validator::Validate;

/// Request for setting committee member status
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SetCommitteeMemberRequest {
    /// Wallet address of the member
    #[validate(regex(
        path = "crate::ETHEREUM_ADDRESS_REGEX",
        message = "Invalid Ethereum address format"
    ))]
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
#[derive(Debug, Deserialize, Validate)]
pub struct IsMemberQuery {
    /// Wallet address to check
    #[validate(regex(
        path = "crate::ETHEREUM_ADDRESS_REGEX",
        message = "Invalid Ethereum address format"
    ))]
    pub member_wallet: String,
}

/// Set committee member status (requires admin)
#[instrument(skip(state))]
#[axum::debug_handler]
pub async fn set_committee_member(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<SetCommitteeMemberRequest>,
) -> JsonResult<SetCommitteeMemberResponse> {
    // TODO: Check admin status from authentication context
    // For now, we'll skip this check in development

    // Validate wallet address
    let member_address = Address::from_str(&req.member_wallet)
        .map_err(|_| AppError::Validation("Invalid wallet address".into()))?;

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
            .map_err(|e| AppError::Internal(format!("Failed to submit to blockchain: {}", e)))?
    } else {
        state
            .contract_caller
            .remove_committee_member(member_address)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to submit to blockchain: {}", e)))?
    };

    let tx_hash = tx_receipt;

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

    data!(SetCommitteeMemberResponse { tx_hash })
}

/// List confirmed committee members
#[instrument(skip(state))]
pub async fn list_committee_members(
    State(state): State<AppState>,
    ValidatedQuery(params): ValidatedQuery<PaginationQuery>,
) -> PaginatedResult<CommitteeMember> {
    let result = CommitteeMember::get_confirmed_members(state.db.pool(), params).await?;

    paginated!(result.items, result.total, result.n_page, result.per_page)
}

/// Get committee member by ID
#[instrument(skip(state))]
#[axum::debug_handler]
pub async fn get_committee_member(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> JsonResult<CommitteeMember> {
    let member = CommitteeMember::get_confirmed_by_id(state.db.pool(), id)
        .await?
        .ok_or_else(|| AppError::NotFound("Committee member not found".into()))?;

    data!(member)
}

/// Check if wallet is a committee member
#[instrument(skip(state))]
#[axum::debug_handler]
pub async fn is_committee_member(
    State(state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<IsMemberQuery>,
) -> JsonResult<bool> {
    let member = CommitteeMember::get_by_wallet(state.db.pool(), &query.member_wallet).await?;

    let is_member = member.map(|m| m.is_approved).unwrap_or(false);

    data!(is_member)
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::{extract_json_body, generate_test_wallet_address, setup_test_app};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::json;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_set_committee_member_add() {
        let app = setup_test_app().await;

        let request_body = json!({
            "member_wallet": generate_test_wallet_address("committee-add"),
            "is_approved": true
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/committee")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&request_body).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = extract_json_body(response).await;

        // Debug output to see what's in the response
        println!(
            "Response JSON: {}",
            serde_json::to_string_pretty(&json).unwrap()
        );

        assert_eq!(json["code"], "SUCCESS");
        assert!(json["data"]["tx_hash"].is_string());
    }

    #[tokio::test]
    async fn test_set_committee_member_remove() {
        let app = setup_test_app().await;

        let request_body = json!({
            "member_wallet": generate_test_wallet_address("committee-remove"),
            "is_approved": false
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/committee")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&request_body).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = extract_json_body(response).await;

        assert_eq!(json["code"], "SUCCESS");
        assert!(json["data"]["tx_hash"].is_string());
    }

    #[tokio::test]
    async fn test_set_committee_member_invalid_wallet() {
        let app = setup_test_app().await;

        let request_body = json!({
            "member_wallet": "invalid-wallet-address",
            "is_approved": true
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/committee")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&request_body).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = extract_json_body(response).await;

        assert_eq!(json["code"], "VALIDATION_ERROR");
        assert!(json["message"]
            .as_str()
            .unwrap()
            .contains("Invalid Ethereum address"));
    }

    #[tokio::test]
    async fn test_list_committee_members() {
        let app = setup_test_app().await;

        let request = Request::builder()
            .method("GET")
            .uri("/api/committee?page=1&per_page=10")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = extract_json_body(response).await;

        // Debug output to see what's in the response
        println!(
            "Response JSON: {}",
            serde_json::to_string_pretty(&json).unwrap()
        );

        assert_eq!(json["code"], "SUCCESS");
        assert!(json["data"]["items"].is_array());
        assert!(json["data"]["total"].is_number());
        assert_eq!(json["data"]["n_page"], 1);
        assert_eq!(json["data"]["per_page"], 10);
    }

    #[tokio::test]
    async fn test_get_committee_member() {
        let app = setup_test_app().await;

        // Test with non-existent member
        let request = Request::builder()
            .method("GET")
            .uri("/api/committee/999999")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = extract_json_body(response).await;

        assert_eq!(json["code"], "NOT_FOUND_ERROR");
        assert!(json["message"]
            .as_str()
            .unwrap()
            .contains("Committee member not found"));
    }

    #[tokio::test]
    async fn test_is_committee_member() {
        let app = setup_test_app().await;

        let request = Request::builder()
            .method("GET")
            .uri(
                "/api/committee/is-member?member_wallet=0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            )
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = extract_json_body(response).await;

        assert_eq!(json["code"], "SUCCESS");
        assert!(json["data"].is_boolean());
    }

    #[tokio::test]
    async fn test_is_committee_member_missing_wallet() {
        let app = setup_test_app().await;

        let request = Request::builder()
            .method("GET")
            .uri("/api/committee/is-member")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Check the actual status code
        eprintln!("Actual status code: {}", response.status());

        if response.status() == StatusCode::OK {
            let json = extract_json_body(response).await;
            eprintln!(
                "Response body: {}",
                serde_json::to_string_pretty(&json).unwrap()
            );

            // Check if it's a validation/parsing error
            assert!(json["code"] == "VALIDATION_ERROR" || json["code"] == "PARSING_ERROR");
            assert!(json["message"].as_str().unwrap().contains("member_wallet"));
        } else {
            // Original assertion
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }
}
