//! Handlers for test report operations.
use crate::{
    models::{
        report::{CreateTestReportRequest, TestReport, TestReportInfo},
        AppState, BlockchainTransaction, ENTITY_TYPE_TEST_REPORT,
    },
    services::key_ctx::{KeyContext, KeyKind},
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use common::{ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginationParams};
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info, instrument};

#[instrument(skip_all, fields(file_hash = %req.file_hash, user = %req.user_wallet))]
pub async fn create_report(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateTestReportRequest>,
) -> ApiResult<Json<ApiResponse<TestReportInfo>>> {
    info!("Starting test report creation process");

    let mut tx = state.db.begin().await?;

    let new_report = match async {
        // 1. (Simulated) Register on blockchain
        info!("Registering test report on blockchain...");
        let key_context = KeyContext::new(
            KeyKind::EthAccount,
            &req.user_wallet,
            "Registering a test report",
        );
        let tx_hash = state
            .ctr_caller_service
            .register_test_report(&req.user_wallet, &req.file_hash, &req.raw_report_cid, &key_context)
            .await?;

        // 2. Create DB entry for the report
        info!("Creating test report record in database...");
        let report = TestReport::create_in_tx(&mut tx, req.clone()).await?;

        // 3. Create corresponding blockchain transaction record
        info!("Logging blockchain transaction...");
        let args = json!({
            "file_hash": req.file_hash,
            "raw_report_cid": req.raw_report_cid,
            "user_wallet": req.user_wallet,
        });
        BlockchainTransaction::create(&mut tx, &tx_hash, report.id, ENTITY_TYPE_TEST_REPORT, &args)
            .await?;

        Ok::<_, ApiError>(report)
    }
    .await
    {
        Ok(report) => report,
        Err(e) => {
            error!("Error during report creation, rolling back: {}", e);
            tx.rollback().await?;
            return Err(e);
        }
    };

    tx.commit().await?;

    info!(
        "Successfully created test report with ID: {}",
        new_report.id
    );
    Ok(Json(ApiResponse::success(new_report.into())))
}

#[instrument(skip(state), fields(id = %id))]
pub async fn get_report_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<Json<ApiResponse<TestReportInfo>>> {
    info!("Getting report by ID");
    let report = TestReport::find_by_id(&state.db, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(ApiResponse::success(report.into())))
}

#[instrument(skip(state), fields(user_wallet = %user_wallet))]
pub async fn list_reports_by_user(
    State(state): State<Arc<AppState>>,
    Path(user_wallet): Path<String>,
    Query(params): Query<PaginationParams>,
) -> ApiResult<Json<ApiResponse<PaginatedResponse<TestReportInfo>>>> {
    info!("Listing reports by user");
    let (reports, total) = TestReport::find_by_user(
        &state.db,
        &user_wallet,
        params.page as i64,
        params.limit as i64,
    )
    .await?;
    let response = PaginatedResponse::new(
        reports.into_iter().map(Into::into).collect(),
        params.page,
        params.limit,
        total as u64,
    );
    Ok(Json(ApiResponse::success(response)))
}

#[instrument(skip(state), fields(dataset = %dataset))]
pub async fn list_reports_by_dataset(
    State(state): State<Arc<AppState>>,
    Path(dataset): Path<String>,
    Query(params): Query<PaginationParams>,
) -> ApiResult<Json<ApiResponse<PaginatedResponse<TestReportInfo>>>> {
    info!("Listing reports by dataset");
    let (reports, total) = TestReport::find_by_dataset(
        &state.db,
        &dataset,
        params.page as i64,
        params.limit as i64,
    )
    .await?;
    let response = PaginatedResponse::new(
        reports.into_iter().map(Into::into).collect(),
        params.page,
        params.limit,
        total as u64,
    );
    Ok(Json(ApiResponse::success(response)))
}

#[instrument(skip(state))]
pub async fn list_all_reports(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> ApiResult<Json<ApiResponse<PaginatedResponse<TestReportInfo>>>> {
    info!("Listing all reports");
    let (reports, total) =
        TestReport::find_all(&state.db, params.page as i64, params.limit as i64).await?;
    let response = PaginatedResponse::new(
        reports.into_iter().map(Into::into).collect(),
        params.page,
        params.limit,
        total as u64,
    );
    Ok(Json(ApiResponse::success(response)))
} 