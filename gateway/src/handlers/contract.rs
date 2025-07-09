//! Contract metadata handlers
//!
//! This module contains handlers for contract metadata management,
//! including listing smart contract information.

use axum::{
    extract::{Query, State},
    response::Json,
};
use tracing::{error, info};

use crate::{
    handlers::{ApiResponse, PaginatedResponse, PaginationParams},
    routes::AppState,
    services::http_client::forward_get,
};

use common::prelude::ContractData;

/// Handler for getting contract list
///
/// GET /api/contracts
/// Returns paginated list of smart contracts
#[utoipa::path(
    get,
    path = "/api/contracts",
    tag = "contracts",
    summary = "List smart contracts",
    description = "Retrieve a paginated list of smart contract metadata",
    params(
        ("page" = Option<u32>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u32>, Query, description = "Items per page (default: 20, max: 100)")
    ),
    responses(
        (status = 200, description = "Smart contracts retrieved successfully", body = ApiResponse<PaginatedResponse<ContractData>>),
        (status = 400, description = "Invalid query parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_contracts_handler(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Json<ApiResponse<PaginatedResponse<ContractData>>> {
    info!(
        "Getting contracts list: page={}, limit={}",
        params.page, params.limit
    );

    // Forward request to Secure service
    let secure_url = format!(
        "{}/api/contracts?page={}&limit={}",
        state.config.services.secure_url, params.page, params.limit
    );

    match forward_get::<PaginatedResponse<ContractData>>(
        state.http_client.as_ref(),
        &secure_url,
        None,
    )
    .await
    {
        Ok(response) => {
            info!(
                "Retrieved {} contracts (page {}/{})",
                response.items.len(),
                response.page,
                response.total_pages
            );
            Json(ApiResponse::success(response))
        }
        Err(e) => {
            error!("Failed to get contracts: {:?}", e);
            Json(ApiResponse::internal_error())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GatewayConfig, ServicesConfig};
    use crate::handlers::ResponseCode;
    #[allow(dead_code)]
    fn create_test_config() -> GatewayConfig {
        let mut config = GatewayConfig::default();
        config.services = ServicesConfig {
            core_url: "http://localhost:8001".to_string(),
            secure_url: "http://localhost:8002".to_string(),
        };
        config
    }

    #[test]
    fn test_contract_data_deserialization() {
        let json = r#"
        {
            "id": 1,
            "name": "DeLong Core Contract",
            "address": "0x1234567890abcdef1234567890abcdef12345678",
            "created_at": "2023-01-01T00:00:00Z"
        }
        "#;

        let data: ContractData = serde_json::from_str(json).unwrap();
        assert_eq!(data.id, 1);
        assert_eq!(data.name, "DeLong Core Contract");
        assert_eq!(data.address, "0x1234567890abcdef1234567890abcdef12345678");
        assert_eq!(data.created_at, "2023-01-01T00:00:00Z");
    }

    #[test]
    fn test_contract_data_serialization() {
        let contract = ContractData {
            id: 42,
            name: "Test Contract".to_string(),
            address: "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd".to_string(),
            created_at: "2023-12-01T10:30:00Z".to_string(),
        };

        let json = serde_json::to_string(&contract).unwrap();
        assert!(json.contains("\"id\":42"));
        assert!(json.contains("Test Contract"));
        assert!(json.contains("0xabcdefabcdefabcdefabcdefabcdefabcdefabcd"));
    }

    #[test]
    fn test_pagination_with_contract_data() {
        let contracts = vec![
            ContractData {
                id: 1,
                name: "Contract One".to_string(),
                address: "0x1111111111111111111111111111111111111111".to_string(),
                created_at: "2023-01-01T00:00:00Z".to_string(),
            },
            ContractData {
                id: 2,
                name: "Contract Two".to_string(),
                address: "0x2222222222222222222222222222222222222222".to_string(),
                created_at: "2023-01-02T00:00:00Z".to_string(),
            },
        ];

        let paginated = PaginatedResponse::new(contracts, 1, 20, 10);
        assert_eq!(paginated.items.len(), 2);
        assert_eq!(paginated.total_items, 10);
        assert_eq!(paginated.total_pages, 1);
    }

    #[test]
    fn test_api_response_with_contracts() {
        let contracts = vec![ContractData {
            id: 1,
            name: "Test Contract".to_string(),
            address: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            created_at: "2023-01-01T00:00:00Z".to_string(),
        }];

        let paginated = PaginatedResponse::new(contracts, 1, 20, 1);
        let api_response = ApiResponse::success(paginated);

        assert_eq!(api_response.code, ResponseCode::Success);
        assert!(api_response.data.is_some());
        let data = api_response.data.unwrap();
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.items[0].name, "Test Contract");
    }

    #[test]
    fn test_pagination_params_for_contracts() {
        let params = PaginationParams::default();
        assert_eq!(params.page, 1);
        assert_eq!(params.limit, 20);

        // Test custom pagination
        let json = r#"{"page": 3, "limit": 10}"#;
        let custom_params: PaginationParams = serde_json::from_str(json).unwrap();
        assert_eq!(custom_params.page, 3);
        assert_eq!(custom_params.limit, 10);
    }
}
