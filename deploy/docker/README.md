# DeLong Protocol Docker Deployment

This guide explains how to deploy the DeLong Protocol using Docker with the new Rust microservices architecture.

## Architecture Overview

The DeLong Protocol consists of four main services:

- **Gateway Service** (Port 8080): API routing, authentication, rate limiting
- **Core Service** (Port 8081): Non-sensitive operations, user management
- **Secure Service** (Port 8082): TEE-enabled secure computation
- **DataPipe Service** (Port 8018): Data pipeline operations (existing)

## Prerequisites

1. **Docker and Docker Compose** installed
2. **PostgreSQL** (replaces MySQL in the original Go version)
3. **Sepolia Test ETH** for blockchain operations (optional for local development)

## Quick Start

### 1. Setup Environment

```bash
cd deploy/docker
cp env.template .env
# Edit .env with your configuration
```

### 2. Local Development (with Anvil)

```bash
# Start all services with local blockchain
docker-compose -f docker-compose.local.yml up -d

# Check service status
docker-compose -f docker-compose.local.yml ps

# View logs
docker-compose -f docker-compose.local.yml logs -f
```

### 3. Production/Staging Deployment

```bash
# Start with pre-built images
docker-compose -f docker-compose.staging.yml up -d

# With custom images
DELONG_GATEWAY_IMAGE=myregistry/delong-gateway:v1.0.0 \
DELONG_CORE_IMAGE=myregistry/delong-core:v1.0.0 \
DELONG_SECURE_IMAGE=myregistry/delong-secure:v1.0.0 \
docker-compose -f docker-compose.staging.yml up -d
```

## Configuration

### Database Configuration (PostgreSQL)

```env
# PostgreSQL (replaces MySQL)
DATABASE_URL=postgresql://delong:delong_test_2025@postgres:5432/delong
POSTGRES_USER=delong
POSTGRES_PASSWORD=delong_test_2025
POSTGRES_DATABASE=delong
```

### Blockchain Configuration

For local development:
```env
ETH_HTTP_URL=http://anvil:8545
ETH_WS_URL=ws://anvil:8545
CHAIN_ID=31337
```

For Sepolia testnet:
```env
ETH_HTTP_URL=https://ethereum-sepolia-rpc.publicnode.com
ETH_WS_URL=wss://ethereum-sepolia-rpc.publicnode.com
CHAIN_ID=11155111
```

### TEE Configuration

For development (mock TEE):
```env
TEE_CLIENT_TYPE=mock
BLOCKCHAIN_CLIENT_TYPE=mock
```

For production (Phala Network):
```env
TEE_CLIENT_TYPE=phala
BLOCKCHAIN_CLIENT_TYPE=ethereum
```

## Service Details

### Gateway Service (Port 8080)

- **Purpose**: API routing and authentication
- **Health Check**: `GET /health`
- **Routes**: 
  - `/api/auth/*` → Core Service
  - `/api/static-datasets/*` → Secure Service
  - `/api/algorithms/*` → Secure Service

### Core Service (Port 8081)

- **Purpose**: Non-sensitive operations
- **Health Check**: `GET /health`
- **Database**: PostgreSQL with SQLx
- **Storage**: Local filesystem for dynamic datasets

### Secure Service (Port 8082)

- **Purpose**: TEE-enabled secure computation
- **Health Check**: `GET /health`
- **Special Features**:
  - Hardware-protected key vault
  - Algorithm execution runtime
  - Blockchain event synchronization
  - IPFS integration for encrypted data

## Building Custom Images

### Build Individual Services

```bash
# Build Gateway service
docker build -t delong-gateway:latest \
  --build-arg SERVICE=gateway \
  -f deploy/docker/Dockerfile .

# Build Core service
docker build -t delong-core:latest \
  --build-arg SERVICE=core \
  -f deploy/docker/Dockerfile .

# Build Secure service
docker build -t delong-secure:latest \
  --build-arg SERVICE=secure \
  -f deploy/docker/Dockerfile .
```

### Build All Services

```bash
# Using the build script
./scripts/build-docker-images.sh

# Or manually
docker-compose -f docker-compose.local.yml build
```

## Testing

### Run Tests in Docker

```bash
# Run all tests
docker-compose -f docker-compose.local.yml run --rm test-runner

# Run specific test
docker-compose -f docker-compose.local.yml run --rm test-runner \
  cargo test --package secure --test runtime_test

# Run tests with coverage
docker-compose -f docker-compose.local.yml run --rm test-runner \
  cargo test --workspace --verbose
```

### Integration Tests

```bash
# Start services
docker-compose -f docker-compose.local.yml up -d

# Wait for services to be ready
./scripts/wait-for-services.sh

# Run integration tests
docker-compose -f docker-compose.local.yml run --rm test-runner \
  cargo test --test integration_tests
```

## Monitoring and Debugging

### View Service Logs

```bash
# All services
docker-compose -f docker-compose.local.yml logs -f

# Specific service
docker-compose -f docker-compose.local.yml logs -f delong-gateway
docker-compose -f docker-compose.local.yml logs -f delong-core
docker-compose -f docker-compose.local.yml logs -f delong-secure
```

### Health Checks

```bash
# Gateway service
curl http://localhost:8080/health

# Core service (through gateway)
curl http://localhost:8080/api/core/health

# Secure service (through gateway)
curl http://localhost:8080/api/secure/health
```

### Debug Mode

```bash
# Enable debug logging
echo "RUST_LOG=debug" >> .env

# Restart services
docker-compose -f docker-compose.local.yml restart

# Enable debug TTY (staging only)
docker-compose -f docker-compose.staging.yml --profile debug up -d
# Access via http://localhost:7681
```

## Database Management

### Run Migrations

```bash
# Migrations run automatically on service startup
# To run manually:
docker-compose -f docker-compose.local.yml exec delong-core \
  sqlx migrate run --database-url $DATABASE_URL
```

### Access Database

```bash
# Connect to PostgreSQL
docker-compose -f docker-compose.local.yml exec postgres \
  psql -U delong -d delong

# View database status
docker-compose -f docker-compose.local.yml exec postgres \
  psql -U delong -d delong -c "\dt"
```

## Troubleshooting

### Common Issues

1. **Services not starting**
   - Check logs: `docker-compose logs <service-name>`
   - Verify environment variables in `.env`
   - Ensure PostgreSQL is running and accessible

2. **Database connection errors**
   - Verify `DATABASE_URL` format
   - Check PostgreSQL container health
   - Ensure migrations have run

3. **Blockchain connection issues**
   - Check `ETH_HTTP_URL` and `ETH_WS_URL`
   - Verify account has sufficient ETH
   - For Sepolia: check faucets for test ETH

4. **TEE/Secure service issues**
   - Verify `TEE_CLIENT_TYPE` setting
   - Check IPFS connectivity
   - Ensure proper volume mounts

### Performance Optimization

```bash
# Limit resource usage
docker-compose -f docker-compose.local.yml up -d \
  --scale delong-gateway=2 \
  --scale delong-core=2

# Monitor resource usage
docker stats

# Optimize for memory
echo "CARGO_BUILD_JOBS=2" >> .env
echo "RUST_MIN_STACK=4194304" >> .env
```

## Migration from Go Version

### Key Changes

1. **Database**: MySQL → PostgreSQL
2. **Architecture**: Monolith → Microservices
3. **Language**: Go → Rust
4. **API**: Same endpoints, different implementation
5. **TEE**: Enhanced security with hardware-protected keys

### Migration Steps

1. **Export data** from MySQL (if needed)
2. **Update environment** variables
3. **Run new services** with PostgreSQL
4. **Verify functionality** with integration tests
5. **Update client** configurations if needed

## Security Notes

- **Never commit `.env` files** to version control
- **Use secure passwords** for all services
- **Rotate keys regularly** in production
- **Monitor TEE attestation** in secure service
- **Keep Docker images updated** for security patches

## Production Deployment

### Using Docker Swarm

```bash
# Initialize swarm
docker swarm init

# Deploy stack
docker stack deploy -c docker-compose.staging.yml delong

# Scale services
docker service scale delong_delong-gateway=3
docker service scale delong_delong-core=2
```

### Using Kubernetes

```bash
# Convert docker-compose to k8s manifests
kompose convert -f docker-compose.staging.yml

# Apply manifests
kubectl apply -f .
```

The DeLong Protocol is now fully containerized with PostgreSQL, enhanced security, and a scalable microservices architecture. 