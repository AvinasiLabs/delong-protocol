# Gateway Observability Stack Troubleshooting Guide

This guide helps diagnose and resolve common issues with the delong-protocol gateway and its observability components.

## Table of Contents

1. [Quick Health Check](#quick-health-check)
2. [Gateway Issues](#gateway-issues)
3. [Vector Issues](#vector-issues)
4. [Loki Issues](#loki-issues)
5. [Jaeger Issues](#jaeger-issues)
6. [Grafana Issues](#grafana-issues)
7. [Common Problems and Solutions](#common-problems-and-solutions)
8. [Performance Troubleshooting](#performance-troubleshooting)
9. [Debug Commands](#debug-commands)

## Quick Health Check

Run this script to quickly check the health of all services:

```bash
#!/bin/bash
# health-check.sh

echo "Checking service status..."
docker-compose ps

echo -e "\n📡 Gateway Health:"
curl -s http://localhost:8080/health || echo "❌ Gateway not responding"

echo -e "\n📊 Vector Health:"
curl -s http://localhost:8686/health || echo "❌ Vector not responding"

echo -e "\n📝 Loki Health:"
curl -s http://localhost:3100/ready || echo "❌ Loki not responding"

echo -e "\n🔍 Jaeger Health:"
curl -s http://localhost:16686/ > /dev/null && echo "✅ Jaeger UI accessible" || echo "❌ Jaeger not responding"

echo -e "\n📈 Grafana Health:"
curl -s http://localhost:3000/api/health || echo "❌ Grafana not responding"
```

## Gateway Issues

### Gateway Not Starting

**Symptoms:**
- Container exits immediately
- No logs in stdout

**Diagnosis:**
```bash
# Check container logs
docker-compose logs gateway

# Check if port 8080 is already in use
lsof -i :8080

# Check compilation errors
docker-compose run --rm gateway cargo check
```

**Solutions:**
1. **Port conflict**: Change the port in docker-compose.yml
2. **Compilation error**: Fix Rust code errors
3. **Missing dependencies**: Run `cargo update`

### No Traces Appearing in Jaeger

**Symptoms:**
- Gateway runs but no traces in Jaeger UI
- Service name not appearing in Jaeger

**Diagnosis:**
```bash
# Check OTLP endpoint configuration
docker-compose exec gateway env | grep OTEL

# Test Jaeger connectivity
docker-compose exec gateway curl -v telnet://jaeger:4317

# Check Gateway logs for OTLP errors
docker-compose logs gateway | grep -i "otlp\|opentelemetry\|trace"
```

**Solutions:**
1. **Wrong endpoint**: Ensure `OTEL_EXPORTER_OTLP_ENDPOINT=http://jaeger:4317`
2. **Network issue**: Check Docker network connectivity
3. **Sampling**: Verify sampling is not set to 0%

## Vector Issues

### Vector Not Collecting Logs

**Symptoms:**
- No logs appearing in Loki
- Vector running but not processing events

**Diagnosis:**
```bash
# Check Vector status
docker-compose exec vector vector top

# Check Vector configuration
docker-compose exec vector vector validate /etc/vector/vector.yaml

# Check Docker socket permissions
docker-compose exec vector ls -la /var/run/docker.sock

# View Vector internal logs
docker-compose logs vector
```

**Solutions:**
1. **Container name mismatch**: Update container name in vector.yaml
   ```yaml
   sources:
     gateway_logs:
       type: docker_logs
       include_containers:
         - "gateway-gateway-1"  # Must match actual container name
   ```

2. **Docker socket permission**: Ensure Vector has access to Docker socket
   ```yaml
   volumes:
     - /var/run/docker.sock:/var/run/docker.sock:ro
   ```

3. **Transform errors**: Check VRL syntax in transforms

### Vector Memory/CPU Usage High

**Symptoms:**
- High resource consumption
- Slow log processing

**Solutions:**
1. Add buffering limits:
   ```yaml
   sinks:
     loki:
       buffer:
         type: memory
         max_events: 10000
         when_full: drop_newest
   ```

2. Enable sampling or filtering (see Performance Tuning guide)

## Loki Issues

### Loki Permission Denied Errors

**Symptoms:**
- Error: `mkdir /loki/rules: permission denied`
- Loki container exits

**Diagnosis:**
```bash
# Check Loki logs
docker-compose logs loki

# Check volume permissions
docker-compose exec loki ls -la /loki
```

**Solutions:**
1. Run Loki as root (development only):
   ```yaml
   loki:
     user: "0:0"
   ```

2. Fix volume permissions:
   ```bash
   docker-compose down
   docker volume rm gateway_loki-storage
   docker-compose up -d
   ```

### Loki Query Errors

**Symptoms:**
- "too many outstanding requests" error
- Slow queries or timeouts

**Solutions:**
1. Increase limits in loki-config.yaml:
   ```yaml
   limits_config:
     max_query_parallelism: 32
     max_outstanding_requests_per_tenant: 2048
   ```

2. Reduce query time range or add more specific filters

## Jaeger Issues

### Jaeger UI Not Loading

**Symptoms:**
- Cannot access http://localhost:16686
- Connection refused

**Diagnosis:**
```bash
# Check if Jaeger is running
docker-compose ps jaeger

# Check port binding
docker-compose port jaeger 16686

# Check Jaeger logs
docker-compose logs jaeger
```

**Solutions:**
1. **Port conflict**: Change port mapping in docker-compose.yml
2. **Container not running**: `docker-compose restart jaeger`

### Traces Not Persisting

**Symptoms:**
- Traces disappear after Jaeger restart
- Storage errors in logs

**Solutions:**
1. Enable persistent storage:
   ```yaml
   jaeger:
     environment:
       - SPAN_STORAGE_TYPE=badger
       - BADGER_EPHEMERAL=false
       - BADGER_DIRECTORY_VALUE=/badger/data
     volumes:
       - jaeger-storage:/badger
   ```

## Grafana Issues

### Cannot Login to Grafana

**Symptoms:**
- Login fails with default credentials
- "Invalid username or password"

**Solutions:**
1. Reset admin password:
   ```bash
   docker-compose exec grafana grafana-cli admin reset-admin-password newpassword
   ```

2. Check environment variable:
   ```yaml
   environment:
     - GF_SECURITY_ADMIN_PASSWORD=admin
   ```

### Datasources Not Working

**Symptoms:**
- "Datasource not found" errors
- Cannot query Loki or Jaeger

**Diagnosis:**
```bash
# Check provisioned datasources
docker-compose exec grafana ls -la /etc/grafana/provisioning/datasources/

# Test Loki connectivity from Grafana
docker-compose exec grafana curl http://loki:3100/ready

# Test Jaeger connectivity from Grafana
docker-compose exec grafana curl http://jaeger:16686/
```

**Solutions:**
1. Ensure datasources.yaml is properly mounted
2. Restart Grafana: `docker-compose restart grafana`
3. Check datasource URLs use internal Docker hostnames

## Common Problems and Solutions

### 1. No Logs Appearing Anywhere

**Quick Fix Checklist:**
- [ ] Gateway is producing logs: `docker-compose logs gateway`
- [ ] Vector is running: `docker-compose ps vector`
- [ ] Loki is healthy: `curl http://localhost:3100/ready`
- [ ] Correct container name in Vector config
- [ ] Grafana datasource is configured

### 2. High Disk Usage

**Check disk usage:**
```bash
# Check Docker volumes
docker system df

# Check specific volumes
docker volume ls
docker volume inspect gateway_loki-storage

# Clean up old data
docker system prune -a --volumes
```

### 3. Services Keep Restarting

**Check resource limits:**
```bash
# View resource usage
docker stats

# Check OOM kills
docker-compose logs | grep -i "killed\|oom"

# Increase memory limits in docker-compose.yml
```

### 4. Slow Performance

**Quick optimizations:**
1. Reduce log verbosity: `RUST_LOG=warn`
2. Enable Vector buffering
3. Add Loki retention policies
4. Implement trace sampling

## Performance Troubleshooting

### Identifying Bottlenecks

```bash
# Monitor resource usage in real-time
docker stats

# Check Vector metrics
curl -s http://localhost:8686/metrics | grep vector_

# Check Loki metrics
curl -s http://localhost:3100/metrics | grep loki_

# Analyze Gateway performance
docker-compose exec gateway cargo install flamegraph
docker-compose exec gateway cargo flamegraph --bin gateway
```

### Common Performance Issues

1. **Too many labels in Loki**: Reduce label cardinality
2. **Vector buffer overflow**: Increase buffer size or enable dropping
3. **Jaeger query timeouts**: Add indexes or reduce retention
4. **Gateway high CPU**: Enable sampling, reduce log level

## Debug Commands

### Useful debugging commands:

```bash
# Follow all logs
docker-compose logs -f

# Check network connectivity between services
docker-compose exec gateway ping loki
docker-compose exec vector ping loki
docker-compose exec grafana wget -O- http://loki:3100/ready

# Inspect Vector pipeline
docker-compose exec vector vector tap 'process_logs'

# Test Loki queries
curl -G -s "http://localhost:3100/loki/api/v1/query_range" \
  --data-urlencode 'query={service="delong-gateway"}' \
  --data-urlencode 'limit=10' | jq .

# List Jaeger services
curl -s "http://localhost:16686/api/services" | jq .

# Get Jaeger traces for a service
curl -s "http://localhost:16686/api/traces?service=delong-gateway&limit=10" | jq .

# Export Grafana dashboards
curl -s -u admin:admin http://localhost:3000/api/dashboards/uid/gateway-monitoring | jq .

# Check container health
docker inspect gateway-gateway-1 | jq '.[0].State.Health'
```

### Enable Debug Logging

For more detailed troubleshooting:

```yaml
# Gateway debug logging
environment:
  - RUST_LOG=debug,hyper=info,tower=info
  - RUST_BACKTRACE=full

# Vector debug logging
vector:
  command: -c /etc/vector/vector.yaml -vv

# Loki debug logging
loki:
  command: -config.file=/etc/loki/local-config.yaml -log.level=debug
```

## Getting Help

If you're still experiencing issues:

1. Check container logs: `docker-compose logs [service]`
2. Verify configuration files are valid (YAML syntax)
3. Ensure all services are on the same Docker network
4. Check for resource constraints (disk space, memory)
5. Review the Performance Tuning guide for optimization tips

For persistent issues, collect:
- Output of `docker-compose logs`
- Service configuration files
- Output of health check script
- System resource usage (`docker stats`)