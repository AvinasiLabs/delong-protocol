# Gateway Benchmarks

This directory contains Criterion-based performance benchmarks for the Delong Protocol Gateway. These benchmarks measure the performance of various gateway endpoints and middleware components to ensure optimal performance.

## Overview

The benchmarks have been updated to work with the new trait-based HTTP client architecture, providing more accurate and consistent performance measurements.

## Key Improvements

### 🔧 Trait-Based HTTP Client Support

The benchmarks now use a dedicated `BenchmarkMockClient` instead of relying on actual backend services:

- **Consistent Performance**: Eliminates network variability by using mock responses
- **Fast Execution**: No actual HTTP requests to backend services
- **Reliable Results**: Predictable response times for accurate benchmarking
- **Isolated Testing**: Measures only gateway performance, not backend latency

### 📊 Updated API Endpoints

All benchmarks have been updated to use the current API structure:

- ✅ `/api/algo-exes` (Algorithm Executions)
- ✅ `/api/committee` (Committee Management)
- ✅ `/api/votes` (Voting System)
- ✅ `/api/auth/keys` (API Key Management)
- ✅ `/api/reports` (Report Upload)
- ✅ `/api/datasets` (Dynamic Datasets)
- ✅ `/api/static-datasets` (Static Datasets)

### 🎯 Comprehensive Test Coverage

The benchmark suite now includes:

1. **Router Creation**: Measures router initialization performance
2. **Health Endpoints**: Basic health check performance
3. **Dataset Operations**: Both static and dynamic dataset endpoints
4. **Algorithm Execution**: Submission and status checking
5. **Committee Management**: Member operations and checks
6. **Voting System**: Vote listing and duration setting
7. **Authentication**: API key operations
8. **Report Upload**: Test report submission
9. **Middleware Performance**: Request processing with all middleware
10. **JSON Processing**: Small and large payload handling
11. **Error Handling**: Various error scenarios
12. **Public Endpoints**: Sample data access

## Running Benchmarks

### Prerequisites

```bash
# Install Criterion dependencies (if needed)
cargo install cargo-criterion
```

### Run All Benchmarks

```bash
cargo bench
```

### Run Specific Benchmark Groups

```bash
# Router creation performance
cargo bench bench_router_creation

# Health endpoint performance
cargo bench bench_health_endpoints

# Dataset operations
cargo bench bench_dataset_endpoints

# Algorithm execution endpoints
cargo bench bench_algo_exe_endpoints

# Committee management
cargo bench bench_committee_endpoints

# Voting system
cargo bench bench_vote_endpoints

# Authentication
cargo bench bench_auth_endpoints

# Report upload
cargo bench bench_report_endpoints

# Middleware performance
cargo bench bench_middleware_performance

# JSON processing
cargo bench bench_json_processing

# Error handling
cargo bench bench_error_handling

# Public endpoints
cargo bench bench_public_endpoints
```

### Generate HTML Reports

```bash
# Run benchmarks with HTML report generation
cargo bench -- --output-format html
```

## Benchmark Architecture

### BenchmarkMockClient

The `BenchmarkMockClient` is a specialized implementation of the `BackendClient` trait designed for performance testing:

```rust
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
        // Returns consistent mock response
        Ok(json!({
            "items": [],
            "total": 0,
            "page": 1,
            "limit": 20,
            "total_pages": 0
        }))
    }
}
```

### Configuration

Benchmarks use a standardized configuration:

```rust
fn create_bench_config() -> GatewayConfig {
    let mut config = GatewayConfig::default();
    config.api.enable_api_key_validation = false; // Disable auth for benchmarks
    config
}
```

## Performance Considerations

### What's Measured

- **Gateway Processing Time**: Route matching, middleware execution, handler processing
- **Serialization/Deserialization**: JSON payload processing
- **Memory Allocation**: Object creation and cleanup
- **Middleware Overhead**: Authentication, logging, request ID generation

### What's NOT Measured

- **Network Latency**: Eliminated by using mock client
- **Backend Processing**: No actual backend calls
- **Database Operations**: Mocked responses
- **External Service Calls**: All mocked

## Interpreting Results

### Typical Performance Expectations

- **Health Endpoints**: < 1ms
- **Simple GET Requests**: < 5ms
- **JSON Processing (small)**: < 10ms
- **JSON Processing (large)**: < 50ms
- **Router Creation**: < 100ms

### Performance Regression Detection

Monitor these key metrics:

1. **Throughput**: Requests per second
2. **Latency**: Response time percentiles
3. **Memory Usage**: Allocation patterns
4. **CPU Usage**: Processing efficiency

## Continuous Integration

To integrate with CI/CD pipelines:

```bash
# Run benchmarks with machine-readable output
cargo bench -- --output-format json > benchmark_results.json

# Compare with baseline
cargo bench -- --save-baseline main
cargo bench -- --baseline main
```

## Troubleshooting

### Common Issues

1. **High Variance**: Ensure system is not under load during benchmarking
2. **Compilation Errors**: Check that all dependencies are up to date
3. **Missing Gnuplot**: Install gnuplot for graphical output or use plotters backend

### Performance Tips

- Run benchmarks on a dedicated machine
- Close other applications during benchmarking
- Use release mode for accurate performance measurements
- Run multiple iterations for statistical significance

## Contributing

When adding new benchmarks:

1. Use the `BenchmarkMockClient` for consistent results
2. Follow the existing naming conventions
3. Include both success and error scenarios
4. Document expected performance characteristics
5. Update this README with new benchmark descriptions

## Future Improvements

- [ ] Add memory usage benchmarks
- [ ] Implement concurrent request benchmarks
- [ ] Add WebSocket connection benchmarks
- [ ] Create baseline comparison automation
- [ ] Integrate with performance monitoring tools