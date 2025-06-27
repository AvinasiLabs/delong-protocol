//! Criterion benchmarks for the Delong Protocol Gateway
//!
//! These benchmarks measure the performance of various gateway endpoints
//! and middleware components to ensure optimal performance.

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use gateway::{config::GatewayConfig, create_router};
use serde_json::json;
use tokio::runtime::Runtime;
use tower::ServiceExt;

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
            let router = create_router(black_box(&config));
            black_box(router)
        });
    });
}

/// Benchmark health endpoint performance
fn bench_health_endpoints(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_bench_config();
    let app = create_router(&config);

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

    c.bench_function("liveness_endpoint", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/health/live")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();

                black_box(response)
            })
        });
    });

    c.bench_function("readiness_endpoint", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/health/ready")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();

                black_box(response)
            })
        });
    });
}

/// Benchmark dataset API endpoints
fn bench_dataset_endpoints(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_bench_config();
    let app = create_router(&config);

    c.bench_function("get_dataset_list", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/v1/datasets")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();

                black_box(response)
            })
        });
    });

    c.bench_function("upload_dataset", |b| {
        b.iter(|| {
            rt.block_on(async {
                let payload = json!({
                    "name": "benchmark_dataset",
                    "description": "A benchmark test dataset",
                    "data": "sample_benchmark_data"
                });

                let request = create_json_request(Method::POST, "/api/v1/datasets", payload);
                let response = app.clone().oneshot(black_box(request)).await.unwrap();

                black_box(response)
            })
        });
    });

    c.bench_function("get_specific_dataset", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/v1/datasets/bench-dataset-id")
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
                    .uri("/api/v1/datasets/bench-dataset-id")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();

                black_box(response)
            })
        });
    });
}

/// Benchmark algorithm API endpoints
fn bench_algorithm_endpoints(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = create_bench_config();
    let app = create_router(&config);

    c.bench_function("submit_algorithm", |b| {
        b.iter(|| {
            rt.block_on(async {
                let payload = json!({
                    "algorithm_type": "privacy_preserving_ml",
                    "dataset_id": "bench-dataset-id",
                    "parameters": {
                        "epsilon": 1.0,
                        "delta": 0.001
                    }
                });

                let request =
                    create_json_request(Method::POST, "/api/v1/algorithms/submit", payload);
                let response = app.clone().oneshot(black_box(request)).await.unwrap();

                black_box(response)
            })
        });
    });

    c.bench_function("get_algorithm_status", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/v1/algorithms/bench-algo-id/status")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();

                black_box(response)
            })
        });
    });

    c.bench_function("get_algorithm_result", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/v1/algorithms/bench-algo-id/result")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(black_box(request)).await.unwrap();

                black_box(response)
            })
        });
    });

    c.bench_function("get_algorithm_details", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/v1/algorithms/bench-algo-id")
                    .body(Body::empty())
                    .unwrap();

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
    let app = create_router(&config);

    c.bench_function("create_api_key", |b| {
        b.iter(|| {
            rt.block_on(async {
                let payload = json!({
                    "name": "bench_key",
                    "description": "A benchmark API key",
                    "permissions": ["read", "write"]
                });

                let request = create_json_request(Method::POST, "/api/v1/auth/keys", payload);
                let response = app.clone().oneshot(black_box(request)).await.unwrap();

                black_box(response)
            })
        });
    });

    c.bench_function("list_api_keys", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/v1/auth/keys")
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

                let request =
                    create_json_request(Method::POST, "/api/v1/auth/keys/validate", payload);
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
                    .uri("/api/v1/auth/keys/bench-key-id")
                    .body(Body::empty())
                    .unwrap();

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

    // Create app with all middleware
    let app_with_middleware = create_router(&config);

    c.bench_function("request_with_all_middleware", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/health")
                    .header("X-Forwarded-For", "192.168.1.100")
                    .header("User-Agent", "BenchmarkClient/1.0")
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
    let app = create_router(&config);

    // Small JSON payload
    c.bench_function("small_json_payload", |b| {
        b.iter(|| {
            rt.block_on(async {
                let payload = json!({
                    "name": "test",
                    "value": 42
                });

                let request = create_json_request(Method::POST, "/api/v1/datasets", payload);
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
                    "data": large_data,
                    "metadata": {
                        "size": large_data.len(),
                        "created_at": "2024-01-01T00:00:00Z",
                        "tags": ["benchmark", "performance", "test"]
                    }
                });

                let request = create_json_request(Method::POST, "/api/v1/datasets", payload);
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
    let app = create_router(&config);

    c.bench_function("not_found_error", |b| {
        b.iter(|| {
            rt.block_on(async {
                let request = Request::builder()
                    .uri("/api/v1/nonexistent")
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
                    .uri("/api/v1/datasets")
                    .header("content-type", "application/json")
                    .body(Body::from("invalid json {"))
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
    bench_algorithm_endpoints,
    bench_auth_endpoints,
    bench_middleware_performance,
    bench_json_processing,
    bench_error_handling
);

criterion_main!(benches);
