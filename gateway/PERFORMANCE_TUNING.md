# Gateway Performance Tuning Guide

This guide provides performance optimization recommendations for the delong-protocol gateway and its observability stack.

## Table of Contents

1. [Gateway Application Tuning](#gateway-application-tuning)
2. [Vector Performance Optimization](#vector-performance-optimization)
3. [Loki Performance Tuning](#loki-performance-tuning)
4. [Jaeger Optimization](#jaeger-optimization)
5. [Docker Resource Management](#docker-resource-management)
6. [Production Best Practices](#production-best-practices)

## Gateway Application Tuning

### 1. Tracing Sampling

Reduce trace overhead by implementing sampling:

```rust
// In main.rs init_tracing() function
use opentelemetry_sdk::trace::{Sampler, SamplingDecision, SamplingResult, ShouldSample};
use opentelemetry::trace::{SpanKind, Link};

// Add sampling configuration
let sampler = Sampler::TraceIdRatioBased(0.1); // Sample 10% of traces

let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
    .with_batch_exporter(trace_exporter)
    .with_sampler(sampler)
    .with_resource(
        Resource::builder()
            .with_service_name("delong-gateway")
            .build(),
    )
    .build();
```

### 2. Log Level Optimization

Adjust log levels for production:

```rust
// Use environment-based log levels
let filter = EnvFilter::from_default_env()
    .or_else(|_| EnvFilter::try_new("gateway=info,tower=warn,hyper=warn"))
    .unwrap();
```

### 3. Connection Pool Settings

Optimize HTTP client connections:

```rust
// If using reqwest or similar HTTP clients
use std::time::Duration;

let client = reqwest::Client::builder()
    .pool_idle_timeout(Duration::from_secs(90))
    .pool_max_idle_per_host(32)
    .timeout(Duration::from_secs(30))
    .build()?;
```

## Vector Performance Optimization

### 1. Buffer Configuration

Optimize Vector's buffer settings in `vector.yaml`:

```yaml
# Global buffer configuration
data_dir: /var/lib/vector

# Per-sink buffer configuration
sinks:
  loki:
    type: loki
    inputs: ["process_logs"]
    endpoint: "http://loki:3100"
    encoding:
      codec: json
    
    # Buffer configuration
    buffer:
      type: disk
      max_size: 268435488  # 256MB
      when_full: drop_newest
    
    # Batch configuration
    batch:
      max_bytes: 1048576  # 1MB
      max_events: 1000
      timeout_secs: 1
    
    # Request configuration
    request:
      rate_limit_duration_secs: 1
      rate_limit_num: 10
      retry_attempts: 3
      retry_initial_backoff_secs: 1
      retry_max_duration_secs: 10
      timeout_secs: 30
      concurrency: 16
```

### 2. Transform Optimization

Optimize VRL transforms for better performance:

```yaml
transforms:
  process_logs:
    type: remap
    inputs: ["gateway_logs"]
    # Use abort-on-error for better performance
    drop_on_error: true
    drop_on_abort: true
    source: |
      # Cache parsed values
      msg = to_string(.message) ?? ""
      
      # Use conditional parsing only when needed
      if starts_with(msg, "{") {
        parsed = parse_json(msg) ?? {}
        
        # Extract only required fields
        .log_level = parsed.level
        .log_message = parsed.fields.message ?? parsed.message
        .log_timestamp = parsed.timestamp
        
        # Remove original message to reduce payload
        del(.message)
      }
      
      # Add minimal metadata
      .service = "delong-gateway"
      .env = "production"
```

### 3. Vector Resource Limits

Set appropriate resource limits:

```yaml
# In docker-compose.yml
vector:
  image: timberio/vector:0.47.0-debian
  deploy:
    resources:
      limits:
        cpus: '2'
        memory: 1G
      reservations:
        cpus: '0.5'
        memory: 256M
```

## Loki Performance Tuning

### 1. Loki Configuration Optimization

Update `loki-config.yaml`:

```yaml
auth_enabled: false

server:
  http_listen_port: 3100
  grpc_server_max_recv_msg_size: 8388608  # 8MB
  grpc_server_max_send_msg_size: 8388608  # 8MB

common:
  instance_addr: 127.0.0.1
  path_prefix: /loki
  storage:
    filesystem:
      chunks_directory: /loki/chunks
      rules_directory: /loki/rules
  replication_factor: 1
  ring:
    kvstore:
      store: inmemory

schema_config:
  configs:
    - from: 2020-10-24
      store: tsdb
      object_store: filesystem
      schema: v13
      index:
        prefix: index_
        period: 24h

ingester:
  chunk_idle_period: 30m
  chunk_retain_period: 15m
  max_chunk_age: 1h
  chunk_target_size: 1572864  # 1.5MB
  chunk_encoding: snappy
  max_transfer_retries: 0

storage_config:
  tsdb_shipper:
    active_index_directory: /loki/index
    cache_location: /loki/index_cache
    cache_ttl: 24h

limits_config:
  enforce_metric_name: false
  reject_old_samples: true
  reject_old_samples_max_age: 168h
  ingestion_rate_mb: 16
  ingestion_burst_size_mb: 32
  per_stream_rate_limit: 5MB
  per_stream_rate_limit_burst: 20MB
  max_query_series: 5000
  max_query_parallelism: 32

chunk_store_config:
  max_look_back_period: 0s

table_manager:
  retention_deletes_enabled: true
  retention_period: 168h  # 7 days
```

### 2. Loki Index Optimization

Add index labels carefully - too many labels can hurt performance:

```yaml
# In Vector's loki sink configuration
labels:
  service: "{{ service }}"
  environment: "{{ env }}"
  level: "{{ log_level }}"
  # Avoid high-cardinality labels like:
  # - request_id
  # - user_id
  # - ip_address
```

## Jaeger Optimization

### 1. Jaeger Memory and Storage

Configure Jaeger with appropriate limits:

```yaml
# In docker-compose.yml
jaeger:
  image: jaegertracing/all-in-one:1.54
  environment:
    - COLLECTOR_OTLP_ENABLED=true
    - SPAN_STORAGE_TYPE=badger
    - BADGER_EPHEMERAL=false
    - BADGER_DIRECTORY_VALUE=/badger/data
    - BADGER_DIRECTORY_KEY=/badger/key
    - BADGER_SPAN_STORE_TTL=72h
    - COLLECTOR_QUEUE_SIZE=5000
    - COLLECTOR_NUM_WORKERS=50
  volumes:
    - jaeger-storage:/badger
  deploy:
    resources:
      limits:
        cpus: '2'
        memory: 2G
```

### 2. Sampling Strategies

Configure adaptive sampling:

```json
{
  "service_strategies": [
    {
      "service": "delong-gateway",
      "type": "adaptive",
      "max_traces_per_second": 100,
      "sampling_rate": 0.1
    }
  ],
  "default_strategy": {
    "type": "probabilistic",
    "param": 0.01
  }
}
```

## Docker Resource Management

### 1. Docker Daemon Configuration

Optimize Docker daemon settings:

```json
{
  "log-driver": "json-file",
  "log-opts": {
    "max-size": "10m",
    "max-file": "3",
    "compress": "true"
  },
  "storage-driver": "overlay2",
  "storage-opts": [
    "overlay2.override_kernel_check=true"
  ]
}
```

### 2. Docker Compose Production Settings

Complete production docker-compose configuration:

```yaml
version: '3.8'

services:
  gateway:
    image: rust:1.85
    command: bash -c "cd /usr/src/app/gateway && cargo run --release"
    ports:
      - "8080:8080"
    volumes:
      - ..:/usr/src/app:ro
    environment:
      - RUST_LOG=info
      - RUST_BACKTRACE=1
      - OTEL_EXPORTER_OTLP_ENDPOINT=http://jaeger:4317
    deploy:
      resources:
        limits:
          cpus: '4'
          memory: 2G
        reservations:
          cpus: '1'
          memory: 512M
    restart: unless-stopped
    logging:
      driver: json-file
      options:
        max-size: "10m"
        max-file: "3"

  vector:
    image: timberio/vector:0.47.0-debian
    volumes:
      - ./vector.yaml:/etc/vector/vector.yaml:ro
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - vector-data:/var/lib/vector
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 1G
    restart: unless-stopped

  loki:
    image: grafana/loki:3.4.1
    user: "0:0"
    volumes:
      - ./loki-config.yaml:/etc/loki/local-config.yaml:ro
      - loki-storage:/loki
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 2G
    restart: unless-stopped

  jaeger:
    image: jaegertracing/all-in-one:1.54
    environment:
      - SPAN_STORAGE_TYPE=badger
      - BADGER_EPHEMERAL=false
      - BADGER_DIRECTORY_VALUE=/badger/data
      - BADGER_DIRECTORY_KEY=/badger/key
    volumes:
      - jaeger-storage:/badger
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 2G
    restart: unless-stopped

  grafana:
    image: grafana/grafana
    volumes:
      - grafana-storage:/var/lib/grafana
      - ./grafana/provisioning:/etc/grafana/provisioning:ro
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_PASSWORD:-admin}
      - GF_SERVER_ROOT_URL=${GRAFANA_ROOT_URL:-http://localhost:3000}
      - GF_INSTALL_PLUGINS=grafana-clock-panel,grafana-simple-json-datasource
    deploy:
      resources:
        limits:
          cpus: '1'
          memory: 512M
    restart: unless-stopped

volumes:
  vector-data:
  loki-storage:
  jaeger-storage:
  grafana-storage:
```

## Production Best Practices

### 1. Monitoring the Monitors

Add health checks and monitoring:

```yaml
# Health check endpoints
services:
  gateway:
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

  vector:
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8686/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  loki:
    healthcheck:
      test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:3100/ready"]
      interval: 30s
      timeout: 10s
      retries: 3
```

### 2. Backup and Retention

Configure data retention policies:

```bash
#!/bin/bash
# backup-observability-data.sh

# Backup Grafana dashboards and settings
docker run --rm -v gateway_grafana-storage:/data -v $(pwd):/backup alpine tar czf /backup/grafana-backup-$(date +%Y%m%d).tar.gz -C /data .

# Backup Loki data (if needed)
docker run --rm -v gateway_loki-storage:/data -v $(pwd):/backup alpine tar czf /backup/loki-backup-$(date +%Y%m%d).tar.gz -C /data .

# Clean old backups
find . -name "*-backup-*.tar.gz" -mtime +7 -delete
```

### 3. Security Considerations

1. **Use TLS** for all service communication in production
2. **Implement authentication** for Grafana and Jaeger UIs
3. **Restrict Vector permissions** - use read-only Docker socket access
4. **Network isolation** - use Docker networks to isolate services
5. **Secrets management** - use Docker secrets or environment variables for sensitive data

### 4. Scaling Considerations

For high-traffic scenarios:

1. **Horizontal scaling**: Deploy multiple Gateway instances behind a load balancer
2. **Loki clustering**: Consider using object storage (S3, GCS) for Loki chunks
3. **Jaeger backend**: Switch from all-in-one to distributed deployment with Elasticsearch
4. **Vector aggregation**: Deploy Vector as an aggregator with multiple agents
5. **Grafana HA**: Use Grafana with external database for high availability

## Monitoring Metrics

Key metrics to monitor:

1. **Gateway**: Request rate, error rate, response time, CPU, memory
2. **Vector**: Events processed/sec, buffer usage, dropped events
3. **Loki**: Ingestion rate, query performance, storage usage
4. **Jaeger**: Span ingestion rate, query latency, storage size
5. **System**: CPU, memory, disk I/O, network traffic

Create alerts for:
- High error rates (>1%)
- Service downtime
- Resource exhaustion (>80% CPU/memory)
- Slow queries (>5s)
- Failed health checks