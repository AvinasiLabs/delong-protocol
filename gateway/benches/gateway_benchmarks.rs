//! Criterion benchmarks for the Delong Protocol Gateway
//!
//! These benchmarks measure the performance of various gateway endpoints
//! and middleware components to ensure optimal performance.

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use gateway::{
    config::GatewayConfig,
    create_router,
    utils::http_client::{BackendClient, HttpClientError},
};
use serde_json::json;
use std::collections::HashMap;
use tokio::runtime::Runtime;
use tower::ServiceExt;

/// Fast mock HTTP client for benchmarks
/// This avoids network overhead and provides consistent performance measurements
pub struct BenchmarkMockClient;

#[async_trait]
impl BackendClient for BenchmarkMockClient {
    async fn forward_request_json(
        &self,
        _url: &str,
        _method: reqwest::Method,
        _payload: Option<serde_json::Value>,
        _headers: Option<HashMap<String, String>>,
    ) -> Result<serde_json::Value, HttpClientError> {
        // Return a consistent mock response for benchmarks
        Ok(json!({
            "items": [],
            "total": 0,
            "page": 1,
            "limit": 20,
            "total_pages": 0
        }))
    }
}

/// Helper function to create test configuration
fn create_bench_config() -> GatewayConfig {
    let mut config = GatewayConfig::default();
    config.api.enable_api_key_validation = false; // Disable auth for benchmarks
    config
}

/// Helper function to create JSON request
fn create_json_request(method: Method, uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// Benchmark router creation performance
fn bench_router_creation(c: &mut Criterion) {
    let config = create_bench_config();

    c.bench_function("router_creation", |b| {
        b.iter(|| {
            let router = create_router(black_box(&config), black_box(BenchmarkMockClient));
            black_box(router)
        });
    });
}

/// Benchmark health endpoint performance
fn bench_health_endpoints(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_bench_config();
    let client = BenchmarkMockClient;
    let app = create_router(&config, client);

    c.bench_function("health_endpoint", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();

                assert_eq!(response.status(), StatusCode::OK);
                black_box(response)
            })
        });
    });
}

/// Benchmark dataset API endpoints
fn bench_dataset_endpoints(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_bench_config();
    let client = BenchmarkMockClient;
    let app = create_router(&config, client);

    c.bench_function("get_static_dataset_list", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/static-datasets")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });

    c.bench_function("get_dynamic_dataset_list", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/datasets")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });

    c.bench_function("create_dynamic_dataset", |b| {
        b.iter(|| {
            rt.block_on(async {
                let payload = json!({
                    "name": "benchmark_dataset",
                    "description": "A benchmark test dataset",
                    "category": "test"
                });

                let request = create_json_request(Method::POST, "/api/datasets", payload);
                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });

    c.bench_function("get_specific_dataset", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/datasets/1")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });

    c.bench_function("delete_dataset", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .method(Method::DELETE)
                    .uri("/api/datasets/1")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });
}

/// Benchmark algorithm execution API endpoints
fn bench_algo_exe_endpoints(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_bench_config();
    let client = BenchmarkMockClient;
    let app = create_router(&config, client);

    c.bench_function("submit_algo_exe", |b| {
        b.iter(|| {
            rt.block_on(async {
                let payload = json!({
                    "github_repo": "example/test-repo",
                    "commit_hash": "abc123def456",
                    "scientist_wallet": "0x1234567890abcdef"
                });

                let request = create_json_request(Method::POST, "/api/algo-exes", payload);
                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });

    c.bench_function("get_algo_exe_list", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/algo-exes")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });

    c.bench_function("get_algo_exe_details", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/algo-exes/1")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });
}

/// Benchmark committee API endpoints
fn bench_committee_endpoints(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_bench_config();
    let client = BenchmarkMockClient;
    let app = create_router(&config, client);

    c.bench_function("get_committee_members", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/committee")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });

    c.bench_function("set_committee_member", |b| {
        b.iter(|| {
            rt.block_on(async {
                let payload = json!({
                    "member_wallet": "0x1234567890abcdef",
                    "is_approved": true
                });

                let request = create_json_request(Method::POST, "/api/committee", payload);
                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });

    c.bench_function("check_committee_membership", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/committee/check/0x1234567890abcdef")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });
}

/// Benchmark voting API endpoints
fn bench_vote_endpoints(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_bench_config();
    let client = BenchmarkMockClient;
    let app = create_router(&config, client);

    c.bench_function("get_votes", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/votes")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });

    c.bench_function("set_vote_duration", |b| {
        b.iter(|| {
            rt.block_on(async {
                let payload = json!({
                    "duration": 3600
                });

                let request = create_json_request(Method::POST, "/api/votes/duration", payload);
                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });
}

/// Benchmark authentication endpoints
fn bench_auth_endpoints(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_bench_config();
    let client = BenchmarkMockClient;
    let app = create_router(&config, client);

    c.bench_function("create_api_key", |b| {
        b.iter(|| {
            rt.block_on(async {
                let payload = json!({
                    "name": "bench_key",
                    "description": "A benchmark API key",
                    "permissions": ["read", "write"]
                });

                let request = create_json_request(Method::POST, "/api/auth/keys", payload);
                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });

    c.bench_function("list_api_keys", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/auth/keys")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });

    c.bench_function("validate_api_key", |b| {
        b.iter(|| {
            rt.block_on(async {
                let payload = json!({
                    "api_key": "bench-api-key-value"
                });

                let request = create_json_request(Method::POST, "/api/auth/keys/validate", payload);
                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });

    c.bench_function("revoke_api_key", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .method(Method::DELETE)
                    .uri("/api/auth/keys/bench-key-id")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });
}

/// Benchmark report upload endpoints
fn bench_report_endpoints(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_bench_config();
    let client = BenchmarkMockClient;
    let app = create_router(&config, client);

    c.bench_function("upload_report", |b| {
        b.iter(|| {
            rt.block_on(async {
                let payload = json!({
                    "report_type": "test_result",
                    "content": "Test report content",
                    "algorithm_id": "test-algo-123",
                    "dataset_id": "test-dataset-456"
                });

                let request = create_json_request(Method::POST, "/api/reports", payload);
                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });
}

/// Benchmark middleware performance by comparing requests with and without middleware
fn bench_middleware_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_bench_config();
    let client = BenchmarkMockClient;

    // Create app with all middleware
    let app_with_middleware = create_router(&config, client);

    c.bench_function("request_with_all_middleware", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/health")
                    .header("X-Forwarded-For", "192.168.1.100")
                    .header("User-Agent", "BenchmarkClient/1.0")
                    .header("X-Request-ID", "bench-request-123")
                    .body(Body::empty())
                    .unwrap();

                let response = app_with_middleware
                    .clone()
                    .oneshot(black_box(request))
                    .await
                    .unwrap();

                black_box(response)
            })
        });
    });
}

/// Benchmark JSON payload processing
fn bench_json_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_bench_config();
    let client = BenchmarkMockClient;
    let app = create_router(&config, client);

    // Small JSON payload
    c.bench_function("small_json_payload", |b| {
        b.iter(|| {
            rt.block_on(async {
                let payload = json!({
                    "name": "test",
                    "description": "Small test dataset"
                });

                let request = create_json_request(Method::POST, "/api/datasets", payload);
                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });

    // Large JSON payload
    c.bench_function("large_json_payload", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut large_data = std::collections::HashMap::new();
                for i in 0..1000 {
                    large_data.insert(format!("key_{}", i), format!("value_{}", i));
                }

                let payload = json!({
                    "name": "large_dataset",
                    "description": "A large benchmark dataset",
                    "metadata": large_data,
                    "category": "benchmark"
                });

                let request = create_json_request(Method::POST, "/api/datasets", payload);
                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            });
        })
    });
}

/// Benchmark error handling performance
fn bench_error_handling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_bench_config();
    let client = BenchmarkMockClient;
    let app = create_router(&config, client);

    c.bench_function("not_found_error", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/nonexistent")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });

    c.bench_function("method_not_allowed_error", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .method(Method::PATCH)
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });

    c.bench_function("invalid_json_error", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .method(Method::POST)
                    .uri("/api/datasets")
                    .header("content-type", "application/json")
                    .body(Body::from("invalid json {"))
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });
}

/// Benchmark sample data access (public endpoint)
fn bench_public_endpoints(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_bench_config();
    let client = BenchmarkMockClient;
    let app = create_router(&config, client);

    c.bench_function("sample_data_access", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/sample/QmTestCID123")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();
                black_box(response)
            })
        });
    });
}

criterion_group!(
    benches,
    bench_router_creation,
    bench_health_endpoints,
    bench_dataset_endpoints,
    bench_algo_exe_endpoints,
    bench_committee_endpoints,
    bench_vote_endpoints,
    bench_auth_endpoints,
    bench_report_endpoints,
    bench_middleware_performance,
    bench_json_processing,
    bench_error_handling,
    bench_public_endpoints
);

criterion_main!(benches);
