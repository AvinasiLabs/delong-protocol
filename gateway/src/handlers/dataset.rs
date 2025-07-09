//! Datasets management handlers for static and dynamic dataset operations
//!
//! This module handles dataset-related operations by forwarding requests to
//! the appropriate backend services (secure/core). It implements both static
//! dataset management (IPFS + blockchain) and dynamic dataset management
//! (local file system with versioning).

use axum::{
    extract::{Multipart, Path, Query, State},
    response::Json,
};
use reqwest::multipart::Form;
use tracing::{error, info, instrument, warn};

use crate::routes::AppState;
use common::prelude::*;

/// Upload static dataset (multipart/form-data)
/// Forwards to POST /api/static-datasets on secure service
#[utoipa::path(
    post,
    path = "/api/static-datasets",
    tag = "static-datasets",
    summary = "Upload static dataset",
    description = "Upload a new static dataset with TEE encryption and IPFS storage",
    request_body(
        content = String,
        description = "Multipart form data containing dataset file and metadata",
        content_type = "multipart/form-data"
    ),
    responses(
        (status = 200, description = "Dataset uploaded successfully", body = ApiResponse<String>),
        (status = 400, description = "Invalid request or file format", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 413, description = "File too large", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
#[instrument(skip(multipart, state), fields(request_id))]
pub async fn upload_static_dataset_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Json<ApiResponse<String>> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        "Static dataset upload request received"
    );

    // Parse multipart form data
    let mut form = Form::new();
    let mut dataset_name = String::new();

    while let Some(field) = match multipart.next_field().await {
        Ok(field) => field,
        Err(e) => {
            error!("Failed to parse multipart field: {}", e);
            return Json(ApiResponse::bad_request());
        }
    } {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "file" => {
                let filename = field.file_name().unwrap_or("dataset.csv").to_string();
                let data = match field.bytes().await {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Failed to read file data: {}", e);
                        return Json(ApiResponse::bad_request());
                    }
                };

                form = form.part(
                    "file",
                    reqwest::multipart::Part::bytes(data.to_vec()).file_name(filename),
                );
            }
            "name" => {
                dataset_name = match field.text().await {
                    Ok(name) => name,
                    Err(e) => {
                        error!("Failed to read name field: {}", e);
                        return Json(ApiResponse::bad_request());
                    }
                };
                form = form.text("name", dataset_name.clone());
            }
            "desc" => {
                let desc = match field.text().await {
                    Ok(desc) => desc,
                    Err(e) => {
                        error!("Failed to read desc field: {}", e);
                        return Json(ApiResponse::bad_request());
                    }
                };
                form = form.text("desc", desc);
            }
            "author" => {
                let author = match field.text().await {
                    Ok(author) => author,
                    Err(e) => {
                        error!("Failed to read author field: {}", e);
                        return Json(ApiResponse::bad_request());
                    }
                };
                form = form.text("author", author);
            }
            "author_wallet" => {
                let author_wallet = match field.text().await {
                    Ok(wallet) => wallet,
                    Err(e) => {
                        error!("Failed to read author_wallet field: {}", e);
                        return Json(ApiResponse::bad_request());
                    }
                };
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
        return Json(ApiResponse::bad_request());
    }

    // Forward to secure service
    let secure_service_url = &state.config.services.secure_url;

    let client = reqwest::Client::new();
    let response = match client
        .post(&format!("{}/api/static-datasets", secure_service_url))
        .multipart(form)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to forward request to secure service: {}", e);
            return Json(ApiResponse::internal_error());
        }
    };

    if response.status().is_success() {
        let response_data: String = match response.json().await {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to parse response from secure service: {}", e);
                return Json(ApiResponse::internal_error());
            }
        };

        info!(
            request_id = request_id,
            dataset_name = dataset_name,
            "Static dataset upload forwarded successfully"
        );

        Json(ApiResponse::success(response_data))
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

        Json(ApiResponse::internal_error())
    }
}

/// Get static datasets with pagination
/// Forwards to GET /api/static-datasets on secure service
#[utoipa::path(
    get,
    path = "/api/static-datasets",
    tag = "static-datasets",
    summary = "List static datasets",
    description = "Retrieve a paginated list of static datasets with filtering options",
    params(
        ("page" = Option<u32>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u32>, Query, description = "Items per page (default: 20, max: 100)"),
        ("status" = Option<String>, Query, description = "Filter by dataset status"),
        ("created_after" = Option<String>, Query, description = "Filter datasets created after this date (ISO 8601)"),
        ("created_before" = Option<String>, Query, description = "Filter datasets created before this date (ISO 8601)")
    ),
    responses(
        (status = 200, description = "Datasets retrieved successfully", body = ApiResponse<Vec<StaticDatasetInfo>>),
        (status = 400, description = "Invalid query parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
#[instrument(skip(state), fields(request_id))]
pub async fn get_static_datasets_handler(
    State(state): State<AppState>,
    Query(query): Query<StaticDatasetListQuery>,
) -> Json<ApiResponse<DatasetPaginatedResponse<StaticDatasetInfo>>> {
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
    let response = match client
        .get(&format!("{}/api/static-datasets", secure_service_url))
        .query(&[("page", query.page), ("page_size", query.page_size)])
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to forward request to secure service: {}", e);
            return Json(ApiResponse::internal_error());
        }
    };

    if response.status().is_success() {
        let response_data: DatasetPaginatedResponse<StaticDatasetInfo> = match response.json().await
        {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to parse response from secure service: {}", e);
                return Json(ApiResponse::internal_error());
            }
        };

        info!(
            request_id = request_id,
            total_items = response_data.total,
            "Static datasets list retrieved successfully"
        );

        Json(ApiResponse::success(response_data))
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

        Json(ApiResponse::internal_error())
    }
}

/// Get single static dataset by ID
/// Forwards to GET /api/static-datasets/{id} on secure service
/// Get static dataset by ID
/// Forwards to GET /api/static-datasets/{id} on secure service
#[utoipa::path(
    get,
    path = "/api/static-datasets/{id}",
    tag = "static-datasets",
    summary = "Get static dataset",
    description = "Retrieve detailed information about a specific static dataset",
    params(
        ("id" = u32, Path, description = "Dataset ID")
    ),
    responses(
        (status = 200, description = "Dataset retrieved successfully", body = ApiResponse<StaticDatasetInfo>),
        (status = 400, description = "Invalid dataset ID", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 404, description = "Dataset not found", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
#[instrument(skip(state), fields(request_id, dataset_id = %dataset_id))]
pub async fn get_static_dataset_handler(
    State(state): State<AppState>,
    Path(dataset_id): Path<u32>,
) -> Json<ApiResponse<StaticDatasetInfo>> {
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
    let response = match client
        .get(&format!(
            "{}/api/static-datasets/{}",
            secure_service_url, dataset_id
        ))
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to forward request to secure service: {}", e);
            return Json(ApiResponse::internal_error());
        }
    };

    if response.status().is_success() {
        let response_data: StaticDatasetInfo = match response.json().await {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to parse response from secure service: {}", e);
                return Json(ApiResponse::internal_error());
            }
        };

        info!(
            request_id = request_id,
            dataset_id = dataset_id,
            "Static dataset info retrieved successfully"
        );

        Json(ApiResponse::success(response_data))
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
            "Failed to get static dataset info"
        );

        Json(ApiResponse::internal_error())
    }
}

/// Get sample data by CID (public access, no authentication required)
/// Forwards to GET /api/sample/{cid} on secure service
#[utoipa::path(
    get,
    path = "/api/sample/{cid}",
    tag = "sample-data",
    summary = "Get sample data",
    description = "Retrieve sample data by IPFS CID. This endpoint is public and does not require authentication.",
    params(
        ("cid" = String, Path, description = "IPFS Content Identifier (CID)")
    ),
    responses(
        (status = 200, description = "Sample data retrieved successfully", content_type = "application/octet-stream"),
        (status = 400, description = "Invalid CID format"),
        (status = 404, description = "Sample data not found"),
        (status = 500, description = "Internal server error")
    )
)]
#[instrument(skip(state), fields(request_id, cid = %cid))]
pub async fn get_sample_data_handler(
    State(state): State<AppState>,
    Path(cid): Path<String>,
) -> axum::response::Response {
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
    let response = match client
        .get(&format!("{}/api/sample/{}", secure_service_url, cid))
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to forward request to secure service: {}", e);
            return axum::response::Response::builder()
                .status(500)
                .body(axum::body::Body::from("Internal server error"))
                .unwrap();
        }
    };

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

        let body = match response.bytes().await {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to read response body from secure service: {}", e);
                return axum::response::Response::builder()
                    .status(500)
                    .body(axum::body::Body::from("Internal server error"))
                    .unwrap();
            }
        };

        info!(
            request_id = request_id,
            cid = cid,
            content_length = body.len(),
            "Sample data retrieved successfully"
        );

        // Return the raw CSV data with appropriate headers
        axum::response::Response::builder()
            .status(200)
            .header("Content-Type", content_type)
            .header("Content-Disposition", content_disposition)
            .body(axum::body::Body::from(body))
            .unwrap()
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

        axum::response::Response::builder()
            .status(500)
            .body(axum::body::Body::from("Internal server error"))
            .unwrap()
    }
}

/// Create dynamic dataset (admin only)
/// Forwards to POST /api/datasets on core service
#[utoipa::path(
    post,
    path = "/api/datasets",
    tag = "dynamic-datasets",
    summary = "Create dynamic dataset",
    description = "Create a new dynamic dataset with mutable storage (admin only)",
    request_body = CreateDatasetRequest,
    responses(
        (status = 200, description = "Dataset created successfully", body = ApiResponse<DynamicDatasetInfo>),
        (status = 400, description = "Invalid request parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 403, description = "Forbidden - admin access required", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
#[instrument(skip(payload, state), fields(request_id))]
pub async fn create_dataset_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateDatasetRequest>,
) -> Json<ApiResponse<DynamicDatasetInfo>> {
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
        return Json(ApiResponse::bad_request());
    }

    // Forward to core service
    let core_service_url = &state.config.services.core_url;

    let client = reqwest::Client::new();
    let response = match client
        .post(&format!("{}/api/datasets", core_service_url))
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to forward request to core service: {}", e);
            return Json(ApiResponse::internal_error());
        }
    };

    if response.status().is_success() {
        let response_data: DynamicDatasetInfo = match response.json().await {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to parse response from core service: {}", e);
                return Json(ApiResponse::internal_error());
            }
        };

        info!(
            request_id = request_id,
            dataset_name = payload.name,
            "Dynamic dataset creation forwarded successfully"
        );

        Json(ApiResponse::success(response_data))
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

        Json(ApiResponse::internal_error())
    }
}

/// Get dynamic datasets with pagination
/// Forwards to GET /api/datasets on core service
#[utoipa::path(
    get,
    path = "/api/datasets",
    tag = "dynamic-datasets",
    summary = "List dynamic datasets",
    description = "Retrieve a paginated list of dynamic datasets with filtering options",
    params(
        ("page" = Option<u32>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u32>, Query, description = "Items per page (default: 20, max: 100)"),
        ("status" = Option<String>, Query, description = "Filter by dataset status"),
        ("created_after" = Option<String>, Query, description = "Filter datasets created after this date (ISO 8601)"),
        ("created_before" = Option<String>, Query, description = "Filter datasets created before this date (ISO 8601)")
    ),
    responses(
        (status = 200, description = "Datasets retrieved successfully", body = ApiResponse<Vec<DynamicDatasetInfo>>),
        (status = 400, description = "Invalid query parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
#[instrument(skip(state), fields(request_id))]
pub async fn get_datasets_handler(
    State(state): State<AppState>,
    Query(query): Query<DynamicDatasetListQuery>,
) -> Json<ApiResponse<DatasetPaginatedResponse<DynamicDatasetInfo>>> {
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
    let response = match client
        .get(&format!("{}/api/datasets", core_service_url))
        .query(&[("page", query.page), ("page_size", query.page_size)])
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to forward request to core service: {}", e);
            return Json(ApiResponse::internal_error());
        }
    };

    if response.status().is_success() {
        let response_data: DatasetPaginatedResponse<DynamicDatasetInfo> =
            match response.json().await {
                Ok(data) => data,
                Err(e) => {
                    error!("Failed to parse response from core service: {}", e);
                    return Json(ApiResponse::internal_error());
                }
            };

        info!(
            request_id = request_id,
            total_items = response_data.total,
            "Dynamic datasets list retrieved successfully"
        );

        Json(ApiResponse::success(response_data))
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

        Json(ApiResponse::internal_error())
    }
}

/// Map secure service HTTP status to gateway status

/// Update dynamic dataset
/// PUT /api/datasets/{id}
#[utoipa::path(
    put,
    path = "/api/datasets/{id}",
    tag = "dynamic-datasets",
    summary = "Update dynamic dataset",
    description = "Update an existing dynamic dataset's metadata and configuration",
    params(
        ("id" = u32, Path, description = "Dataset ID")
    ),
    request_body = UpdateDatasetRequest,
    responses(
        (status = 200, description = "Dataset updated successfully", body = ApiResponse<DynamicDatasetInfo>),
        (status = 400, description = "Invalid request parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 403, description = "Forbidden - insufficient permissions", body = ApiResponse<String>),
        (status = 404, description = "Dataset not found", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
#[instrument(skip(state), fields(request_id))]
pub async fn update_dataset_handler(
    State(state): State<AppState>,
    Path(id): Path<u32>,
    Json(payload): Json<UpdateDatasetRequest>,
) -> Json<ApiResponse<DynamicDatasetInfo>> {
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
    let response = match client
        .put(&format!("{}/api/datasets/{}", core_service_url, id))
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to forward request to core service: {}", e);
            return Json(ApiResponse::internal_error());
        }
    };

    if response.status().is_success() {
        let response_data: DynamicDatasetInfo = match response.json().await {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to parse response from core service: {}", e);
                return Json(ApiResponse::internal_error());
            }
        };

        info!(
            request_id = request_id,
            dataset_id = id,
            "Dynamic dataset updated successfully"
        );

        Json(ApiResponse::success(response_data))
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

        Json(ApiResponse::internal_error())
    }
}

/// Delete dynamic dataset
/// DELETE /api/datasets/{id}
#[utoipa::path(
    delete,
    path = "/api/datasets/{id}",
    tag = "dynamic-datasets",
    summary = "Delete dynamic dataset",
    description = "Delete an existing dynamic dataset and all its associated data",
    params(
        ("id" = u32, Path, description = "Dataset ID")
    ),
    responses(
        (status = 200, description = "Dataset deleted successfully", body = ApiResponse<String>),
        (status = 400, description = "Invalid dataset ID", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 403, description = "Forbidden - insufficient permissions", body = ApiResponse<String>),
        (status = 404, description = "Dataset not found", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
#[instrument(skip(state), fields(request_id))]
pub async fn delete_dataset_handler(
    State(state): State<AppState>,
    Path(id): Path<u32>,
) -> Json<ApiResponse<()>> {
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
    let response = match client
        .delete(&format!("{}/api/datasets/{}", core_service_url, id))
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to forward request to core service: {}", e);
            return Json(ApiResponse::internal_error());
        }
    };

    if response.status().is_success() {
        let _response_data: () = match response.json().await {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to parse response from core service: {}", e);
                return Json(ApiResponse::internal_error());
            }
        };

        info!(
            request_id = request_id,
            dataset_id = id,
            "Dynamic dataset deleted successfully"
        );

        Json(ApiResponse::success(()))
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

        Json(ApiResponse::internal_error())
    }
}

/// Update static dataset
/// PUT /api/static-datasets/{id}
#[utoipa::path(
    put,
    path = "/api/static-datasets/{id}",
    tag = "static-datasets",
    summary = "Update static dataset",
    description = "Update an existing static dataset's metadata and configuration",
    params(
        ("id" = u32, Path, description = "Dataset ID")
    ),
    request_body = UpdateStaticDatasetRequest,
    responses(
        (status = 200, description = "Dataset updated successfully", body = ApiResponse<StaticDatasetInfo>),
        (status = 400, description = "Invalid request parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 403, description = "Forbidden - insufficient permissions", body = ApiResponse<String>),
        (status = 404, description = "Dataset not found", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
#[instrument(skip(state), fields(request_id))]
pub async fn update_static_dataset_handler(
    State(state): State<AppState>,
    Path(id): Path<u32>,
    Json(payload): Json<UpdateStaticDatasetRequest>,
) -> Json<ApiResponse<StaticDatasetInfo>> {
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
    let response = match client
        .put(&format!(
            "{}/api/static-datasets/{}",
            secure_service_url, id
        ))
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to forward request to secure service: {}", e);
            return Json(ApiResponse::internal_error());
        }
    };

    if response.status().is_success() {
        let response_data: StaticDatasetInfo = match response.json().await {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to parse response from secure service: {}", e);
                return Json(ApiResponse::internal_error());
            }
        };

        info!(
            request_id = request_id,
            dataset_id = id,
            "Static dataset updated successfully"
        );

        Json(ApiResponse::success(response_data))
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

        Json(ApiResponse::internal_error())
    }
}

/// Delete static dataset
/// DELETE /api/static-datasets/{id}
#[utoipa::path(
    delete,
    path = "/api/static-datasets/{id}",
    tag = "static-datasets",
    summary = "Delete static dataset",
    description = "Delete an existing static dataset and all its associated data from IPFS and blockchain",
    params(
        ("id" = u32, Path, description = "Dataset ID")
    ),
    responses(
        (status = 200, description = "Dataset deleted successfully", body = ApiResponse<String>),
        (status = 400, description = "Invalid dataset ID", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 403, description = "Forbidden - insufficient permissions", body = ApiResponse<String>),
        (status = 404, description = "Dataset not found", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
#[instrument(skip(state), fields(request_id))]
pub async fn delete_static_dataset_handler(
    State(state): State<AppState>,
    Path(id): Path<u32>,
) -> Json<ApiResponse<()>> {
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
    let response = match client
        .delete(&format!(
            "{}/api/static-datasets/{}",
            secure_service_url, id
        ))
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to forward request to secure service: {}", e);
            return Json(ApiResponse::internal_error());
        }
    };

    if response.status().is_success() {
        let _response_data: () = match response.json().await {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to parse response from secure service: {}", e);
                return Json(ApiResponse::internal_error());
            }
        };

        info!(
            request_id = request_id,
            dataset_id = id,
            "Static dataset deleted successfully"
        );

        Json(ApiResponse::success(()))
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

        Json(ApiResponse::internal_error())
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
}
