# DeLong Protocol Deployment Guide

## 🏗️ Architecture Overview

DeLong Protocol uses a **split deployment model** separating traditional services from TEE operations:

```
┌─────────────────────────────────┐    ┌─────────────────────────────────┐
│     Traditional Cloud Env       │    │      Phala Cloud TEE Env       │
│                                 │    │                                 │
│  ┌─────────┐  ┌─────────┐      │    │  ┌─────────┐                   │
│  │Gateway  │  │  Core   │      │◄──►│  │ Secure  │                   │
│  │ :11111  │  │ :11112  │      │    │  │ :11113  │                   │
│  └─────────┘  └─────────┘      │    │  └─────────┘                   │
│  ┌─────────┐  ┌─────────┐      │    │  ┌─────────┐                   │
│  │Postgres │  │  Redis  │      │    │  │  IPFS   │                   │
│  └─────────┘  └─────────┘      │    │  └─────────┘                   │
└─────────────────────────────────┘    └─────────────────────────────────┘
```

## 🚀 Quick Start

### 1. Environment Setup

```bash
# Copy and configure environment
cp env.example .env

# Edit .env with your settings
# Required: POSTGRES_PASSWORD, REDIS_PASSWORD, JWT_SECRET
```

### 2. Deploy Main Services

```bash
# Deploy Gateway, Core, PostgreSQL, Redis
docker-compose up -d

# Check status
docker-compose ps
curl http://localhost:11111/health  # Gateway
curl http://localhost:11112/health  # Core
```

### 3. Deploy Secure Service

For **Local TEE Simulation**:
```bash
# Set TEE environment variables
export CORE_SERVICE_URL="http://localhost:11112"
export GATEWAY_SERVICE_URL="http://localhost:11111"
export BLOCKCHAIN_RPC_URL="your-blockchain-rpc-url"
export JWT_SECRET="your-jwt-secret"

# Deploy locally
docker-compose -f docker-compose.secure.yml up -d
curl http://localhost:11113/health  # Check health
```

For **Phala Cloud TEE**:
```bash
# 1. Create .env file with TEE configuration
# 2. Upload .env and docker-compose.secure.yml to Phala Cloud
# 3. Deploy through Phala Cloud dashboard
# See: https://docs.phala.network/phala-cloud/phala-cloud-user-guides
```

## 📋 Service Ports

| Service | Port | Purpose |
|---------|------|---------|
| Gateway | 11111 | API Gateway |
| Core | 11112 | Business Logic |
| Secure | 11113 | TEE Operations |
| PostgreSQL | 5432 | Database |
| Redis | 6379 | Caching |
| IPFS | 5001/8080 | Storage |

## 🔧 Configuration

### Main Services (.env)
```bash
# Database
POSTGRES_PASSWORD=your_secure_password
DATABASE_URL=postgresql://delong_user:password@postgres:5432/delong_protocol

# Redis
REDIS_PASSWORD=your_redis_password
REDIS_URL=redis://:password@redis:6379

# JWT
JWT_SECRET=your-super-secret-jwt-key-minimum-32-characters

# Environment
ENVIRONMENT=production
RUST_LOG=info
```

### Secure Service (Environment Variables)
```bash
# TEE Configuration
TEE_ENABLED=true

# External Services
CORE_SERVICE_URL="https://core.yourdomain.com"
GATEWAY_SERVICE_URL="https://api.yourdomain.com"
BLOCKCHAIN_RPC_URL="your-blockchain-endpoint"

# Security
JWT_SECRET="same-as-main-services"
```

## 🌐 Production Deployment

### For Phala Cloud TEE

1. **Deploy main services** to your cloud provider (AWS/GCP/Azure)
2. **Create .env file** with production TEE configuration:
   ```bash
   CORE_SERVICE_URL="https://core.yourdomain.com"
   GATEWAY_SERVICE_URL="https://api.yourdomain.com"
   BLOCKCHAIN_RPC_URL="your-blockchain-endpoint"
   JWT_SECRET="your-jwt-secret"
   ```
3. **Upload to Phala Cloud**: Provide `.env` file and `docker-compose.secure.yml`
4. **Deploy through Phala Cloud dashboard**
5. **Reference**: https://docs.phala.network/phala-cloud/phala-cloud-user-guides

### SSL/TLS Setup

Configure reverse proxy (nginx/traefik) for HTTPS:
```nginx
server {
    listen 443 ssl;
    server_name api.yourdomain.com;

    location / {
        proxy_pass http://localhost:11111;
    }
}
```

## 🛠️ Operations

### Service Management
```bash
# Main services
docker-compose up -d        # Start
docker-compose down         # Stop
docker-compose logs -f      # View logs
docker-compose restart      # Restart

# Secure services (local TEE simulation)
docker-compose -f docker-compose.secure.yml up -d
docker-compose -f docker-compose.secure.yml down
docker-compose -f docker-compose.secure.yml logs -f

# Secure services (Phala Cloud TEE)
# Managed through Phala Cloud dashboard
# See: https://docs.phala.network/phala-cloud/phala-cloud-user-guides
```

### Database Operations
```bash
# Connect to database
docker-compose exec postgres psql -U delong_user -d delong_protocol

# Backup
docker-compose exec postgres pg_dump -U delong_user delong_protocol > backup.sql

# Restore
docker-compose exec -T postgres psql -U delong_user -d delong_protocol < backup.sql
```

### Health Checks
```bash
# Main services
curl http://localhost:11111/health  # Gateway
curl http://localhost:11112/health  # Core

# Secure service
curl http://localhost:11113/health  # Secure (local)
# or https://your-secure-domain.com/health (Phala Cloud)
```

## 🔒 Security Considerations

1. **Never commit secrets** to version control
2. **Use strong passwords** (32+ characters for encryption keys)
3. **Enable firewall** and limit port access
4. **Regular key rotation** for production
5. **Monitor TEE attestation** status
6. **Use HTTPS** in production
7. **Separate networks** for database access

## 🚨 Troubleshooting

### Common Issues

**Port conflicts:**
```bash
sudo netstat -tlnp | grep -E "(11111|11112|11113)"
```

**Docker issues:**
```bash
docker system prune -a
sudo systemctl restart docker
```

**Database connection:**
```bash
docker-compose exec postgres pg_isready -U delong_user
```

**Service logs:**
```bash
docker-compose logs gateway
docker-compose logs core
docker-compose -f docker-compose.secure.yml logs secure
```

## 📊 Monitoring

### Built-in Health Endpoints
- Gateway: `/health`
- Core: `/health`
- Secure: `/health`

### Optional Monitoring Stack
```bash
# Enable Prometheus and Grafana
docker-compose --profile monitoring up -d

# Access dashboards
# Prometheus: http://localhost:9090
# Grafana: http://localhost:3001 (admin/admin)
```

---

## ✅ Deployment Checklist

- [ ] Configure `.env` with secure passwords
- [ ] Deploy main services with `docker-compose up -d`
- [ ] Verify main services health checks
- [ ] Configure secure service environment variables
- [ ] Deploy secure service with TEE configuration
- [ ] Verify secure service health and TEE attestation
- [ ] Configure DNS and SSL certificates
- [ ] Set up monitoring and backups
- [ ] Test complete workflow
