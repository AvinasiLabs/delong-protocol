//! Benchmarks for API response and error handling performance
//!
//! These benchmarks measure the performance of API response creation,
//! error handling, and serialization operations to ensure efficient
//! request processing in production environments.

use common::{ApiError, ApiResponse, ResponseCode};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use serde_json;

/// Benchmark creating successful API responses
fn bench_api_response_creation(c: &mut Criterion) {
    let data = "sample response data";

    c.bench_function("api_response_success", |b| {
        b.iter(|| {
            let response = ApiResponse::success(black_box(data));
            black_box(response)
        })
    });

    c.bench_function("api_response_success_with_id", |b| {
        b.iter(|| {
            let response = ApiResponse::success_with_id(black_box(data), "req_123".to_string());
            black_box(response)
        })
    });

    c.bench_function("api_response_success_with_message_and_id", |b| {
        b.iter(|| {
            let response = ApiResponse::success_with_message_and_id(
                black_box(data),
                "Operation successful",
                "req_456".to_string(),
            );
            black_box(response)
        })
    });
}

/// Benchmark creating error API responses
fn bench_api_error_creation(c: &mut Criterion) {
    c.bench_function("api_response_error", |b| {
        b.iter(|| {
            let response: ApiResponse<()> = ApiResponse::error(
                black_box(ResponseCode::BadRequest),
                black_box("Invalid input"),
            );
            black_box(response)
        })
    });

    c.bench_function("api_response_convenience_errors", |b| {
        b.iter(|| {
            let response = ApiResponse::bad_request(black_box("Bad input"));
            black_box(response)
        })
    });

    c.bench_function("api_error_to_response", |b| {
        b.iter(|| {
            let error = ApiError::BadRequest(black_box("Invalid data".to_string()));
            let response = error.to_response();
            black_box(response)
        })
    });

    c.bench_function("api_error_to_response_with_id", |b| {
        b.iter(|| {
            let error = ApiError::InternalError(black_box("Server error".to_string()));
            let response = error.to_response_with_id("req_error_789".to_string());
            black_box(response)
        })
    });
}

/// Benchmark API response serialization
fn bench_api_response_serialization(c: &mut Criterion) {
    let success_response = ApiResponse::success("test data");
    let error_response: ApiResponse<()> = ApiResponse::bad_request("Invalid request");

    c.bench_function("serialize_success_response", |b| {
        b.iter(|| {
            let json = serde_json::to_string(black_box(&success_response)).unwrap();
            black_box(json)
        })
    });

    c.bench_function("serialize_error_response", |b| {
        b.iter(|| {
            let json = serde_json::to_string(black_box(&error_response)).unwrap();
            black_box(json)
        })
    });

    c.bench_function("serialize_to_vec_success", |b| {
        b.iter(|| {
            let bytes = serde_json::to_vec(black_box(&success_response)).unwrap();
            black_box(bytes)
        })
    });
}

/// Benchmark API response deserialization
fn bench_api_response_deserialization(c: &mut Criterion) {
    let success_json = r#"{
        "code": "SUCCESS",
        "data": "test data",
        "message": "Operation completed successfully",
        "timestamp": "2023-01-01T00:00:00Z"
    }"#;

    let error_json = r#"{
        "code": "BAD_REQUEST",
        "message": "Invalid request",
        "timestamp": "2023-01-01T00:00:00Z"
    }"#;

    c.bench_function("deserialize_success_response", |b| {
        b.iter(|| {
            let response: ApiResponse<String> =
                serde_json::from_str(black_box(success_json)).unwrap();
            black_box(response)
        })
    });

    c.bench_function("deserialize_error_response", |b| {
        b.iter(|| {
            let response: ApiResponse<()> = serde_json::from_str(black_box(error_json)).unwrap();
            black_box(response)
        })
    });
}

/// Benchmark response code operations
fn bench_response_code_operations(c: &mut Criterion) {
    let codes = vec![
        ResponseCode::Success,
        ResponseCode::BadRequest,
        ResponseCode::Unauthorized,
        ResponseCode::Forbidden,
        ResponseCode::NotFound,
        ResponseCode::InternalServerError,
    ];

    c.bench_function("response_code_to_status", |b| {
        b.iter(|| {
            for code in &codes {
                let status = black_box(code).to_status_code();
                black_box(status);
            }
        })
    });

    c.bench_function("response_code_serialization", |b| {
        b.iter(|| {
            for code in &codes {
                let json = serde_json::to_string(black_box(code)).unwrap();
                black_box(json);
            }
        })
    });
}

/// Benchmark error type conversions
fn bench_error_conversions(c: &mut Criterion) {
    let errors = vec![
        ApiError::BadRequest("Bad input".to_string()),
        ApiError::Unauthorized("Access denied".to_string()),
        ApiError::Forbidden("Permission denied".to_string()),
        ApiError::NotFound("Resource not found".to_string()),
        ApiError::InternalError("Server error".to_string()),
    ];

    c.bench_function("api_error_to_response_code", |b| {
        b.iter(|| {
            for error in &errors {
                let code = black_box(error).to_response_code();
                black_box(code);
            }
        })
    });

    c.bench_function("api_error_display", |b| {
        b.iter(|| {
            for error in &errors {
                let message = black_box(error).to_string();
                black_box(message);
            }
        })
    });
}

/// Benchmark bulk response operations (simulating high-throughput scenarios)
fn bench_bulk_operations(c: &mut Criterion) {
    c.bench_function("bulk_success_responses", |b| {
        b.iter(|| {
            let responses: Vec<_> = (0..1000)
                .map(|i| ApiResponse::success(format!("data_{}", i)))
                .collect();
            black_box(responses)
        })
    });

    c.bench_function("bulk_error_responses", |b| {
        b.iter(|| {
            let responses: Vec<_> = (0..1000)
                .map(|i| ApiResponse::bad_request(&format!("error_{}", i)))
                .collect();
            black_box(responses)
        })
    });

    c.bench_function("bulk_serialization", |b| {
        let responses: Vec<_> = (0..100)
            .map(|i| ApiResponse::success(format!("data_{}", i)))
            .collect();

        b.iter(|| {
            let serialized: Vec<_> = responses
                .iter()
                .map(|r| serde_json::to_string(black_box(r)).unwrap())
                .collect();
            black_box(serialized)
        })
    });
}

criterion_group!(
    benches,
    bench_api_response_creation,
    bench_api_error_creation,
    bench_api_response_serialization,
    bench_api_response_deserialization,
    bench_response_code_operations,
    bench_error_conversions,
    bench_bulk_operations
);
criterion_main!(benches);
