#!/bin/bash

# Script to set up dstack proxy for testing
# This proxy allows HTTP access to dstack-simulator's Unix socket

set -e

PROXY_NAME="dstack-proxy"
PROXY_PORT="11010"
NETWORK="secure-dev-network"
VOLUME="secure_simulator_sockets_dev"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Setting up dstack proxy for testing...${NC}"

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo -e "${RED}Error: Docker is not running${NC}"
    exit 1
fi

# Check if secure-dstack-simulator-dev is running
if ! docker ps | grep -q "secure-dstack-simulator-dev"; then
    echo -e "${YELLOW}Warning: secure-dstack-simulator-dev is not running${NC}"
    echo "Please run: cd secure && docker-compose up -d dstack-simulator"
    exit 1
fi

# Check if proxy is already running
if docker ps | grep -q "$PROXY_NAME"; then
    echo -e "${YELLOW}Proxy is already running${NC}"
    echo "To restart, run: docker rm -f $PROXY_NAME && $0"
    exit 0
fi

# Remove old proxy container if it exists
if docker ps -a | grep -q "$PROXY_NAME"; then
    echo "Removing old proxy container..."
    docker rm -f "$PROXY_NAME" > /dev/null 2>&1
fi

# Start the proxy container
echo "Starting dstack proxy on port $PROXY_PORT..."
docker run -d \
    --name "$PROXY_NAME" \
    --rm \
    --network "$NETWORK" \
    -v "${VOLUME}:/sockets:ro" \
    -p "${PROXY_PORT}:8090" \
    alpine/socat \
    TCP-LISTEN:8090,fork,reuseaddr UNIX-CONNECT:/sockets/dstack.sock

# Wait for proxy to be ready
echo -n "Waiting for proxy to be ready"
for i in {1..10}; do
    if curl -s -X POST "http://localhost:${PROXY_PORT}/GetKey" \
        -H "Content-Type: application/json" \
        -d '{"path":"test","purpose":"test"}' > /dev/null 2>&1; then
        echo -e " ${GREEN}✓${NC}"
        break
    fi
    echo -n "."
    sleep 1
done

# Test the connection
echo "Testing proxy connection..."
if curl -s -X POST "http://localhost:${PROXY_PORT}/GetKey" \
    -H "Content-Type: application/json" \
    -d '{"path":"test","purpose":"test"}' | jq -e '.key' > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Proxy is working correctly${NC}"
    echo ""
    echo "You can now run tests with:"
    echo "  cd secure && cargo test"
    echo ""
    echo "Or set custom endpoint:"
    echo "  export DSTACK_SIMULATOR_ENDPOINT=http://localhost:${PROXY_PORT}"
    echo ""
    echo "To stop the proxy:"
    echo "  docker rm -f $PROXY_NAME"
else
    echo -e "${RED}✗ Proxy test failed${NC}"
    echo "Check logs with: docker logs $PROXY_NAME"
    exit 1
fi
