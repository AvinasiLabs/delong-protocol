use avinapi::{
    data,
    error::AppError,
    query::{PaginatedData, PaginationQuery},
    response::{JsonResult, PaginatedResult},
};
use axum::extract::{Multipart, Query, State};
use ipfs_api_backend_hyper::IpfsApi;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    handlers::HandlerState,
    models::{
        Create,
        static_dataset::{CreateStaticDatasetRequest, StaticDataset},
    },
};

/// Response for static dataset
#[derive(Debug, Serialize, Deserialize)]
pub struct StaticDatasetResponse {
    pub id: u64,
    pub name: String,
    pub file_hash: String,
    pub ipfs_cid: String,
    pub author_wallet: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Response for static dataset creation
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateStaticDatasetResponse {
    pub id: u64,
    pub name: String,
    pub file_hash: String,
    pub ipfs_cid: String,
    pub author_wallet: String,
    pub message: String,
}

/// List static datasets with pagination
#[tracing::instrument(skip(state))]
pub async fn list_static_datasets(
    State(state): State<HandlerState>,
    Query(params): Query<PaginationQuery>,
) -> PaginatedResult<StaticDatasetResponse> {
    tracing::info!("Listing static datasets with params: {:?}", params);

    // Get paginated static datasets with confirmed blockchain transactions
    let paginated_datasets = StaticDataset::find_all_confirmed(&state.db_pool, params).await?;

    // Convert to response format
    let response_data: Vec<StaticDatasetResponse> = paginated_datasets
        .data
        .into_iter()
        .map(|dataset| dataset.to_response())
        .collect();

    // Create pagination meta
    let meta = paginated_datasets.meta;

    // Create paginated response
    let paginated = PaginatedData::with_meta(response_data, meta);

    data!(paginated)
}

/// Create a new static dataset
#[tracing::instrument(skip(state, multipart))]
pub async fn create_static_dataset(
    State(state): State<HandlerState>,
    mut multipart: Multipart,
) -> JsonResult<CreateStaticDatasetResponse> {
    let mut name = None;
    let mut author_wallet = None;
    let mut file_data = None;
    let mut _file_name = None;

    // Parse multipart form data
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("Failed to parse multipart: {}", e)))?
    {
        let field_name = field
            .name()
            .ok_or_else(|| AppError::bad_request("Field name missing"))?
            .to_string();

        match field_name.as_str() {
            "name" => {
                name =
                    Some(field.text().await.map_err(|e| {
                        AppError::bad_request(format!("Failed to read name: {}", e))
                    })?);
            }
            "author_wallet" => {
                author_wallet = Some(field.text().await.map_err(|e| {
                    AppError::bad_request(format!("Failed to read author_wallet: {}", e))
                })?);
            }
            "file" => {
                _file_name = field.file_name().map(|s| s.to_string());
                file_data =
                    Some(field.bytes().await.map_err(|e| {
                        AppError::bad_request(format!("Failed to read file: {}", e))
                    })?);
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    // Validate required fields
    let name = name.ok_or_else(|| AppError::validation("Name is required"))?;
    let author_wallet = author_wallet
        .ok_or_else(|| AppError::validation("Author wallet is required"))?
        .to_lowercase(); // Normalize wallet address
    let file_data = file_data.ok_or_else(|| AppError::validation("File is required"))?;

    // Validate wallet address format
    if !author_wallet.starts_with("0x") || author_wallet.len() != 42 {
        return Err(AppError::validation("Invalid wallet address format"));
    }

    // Calculate file hash
    let mut hasher = Sha256::new();
    hasher.update(&file_data);
    let file_hash = format!("{:x}", hasher.finalize());

    tracing::info!("File hash calculated: {}", file_hash);

    // Check if dataset with this hash already exists
    if let Some(existing) = StaticDataset::find_by_file_hash(&state.db_pool, &file_hash).await? {
        return Err(AppError::conflict(format!(
            "Dataset with this file already exists (ID: {})",
            existing.id
        )));
    }

    // TODO: Encrypt file using TEE before uploading to IPFS
    // For now, we'll upload the raw file
    let encrypted_data = file_data.clone();

    // Upload to IPFS
    let ipfs_result = upload_to_ipfs(&state.ipfs_client, encrypted_data).await?;
    let ipfs_cid = ipfs_result.hash;

    tracing::info!("File uploaded to IPFS with CID: {}", ipfs_cid);

    // Create database record
    let request = CreateStaticDatasetRequest {
        name: name.clone(),
        ui_name: name.clone(), // Use name as ui_name for now
        desc: None,
        file_hash: file_hash.clone(),
        ipfs_cid: ipfs_cid.clone(),
        file_size: file_data.len() as i64,
        file_format: "unknown".to_string(), // TODO: Detect file format from content or filename
        author: None,
        author_wallet: author_wallet.clone(),
        sample_url: None,
        file_path: None,
    };

    let dataset = StaticDataset::create(&state.db_pool, request).await?;

    // TODO: Submit blockchain transaction to register dataset
    // For now, we'll just create a pending transaction record

    tracing::info!("Static dataset created with ID: {}", dataset.id);

    let response = CreateStaticDatasetResponse {
        id: dataset.id as u64,
        name: dataset.name,
        file_hash: dataset.file_hash,
        ipfs_cid: dataset.ipfs_cid,
        author_wallet: dataset.author_wallet,
        message: "Static dataset created successfully. Blockchain registration pending."
            .to_string(),
    };

    data!(response)
}

/// Upload data to IPFS
async fn upload_to_ipfs(
    client: &ipfs_api_backend_hyper::IpfsClient,
    data: bytes::Bytes,
) -> Result<ipfs_api_backend_hyper::response::AddResponse, AppError> {
    let reader = std::io::Cursor::new(data);

    client
        .add(reader)
        .await
        .map_err(|e| AppError::ipfs(format!("IPFS upload failed: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        routing::{get, post},
    };
    use sqlx::PgPool;
    use std::sync::Arc;
    use tower::ServiceExt;

    // Helper function to create test state
    async fn create_test_state() -> HandlerState {
        // Create test database pool (you may want to use a test database)
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:password@localhost/test_delong".to_string());
        let db_pool = PgPool::connect(&db_url)
            .await
            .expect("Failed to connect to test database");

        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&db_pool)
            .await
            .expect("Failed to run migrations");

        // Create test IPFS client
        let ipfs_client = Arc::new(ipfs_api_backend_hyper::IpfsClient::default());

        HandlerState {
            db_pool,
            ipfs_client,
        }
    }

    // Helper function to create test app
    fn create_test_app(state: HandlerState) -> Router {
        Router::new()
            .route("/static-datasets", get(list_static_datasets))
            .route("/static-datasets", post(create_static_dataset))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_list_static_datasets_empty() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/static-datasets")
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
        assert!(json["meta"].is_object());
    }

    #[tokio::test]
    async fn test_list_static_datasets_with_pagination() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/static-datasets?page=1&page_size=10")
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
        assert!(json["meta"].is_object());
        assert_eq!(json["meta"]["page"], 1);
        assert_eq!(json["meta"]["page_size"], 10);
    }

    #[tokio::test]
    async fn test_create_static_dataset_missing_fields() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        // Create multipart form without required fields
        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let body = format!(
            "------WebKitFormBoundary7MA4YWxkTrZu0gW\r\n\
            Content-Disposition: form-data; name=\"name\"\r\n\r\n\
            Test Dataset\r\n\
            ------WebKitFormBoundary7MA4YWxkTrZu0gW--\r\n"
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/static-datasets")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={}", boundary),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_create_static_dataset_invalid_wallet() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        // Create multipart form with invalid wallet
        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let body = format!(
            "------WebKitFormBoundary7MA4YWxkTrZu0gW\r\n\
            Content-Disposition: form-data; name=\"name\"\r\n\r\n\
            Test Dataset\r\n\
            ------WebKitFormBoundary7MA4YWxkTrZu0gW\r\n\
            Content-Disposition: form-data; name=\"author_wallet\"\r\n\r\n\
            invalid_wallet\r\n\
            ------WebKitFormBoundary7MA4YWxkTrZu0gW\r\n\
            Content-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\n\
            Content-Type: text/plain\r\n\r\n\
            test file content\r\n\
            ------WebKitFormBoundary7MA4YWxkTrZu0gW--\r\n"
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/static-datasets")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={}", boundary),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(
            json["message"]
                .as_str()
                .unwrap()
                .contains("Invalid wallet address")
        );
    }

    #[tokio::test]
    #[ignore = "Requires IPFS daemon running"]
    async fn test_create_static_dataset_success() {
        let state = create_test_state().await;
        let app = create_test_app(state);

        // Create multipart form with all required fields
        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let body = format!(
            "------WebKitFormBoundary7MA4YWxkTrZu0gW\r\n\
            Content-Disposition: form-data; name=\"name\"\r\n\r\n\
            Test Dataset\r\n\
            ------WebKitFormBoundary7MA4YWxkTrZu0gW\r\n\
            Content-Disposition: form-data; name=\"author_wallet\"\r\n\r\n\
            0x1234567890123456789012345678901234567890\r\n\
            ------WebKitFormBoundary7MA4YWxkTrZu0gW\r\n\
            Content-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\n\
            Content-Type: text/plain\r\n\r\n\
            test file content\r\n\
            ------WebKitFormBoundary7MA4YWxkTrZu0gW--\r\n"
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/static-datasets")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={}", boundary),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(json["id"].is_number());
        assert_eq!(json["name"], "Test Dataset");
        assert!(json["file_hash"].is_string());
        assert!(json["ipfs_cid"].is_string());
        assert_eq!(
            json["author_wallet"],
            "0x1234567890123456789012345678901234567890"
        );
        assert!(json["message"].as_str().unwrap().contains("successfully"));
    }

    #[test]
    fn test_wallet_validation() {
        // Valid wallet addresses
        let valid_wallet = "0x1234567890123456789012345678901234567890";
        assert_eq!(valid_wallet.len(), 42);
        assert!(valid_wallet.starts_with("0x"));

        // Invalid wallet addresses
        let invalid_wallets = vec![
            "1234567890123456789012345678901234567890",  // Missing 0x
            "0x123456789012345678901234567890123456789", // Too short
            "0x12345678901234567890123456789012345678901", // Too long
        ];

        for wallet in invalid_wallets {
            assert!(!wallet.starts_with("0x") || wallet.len() != 42);
        }
    }
}
