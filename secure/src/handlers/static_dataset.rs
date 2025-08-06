use avinapi::prelude::{
    data, paginated, AppError, JsonResult, PaginatedResult, PaginationQuery, ValidatedQuery,
};
use axum::extract::{Multipart, State};
use ipfs_api_backend_hyper::IpfsApi;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use validator::Validate;

use crate::{
    models::{
        static_dataset::{CreateStaticDatasetRequest, StaticDataset},
        Create,
    },
    routes::AppState,
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

/// Form data for creating static dataset (for validation after multipart parsing)
#[derive(Debug, Validate)]
struct CreateStaticDatasetForm {
    #[validate(length(
        min = 1,
        max = 100,
        message = "Name must be between 1 and 100 characters"
    ))]
    pub name: String,
    #[validate(regex(
        path = "crate::ETHEREUM_ADDRESS_REGEX",
        message = "Invalid Ethereum address format"
    ))]
    pub author_wallet: String,
}

/// List static datasets with pagination
#[tracing::instrument(skip(state))]
pub async fn list_static_datasets(
    State(state): State<AppState>,
    ValidatedQuery(params): ValidatedQuery<PaginationQuery>,
) -> PaginatedResult<StaticDatasetResponse> {
    tracing::info!("Listing static datasets with params: {:?}", params);

    // Get static datasets with confirmed blockchain transactions
    let (datasets, total) =
        StaticDataset::find_all_confirmed(state.db.pool(), params.page, params.per_page).await?;

    // Convert to response format
    let items: Vec<StaticDatasetResponse> = datasets
        .into_iter()
        .map(|dataset| dataset.to_response())
        .collect();

    paginated!(items, total, params.page, params.per_page)
}

/// Create a new static dataset
#[tracing::instrument(skip(state, multipart))]
pub async fn create_static_dataset(
    State(state): State<AppState>,
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
        .map_err(|e| AppError::Validation(format!("Failed to parse multipart: {}", e)))?
    {
        let field_name = field
            .name()
            .ok_or_else(|| AppError::Validation("Field name missing".to_string()))?
            .to_string();

        match field_name.as_str() {
            "name" => {
                name =
                    Some(field.text().await.map_err(|e| {
                        AppError::Validation(format!("Failed to read name: {}", e))
                    })?);
            }
            "author_wallet" => {
                author_wallet = Some(field.text().await.map_err(|e| {
                    AppError::Validation(format!("Failed to read author_wallet: {}", e))
                })?);
            }
            "file" => {
                _file_name = field.file_name().map(|s| s.to_string());
                file_data =
                    Some(field.bytes().await.map_err(|e| {
                        AppError::Validation(format!("Failed to read file: {}", e))
                    })?);
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    // Validate required fields
    let name = name.ok_or_else(|| AppError::Validation("Name is required".to_string()))?;
    let author_wallet = author_wallet
        .ok_or_else(|| AppError::Validation("Author wallet is required".to_string()))?;
    let file_data =
        file_data.ok_or_else(|| AppError::Validation("File is required".to_string()))?;

    // Create form struct for validation
    let form = CreateStaticDatasetForm {
        name,
        author_wallet: author_wallet.clone(),
    };

    // Validate form data
    form.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    // Normalize wallet address after validation
    let author_wallet = form.author_wallet.to_lowercase();

    // Continue with the original logic using validated data
    let name = form.name;

    // Calculate file hash
    let mut hasher = Sha256::new();
    hasher.update(&file_data);
    let file_hash = format!("{:x}", hasher.finalize());

    tracing::info!("File hash calculated: {}", file_hash);

    // Check if dataset with this hash already exists
    if let Some(existing) = StaticDataset::find_by_file_hash(state.db.pool(), &file_hash).await? {
        return Err(AppError::Conflict(format!(
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

    let dataset = StaticDataset::create(state.db.pool(), request).await?;

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
        .map_err(|e| AppError::Internal(format!("IPFS upload failed: {}", e)))
}

#[cfg(test)]
mod tests {
    use crate::{
        infra::{db::Database, Hub},
        routes, Config,
    };
    use axum::{
        body::Body,
        http::{header, Request, StatusCode},
        Router,
    };
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn setup_test_app() -> Router {
        // Initialize test environment
        dotenvy::from_filename(".env").ok();

        // Initialize configuration
        let config = Config::from_env().expect("Failed to load config");

        // Initialize database
        let db = Database::new(&config.database)
            .await
            .expect("Failed to connect to database");

        // Run migrations
        // Migrations are handled elsewhere

        // Initialize WebSocket hub
        let ws_hub = Arc::new(Hub::new());

        // Create app
        routes::create_app(db, config, ws_hub).await
    }

    fn create_multipart_body(
        name: &str,
        author_wallet: &str,
        file_content: &[u8],
        file_name: &str,
    ) -> (String, Vec<u8>) {
        let boundary = "WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body = Vec::new();

        // Add name field
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"name\"\r\n\r\n");
        body.extend_from_slice(name.as_bytes());
        body.extend_from_slice(b"\r\n");

        // Add author_wallet field
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"author_wallet\"\r\n\r\n");
        body.extend_from_slice(author_wallet.as_bytes());
        body.extend_from_slice(b"\r\n");

        // Add file field
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
                file_name
            )
            .as_bytes(),
        );
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(file_content);
        body.extend_from_slice(b"\r\n");

        // End boundary
        body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        (format!("multipart/form-data; boundary={}", boundary), body)
    }

    #[tokio::test]
    async fn test_create_static_dataset() {
        let app = setup_test_app().await;

        let file_content = format!(
            "test file content for static dataset - {}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let (content_type, body) = create_multipart_body(
            "Test Dataset",
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            file_content.as_bytes(),
            "test.txt",
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/static-datasets")
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        // Debug: print the actual response body
        println!(
            "test_create_static_dataset Response body: {}",
            serde_json::to_string_pretty(&body).unwrap()
        );

        assert_eq!(body["code"].as_str().unwrap(), "SUCCESS");
        assert!(body["data"]["id"].is_i64());
        assert_eq!(body["data"]["name"].as_str().unwrap(), "Test Dataset");
        assert!(body["data"]["file_hash"].is_string());
        assert!(body["data"]["ipfs_cid"].is_string());
        assert_eq!(
            body["data"]["author_wallet"].as_str().unwrap(),
            "0x70997970c51812dc3a010c7d01b50e0d17dc79c8" // Should be lowercase
        );
    }

    #[tokio::test]
    async fn test_list_static_datasets() {
        let app = setup_test_app().await;

        let request = Request::builder()
            .method("GET")
            .uri("/api/static-datasets?page=1&per_page=10")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        // Debug: print the actual response body
        println!(
            "Response body: {}",
            serde_json::to_string_pretty(&body).unwrap()
        );

        assert_eq!(body["code"].as_str().unwrap(), "SUCCESS");
        assert!(body["data"]["items"].is_array());
        assert_eq!(body["data"]["n_page"], 1);
        assert_eq!(body["data"]["per_page"], 10);
        assert!(body["data"]["total"].is_i64());
    }

    #[tokio::test]
    async fn test_create_static_dataset_invalid_wallet() {
        let app = setup_test_app().await;

        let file_content = b"test file content";
        let (content_type, body) = create_multipart_body(
            "Test Dataset",
            "invalid-wallet-address",
            file_content,
            "test.txt",
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/static-datasets")
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        // Debug: print the actual response body
        println!(
            "test_create_static_dataset_invalid_wallet Response body: {}",
            serde_json::to_string_pretty(&body).unwrap()
        );

        assert_eq!(body["code"].as_str().unwrap(), "VALIDATION_ERROR");
        assert!(body["message"]
            .as_str()
            .unwrap()
            .contains("Invalid Ethereum address"));
    }

    #[tokio::test]
    async fn test_create_static_dataset_missing_name() {
        let app = setup_test_app().await;

        let boundary = "WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body = Vec::new();

        // Only add author_wallet field (missing name and file)
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"author_wallet\"\r\n\r\n");
        body.extend_from_slice(b"0x70997970C51812dc3A010C7d01b50e0d17dc79C8");
        body.extend_from_slice(b"\r\n");

        // End boundary
        body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri("/api/static-datasets")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        // Debug: print the actual response body
        println!(
            "test_create_static_dataset_missing_name Response body: {}",
            serde_json::to_string_pretty(&body).unwrap()
        );

        assert_eq!(body["code"].as_str().unwrap(), "VALIDATION_ERROR");
        assert!(body["message"]
            .as_str()
            .unwrap()
            .contains("Name is required"));
    }

    #[tokio::test]
    async fn test_create_duplicate_static_dataset() {
        let app = setup_test_app().await;

        // Create the first dataset
        let file_content = format!(
            "unique test content for duplicate test - {}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let (content_type, body) = create_multipart_body(
            "First Dataset",
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            file_content.as_bytes(),
            "test.txt",
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/static-datasets")
            .header(header::CONTENT_TYPE, content_type.clone())
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Try to create a duplicate with the same file content
        let (_, body2) = create_multipart_body(
            "Duplicate Dataset",
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            file_content.as_bytes(), // Same content will produce same hash
            "test2.txt",
        );

        let request2 = Request::builder()
            .method("POST")
            .uri("/api/static-datasets")
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(body2))
            .unwrap();

        let response2 = app.oneshot(request2).await.unwrap();
        assert_eq!(response2.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response2.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        // Debug: print the actual response body
        println!(
            "test_create_duplicate_static_dataset Response body: {}",
            serde_json::to_string_pretty(&body).unwrap()
        );

        assert_eq!(body["code"].as_str().unwrap(), "CONFLICT_ERROR");
        assert!(body["message"]
            .as_str()
            .unwrap()
            .contains("Dataset with this file already exists"));
    }
}
