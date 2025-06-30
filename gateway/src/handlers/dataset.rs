//! Datasets management handlers for static and dynamic dataset operations
//!
//! This module handles dataset-related operations by forwarding requests to
//! the appropriate backend services (secure/core). It implements both static
//! dataset management (IPFS + blockchain) and dynamic dataset management
//! (local file system with versioning).

use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::Json,
};
use reqwest::multipart::Form;
use tracing::{error, info, instrument, warn};

use crate::{routes::AppState, utils::generate_request_id};

use common::{
    CreateDatasetRequest, DatasetPaginatedResponse, DelongApiResponse, DynamicDatasetInfo,
    DynamicDatasetListQuery, StaticDatasetInfo, StaticDatasetListQuery, UpdateDatasetRequest,
    UpdateStaticDatasetRequest,
};

/// Upload static dataset (multipart/form-data)
/// Forwards to POST /api/static-datasets on secure service
#[instrument(skip(multipart, state), fields(request_id))]
pub async fn upload_static_dataset_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<DelongApiResponse<String>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        "Static dataset upload request received"
    );

    // Parse multipart form data
    let mut form = Form::new();
    let mut dataset_name = String::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        error!("Failed to parse multipart field: {}", e);
        StatusCode::BAD_REQUEST
    })? {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "file" => {
                let filename = field.file_name().unwrap_or("dataset.csv").to_string();
                let data = field.bytes().await.map_err(|e| {
                    error!("Failed to read file data: {}", e);
                    StatusCode::BAD_REQUEST
                })?;

                form = form.part(
                    "file",
                    reqwest::multipart::Part::bytes(data.to_vec()).file_name(filename),
                );
            }
            "name" => {
                dataset_name = field.text().await.map_err(|e| {
                    error!("Failed to read name field: {}", e);
                    StatusCode::BAD_REQUEST
                })?;
                form = form.text("name", dataset_name.clone());
            }
            "desc" => {
                let desc = field.text().await.map_err(|e| {
                    error!("Failed to read desc field: {}", e);
                    StatusCode::BAD_REQUEST
                })?;
                form = form.text("desc", desc);
            }
            "author" => {
                let author = field.text().await.map_err(|e| {
                    error!("Failed to read author field: {}", e);
                    StatusCode::BAD_REQUEST
                })?;
                form = form.text("author", author);
            }
            "author_wallet" => {
                let author_wallet = field.text().await.map_err(|e| {
                    error!("Failed to read author_wallet field: {}", e);
                    StatusCode::BAD_REQUEST
                })?;
                form = form.text("author_wallet", author_wallet);
            }
            _ => {
                warn!("Unknown form field: {}", field_name);
            }
        }
    }

    // Validate required fields
    if dataset_name.is_empty() {
        warn!(request_id = request_id, "Dataset name is required");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Forward to secure service
    let secure_service_url = &state.config.services.secure_url;

    let client = reqwest::Client::new();
    let response = client
        .post(&format!("{}/api/static-datasets", secure_service_url))
        .multipart(form)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to forward request to secure service: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if response.status().is_success() {
        let response_data: DelongApiResponse<String> = response.json().await.map_err(|e| {
            error!("Failed to parse response from secure service: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        info!(
            request_id = request_id,
            dataset_name = dataset_name,
            "Static dataset upload forwarded successfully"
        );

        Ok(Json(response_data))
    } else {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        error!(
            request_id = request_id,
            status = %status,
            error = error_text,
            "Secure service returned error"
        );

        Err(map_secure_service_status(status))
    }
}

/// Get static datasets list with pagination
/// Forwards to GET /api/static-datasets on secure service
#[instrument(skip(state), fields(request_id))]
pub async fn get_static_datasets_handler(
    State(state): State<AppState>,
    Query(query): Query<StaticDatasetListQuery>,
) -> Result<Json<DelongApiResponse<DatasetPaginatedResponse<StaticDatasetInfo>>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        page = query.page,
        page_size = query.page_size,
        "Static datasets list request received"
    );

    // Forward to secure service
    let secure_service_url = &state.config.services.secure_url;

    let client = reqwest::Client::new();
    let response = client
        .get(&format!("{}/api/static-datasets", secure_service_url))
        .query(&[("page", query.page), ("page_size", query.page_size)])
        .send()
        .await
        .map_err(|e| {
            error!("Failed to forward request to secure service: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if response.status().is_success() {
        let response_data: DelongApiResponse<DatasetPaginatedResponse<StaticDatasetInfo>> =
            response.json().await.map_err(|e| {
                error!("Failed to parse response from secure service: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!(
            request_id = request_id,
            total_items = response_data.data.as_ref().map(|d| d.total).unwrap_or(0),
            "Static datasets list retrieved successfully"
        );

        Ok(Json(response_data))
    } else {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        error!(
            request_id = request_id,
            status = %status,
            error = error_text,
            "Secure service returned error"
        );

        Err(map_secure_service_status(status))
    }
}

/// Get single static dataset by ID
/// Forwards to GET /api/static-datasets/{id} on secure service
#[instrument(skip(state), fields(request_id, dataset_id = %dataset_id))]
pub async fn get_static_dataset_handler(
    State(state): State<AppState>,
    Path(dataset_id): Path<u32>,
) -> Result<Json<DelongApiResponse<StaticDatasetInfo>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        dataset_id = dataset_id,
        "Static dataset info request received"
    );

    // Forward to secure service
    let secure_service_url = &state.config.services.secure_url;

    let client = reqwest::Client::new();
    let response = client
        .get(&format!(
            "{}/api/static-datasets/{}",
            secure_service_url, dataset_id
        ))
        .send()
        .await
        .map_err(|e| {
            error!("Failed to forward request to secure service: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if response.status().is_success() {
        let response_data: DelongApiResponse<StaticDatasetInfo> =
            response.json().await.map_err(|e| {
                error!("Failed to parse response from secure service: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!(
            request_id = request_id,
            dataset_id = dataset_id,
            "Static dataset info retrieved successfully"
        );

        Ok(Json(response_data))
    } else {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        error!(
            request_id = request_id,
            dataset_id = dataset_id,
            status = %status,
            error = error_text,
            "Secure service returned error"
        );

        Err(map_secure_service_status(status))
    }
}

/// Get sample data by CID (public access, no authentication required)
/// Forwards to GET /api/sample/{cid} on secure service
#[instrument(skip(state), fields(request_id, cid = %cid))]
pub async fn get_sample_data_handler(
    State(state): State<AppState>,
    Path(cid): Path<String>,
) -> Result<axum::response::Response, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        cid = cid,
        "Sample data request received"
    );

    // Forward to secure service
    let secure_service_url = &state.config.services.secure_url;

    let client = reqwest::Client::new();
    let response = client
        .get(&format!("{}/api/sample/{}", secure_service_url, cid))
        .send()
        .await
        .map_err(|e| {
            error!("Failed to forward request to secure service: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if response.status().is_success() {
        let headers = response.headers().clone();
        let content_type = headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/csv");

        let default_disposition = format!("attachment; filename=\"sample_{}.csv\"", cid);
        let content_disposition = headers
            .get("content-disposition")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(&default_disposition);

        let body = response.bytes().await.map_err(|e| {
            error!("Failed to read response body from secure service: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        info!(
            request_id = request_id,
            cid = cid,
            content_length = body.len(),
            "Sample data retrieved successfully"
        );

        // Return the raw CSV data with appropriate headers
        Ok(axum::response::Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", content_type)
            .header("Content-Disposition", content_disposition)
            .body(axum::body::Body::from(body))
            .unwrap())
    } else {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        error!(
            request_id = request_id,
            cid = cid,
            status = %status,
            error = error_text,
            "Secure service returned error"
        );

        Err(map_secure_service_status(status))
    }
}

/// Create dynamic dataset (admin only)
/// Forwards to POST /api/datasets on core service
#[instrument(skip(payload, state), fields(request_id))]
pub async fn create_dataset_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateDatasetRequest>,
) -> Result<Json<DelongApiResponse<DynamicDatasetInfo>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        dataset_name = payload.name,
        "Dynamic dataset creation request received"
    );

    // Validate request
    if payload.name.trim().is_empty() {
        warn!(request_id = request_id, "Dataset name is required");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Forward to core service
    let core_service_url = &state.config.services.core_url;

    let client = reqwest::Client::new();
    let response = client
        .post(&format!("{}/api/datasets", core_service_url))
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to forward request to core service: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if response.status().is_success() {
        let response_data: DelongApiResponse<DynamicDatasetInfo> =
            response.json().await.map_err(|e| {
                error!("Failed to parse response from core service: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!(
            request_id = request_id,
            dataset_name = payload.name,
            "Dynamic dataset creation forwarded successfully"
        );

        Ok(Json(response_data))
    } else {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        error!(
            request_id = request_id,
            status = %status,
            error = error_text,
            "Core service returned error"
        );

        Err(map_core_service_status(status))
    }
}

/// Get dynamic datasets list with pagination
/// Forwards to GET /api/datasets on core service
#[instrument(skip(state), fields(request_id))]
pub async fn get_datasets_handler(
    State(state): State<AppState>,
    Query(query): Query<DynamicDatasetListQuery>,
) -> Result<Json<DelongApiResponse<DatasetPaginatedResponse<DynamicDatasetInfo>>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        page = query.page,
        page_size = query.page_size,
        "Dynamic datasets list request received"
    );

    // Forward to core service
    let core_service_url = &state.config.services.core_url;

    let client = reqwest::Client::new();
    let response = client
        .get(&format!("{}/api/datasets", core_service_url))
        .query(&[("page", query.page), ("page_size", query.page_size)])
        .send()
        .await
        .map_err(|e| {
            error!("Failed to forward request to core service: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if response.status().is_success() {
        let response_data: DelongApiResponse<DatasetPaginatedResponse<DynamicDatasetInfo>> =
            response.json().await.map_err(|e| {
                error!("Failed to parse response from core service: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!(
            request_id = request_id,
            total_items = response_data.data.as_ref().map(|d| d.total).unwrap_or(0),
            "Dynamic datasets list retrieved successfully"
        );

        Ok(Json(response_data))
    } else {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        error!(
            request_id = request_id,
            status = %status,
            error = error_text,
            "Core service returned error"
        );

        Err(map_core_service_status(status))
    }
}

/// Map secure service HTTP status to gateway status
fn map_secure_service_status(status: reqwest::StatusCode) -> StatusCode {
    match status {
        reqwest::StatusCode::BAD_REQUEST => StatusCode::BAD_REQUEST,
        reqwest::StatusCode::UNAUTHORIZED => StatusCode::UNAUTHORIZED,
        reqwest::StatusCode::FORBIDDEN => StatusCode::FORBIDDEN,
        reqwest::StatusCode::NOT_FOUND => StatusCode::NOT_FOUND,
        reqwest::StatusCode::CONFLICT => StatusCode::CONFLICT,
        reqwest::StatusCode::UNPROCESSABLE_ENTITY => StatusCode::UNPROCESSABLE_ENTITY,
        reqwest::StatusCode::INTERNAL_SERVER_ERROR => StatusCode::INTERNAL_SERVER_ERROR,
        reqwest::StatusCode::SERVICE_UNAVAILABLE => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Map core service HTTP status to gateway status
fn map_core_service_status(status: reqwest::StatusCode) -> StatusCode {
    match status {
        reqwest::StatusCode::BAD_REQUEST => StatusCode::BAD_REQUEST,
        reqwest::StatusCode::UNAUTHORIZED => StatusCode::UNAUTHORIZED,
        reqwest::StatusCode::FORBIDDEN => StatusCode::FORBIDDEN,
        reqwest::StatusCode::NOT_FOUND => StatusCode::NOT_FOUND,
        reqwest::StatusCode::CONFLICT => StatusCode::CONFLICT,
        reqwest::StatusCode::UNPROCESSABLE_ENTITY => StatusCode::UNPROCESSABLE_ENTITY,
        reqwest::StatusCode::INTERNAL_SERVER_ERROR => StatusCode::INTERNAL_SERVER_ERROR,
        reqwest::StatusCode::SERVICE_UNAVAILABLE => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Update dynamic dataset
/// PUT /api/datasets/{id}
#[instrument(skip(state), fields(request_id))]
pub async fn update_dataset_handler(
    State(state): State<AppState>,
    Path(id): Path<u32>,
    Json(payload): Json<UpdateDatasetRequest>,
) -> Result<Json<DelongApiResponse<DynamicDatasetInfo>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        dataset_id = id,
        "Dynamic dataset update request received"
    );

    // Forward to core service
    let core_service_url = &state.config.services.core_url;

    let client = reqwest::Client::new();
    let response = client
        .put(&format!("{}/api/datasets/{}", core_service_url, id))
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to forward request to core service: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if response.status().is_success() {
        let response_data: DelongApiResponse<DynamicDatasetInfo> =
            response.json().await.map_err(|e| {
                error!("Failed to parse response from core service: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!(
            request_id = request_id,
            dataset_id = id,
            "Dynamic dataset updated successfully"
        );

        Ok(Json(response_data))
    } else {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        error!(
            request_id = request_id,
            status = %status,
            error = error_text,
            "Core service returned error"
        );

        Err(map_core_service_status(status))
    }
}

/// Delete dynamic dataset
/// DELETE /api/datasets/{id}
#[instrument(skip(state), fields(request_id))]
pub async fn delete_dataset_handler(
    State(state): State<AppState>,
    Path(id): Path<u32>,
) -> Result<Json<DelongApiResponse<()>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        dataset_id = id,
        "Dynamic dataset delete request received"
    );

    // Forward to core service
    let core_service_url = &state.config.services.core_url;

    let client = reqwest::Client::new();
    let response = client
        .delete(&format!("{}/api/datasets/{}", core_service_url, id))
        .send()
        .await
        .map_err(|e| {
            error!("Failed to forward request to core service: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if response.status().is_success() {
        let response_data: DelongApiResponse<()> = response.json().await.map_err(|e| {
            error!("Failed to parse response from core service: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        info!(
            request_id = request_id,
            dataset_id = id,
            "Dynamic dataset deleted successfully"
        );

        Ok(Json(response_data))
    } else {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        error!(
            request_id = request_id,
            status = %status,
            error = error_text,
            "Core service returned error"
        );

        Err(map_core_service_status(status))
    }
}

/// Update static dataset
/// PUT /api/static-datasets/{id}
#[instrument(skip(state), fields(request_id))]
pub async fn update_static_dataset_handler(
    State(state): State<AppState>,
    Path(id): Path<u32>,
    Json(payload): Json<UpdateStaticDatasetRequest>,
) -> Result<Json<DelongApiResponse<StaticDatasetInfo>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        dataset_id = id,
        "Static dataset update request received"
    );

    // Forward to secure service
    let secure_service_url = &state.config.services.secure_url;

    let client = reqwest::Client::new();
    let response = client
        .put(&format!(
            "{}/api/static-datasets/{}",
            secure_service_url, id
        ))
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to forward request to secure service: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if response.status().is_success() {
        let response_data: DelongApiResponse<StaticDatasetInfo> =
            response.json().await.map_err(|e| {
                error!("Failed to parse response from secure service: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!(
            request_id = request_id,
            dataset_id = id,
            "Static dataset updated successfully"
        );

        Ok(Json(response_data))
    } else {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        error!(
            request_id = request_id,
            status = %status,
            error = error_text,
            "Secure service returned error"
        );

        Err(map_secure_service_status(status))
    }
}

/// Delete static dataset
/// DELETE /api/static-datasets/{id}
#[instrument(skip(state), fields(request_id))]
pub async fn delete_static_dataset_handler(
    State(state): State<AppState>,
    Path(id): Path<u32>,
) -> Result<Json<DelongApiResponse<()>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        dataset_id = id,
        "Static dataset delete request received"
    );

    // Forward to secure service
    let secure_service_url = &state.config.services.secure_url;

    let client = reqwest::Client::new();
    let response = client
        .delete(&format!(
            "{}/api/static-datasets/{}",
            secure_service_url, id
        ))
        .send()
        .await
        .map_err(|e| {
            error!("Failed to forward request to secure service: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if response.status().is_success() {
        let response_data: DelongApiResponse<()> = response.json().await.map_err(|e| {
            error!("Failed to parse response from secure service: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        info!(
            request_id = request_id,
            dataset_id = id,
            "Static dataset deleted successfully"
        );

        Ok(Json(response_data))
    } else {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        error!(
            request_id = request_id,
            status = %status,
            error = error_text,
            "Secure service returned error"
        );

        Err(map_secure_service_status(status))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_pagination_values() {
        assert_eq!(1, 1);
        assert_eq!(10, 10);
    }

    #[test]
    fn test_static_dataset_list_query_defaults() {
        let query_str = "";
        let query: StaticDatasetListQuery = serde_urlencoded::from_str(query_str).unwrap();
        assert_eq!(query.page, 1);
        assert_eq!(query.page_size, 10);
    }

    #[test]
    fn test_create_dataset_request_validation() {
        let request = CreateDatasetRequest {
            name: "test_dataset".to_string(),
            ui_name: "Test Dataset".to_string(),
            description: Some("A test dataset".to_string()),
        };

        assert_eq!(request.name, "test_dataset");
        assert_eq!(request.ui_name, "Test Dataset");
        assert!(request.description.is_some());
    }

    #[test]
    fn test_status_code_mapping() {
        assert_eq!(
            map_secure_service_status(reqwest::StatusCode::BAD_REQUEST),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            map_secure_service_status(reqwest::StatusCode::NOT_FOUND),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            map_secure_service_status(reqwest::StatusCode::INTERNAL_SERVER_ERROR),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
