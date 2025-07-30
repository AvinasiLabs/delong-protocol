#!/bin/bash

# Development startup script for secure service
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "🚀 Starting development environment for secure service..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# Check if docker and docker-compose are installed
if ! command -v docker &> /dev/null; then
    echo -e "${RED}❌ Docker is not installed. Please install Docker first.${NC}"
    exit 1
fi

if ! command -v docker-compose &> /dev/null; then
    echo -e "${RED}❌ Docker Compose is not installed. Please install Docker Compose first.${NC}"
    exit 1
fi

# Navigate to project directory
cd "$PROJECT_DIR"

# Stop any existing containers
echo -e "${YELLOW}🛑 Stopping existing containers...${NC}"
docker-compose -f docker-compose.dev.yml down

# Start services
echo -e "${GREEN}🐳 Starting Docker services...${NC}"
docker-compose -f docker-compose.dev.yml up -d

# Wait for services to be healthy
echo -e "${YELLOW}⏳ Waiting for services to be ready...${NC}"

# Wait for PostgreSQL
echo -n "Waiting for PostgreSQL..."
until docker exec secure-postgres-dev pg_isready -U secure_user -d secure_db &>/dev/null; do
    echo -n "."
    sleep 1
done
echo -e " ${GREEN}✓${NC}"

# Wait for Redis
echo -n "Waiting for Redis..."
until docker exec secure-redis-dev redis-cli --raw ping &>/dev/null; do
    echo -n "."
    sleep 1
done
echo -e " ${GREEN}✓${NC}"

# Wait for Foundry (Anvil)
echo -n "Waiting for Anvil..."
until curl -s -X POST http://localhost:8545 -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' &>/dev/null; do
    echo -n "."
    sleep 1
done
echo -e " ${GREEN}✓${NC}"

# Wait for IPFS
echo -n "Waiting for IPFS..."
until curl -s http://localhost:5001/api/v0/version &>/dev/null; do
    echo -n "."
    sleep 1
done
echo -e " ${GREEN}✓${NC}"

# Run database migrations if they exist
if [ -f "$PROJECT_DIR/migrations/run.sh" ]; then
    echo -e "${GREEN}🗄️  Running database migrations...${NC}"
    "$PROJECT_DIR/migrations/run.sh"
elif command -v diesel &> /dev/null && [ -f "$PROJECT_DIR/diesel.toml" ]; then
    echo -e "${GREEN}🗄️  Running Diesel migrations...${NC}"
    diesel migration run
else
    echo -e "${YELLOW}⚠️  No migration tool found. Skipping migrations.${NC}"
fi

# Load environment variables
if [ -f "$PROJECT_DIR/.env.dev" ]; then
    echo -e "${GREEN}📝 Loading environment variables from .env.dev${NC}"
    export $(cat "$PROJECT_DIR/.env.dev" | grep -v '^#' | xargs)
else
    echo -e "${YELLOW}⚠️  No .env.dev file found. Using default environment variables.${NC}"
fi

# Build the project
echo -e "${GREEN}🔨 Building the project...${NC}"
cargo build

# Display service URLs
echo -e "\n${GREEN}✅ Development environment is ready!${NC}"
echo -e "\n📋 Service URLs:"
echo -e "  - PostgreSQL: ${GREEN}postgres://secure_user:secure_dev_password@localhost:5432/secure_db${NC}"
echo -e "  - Redis: ${GREEN}redis://:secure_redis_dev_password@localhost:6379${NC}"
echo -e "  - Anvil RPC: ${GREEN}http://localhost:8545${NC} (HTTP & WebSocket)"
echo -e "  - IPFS API: ${GREEN}http://localhost:5001${NC}"
echo -e "  - IPFS Gateway: ${GREEN}http://localhost:8080${NC}"

echo -e "\n📝 Available accounts (Anvil):"
echo -e "  Mnemonic: ${YELLOW}test test test test test test test test test test test junk${NC}"
echo -e "  Balance: ${GREEN}10000 ETH each${NC}"

echo -e "\n🏃 To start the secure service, run:"
echo -e "  ${GREEN}cargo run${NC}"

echo -e "\n🛑 To stop all services, run:"
echo -e "  ${GREEN}docker-compose -f docker-compose.dev.yml down${NC}"

echo -e "\n📊 To view logs, run:"
echo -e "  ${GREEN}docker-compose -f docker-compose.dev.yml logs -f [service_name]${NC}"
