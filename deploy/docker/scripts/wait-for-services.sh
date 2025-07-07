#!/bin/bash

# DeLong Protocol Service Health Check Script
# This script waits for all services to be healthy before proceeding

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$(dirname "$SCRIPT_DIR")"

# Default configuration
TIMEOUT=300  # 5 minutes
CHECK_INTERVAL=5  # 5 seconds
COMPOSE_FILE="docker-compose.local.yml"

# Service endpoints to check
SERVICES=(
    "delong-gateway:8080:http://localhost:8080/health"
    "delong-core:8081:http://localhost:8081/health"
    "delong-secure:8082:http://localhost:8082/health"
)

# Parse command line arguments
usage() {
    echo "Usage: $0 [options]"
    echo "Options:"
    echo "  -t, --timeout SECONDS       Timeout in seconds (default: 300)"
    echo "  -i, --interval SECONDS      Check interval in seconds (default: 5)"
    echo "  -f, --compose-file FILE     Docker compose file (default: docker-compose.local.yml)"
    echo "  -h, --help                  Show this help message"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        -i|--interval)
            CHECK_INTERVAL="$2"
            shift 2
            ;;
        -f|--compose-file)
            COMPOSE_FILE="$2"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo "Unknown option: $1"
            usage
            ;;
    esac
done

echo "🕐 Waiting for DeLong Protocol services to be healthy"
echo "===================================================="
echo "Timeout: ${TIMEOUT}s"
echo "Check interval: ${CHECK_INTERVAL}s"
echo "Compose file: $COMPOSE_FILE"
echo ""

# Change to docker directory
cd "$DOCKER_DIR"

# Function to check if a service is healthy
check_service() {
    local service_name=$1
    local port=$2
    local health_url=$3
    
    # Check if service is running
    if ! docker-compose -f "$COMPOSE_FILE" ps "$service_name" | grep -q "Up"; then
        echo "❌ $service_name is not running"
        return 1
    fi
    
    # Check health endpoint
    if curl -f -s "$health_url" > /dev/null 2>&1; then
        echo "✅ $service_name is healthy"
        return 0
    else
        echo "⏳ $service_name is not ready yet"
        return 1
    fi
}

# Main waiting loop
start_time=$(date +%s)
all_healthy=false

while [ $all_healthy = false ]; do
    current_time=$(date +%s)
    elapsed=$((current_time - start_time))
    
    # Check timeout
    if [ $elapsed -ge $TIMEOUT ]; then
        echo "❌ Timeout reached after ${elapsed}s"
        echo "Services that are not healthy:"
        for service_info in "${SERVICES[@]}"; do
            IFS=':' read -r service_name port health_url <<< "$service_info"
            if ! check_service "$service_name" "$port" "$health_url"; then
                echo "  - $service_name ($health_url)"
            fi
        done
        exit 1
    fi
    
    # Check all services
    healthy_count=0
    echo "⏳ Checking services... (${elapsed}s elapsed)"
    
    for service_info in "${SERVICES[@]}"; do
        IFS=':' read -r service_name port health_url <<< "$service_info"
        if check_service "$service_name" "$port" "$health_url"; then
            healthy_count=$((healthy_count + 1))
        fi
    done
    
    # Check if all services are healthy
    if [ $healthy_count -eq ${#SERVICES[@]} ]; then
        all_healthy=true
        echo ""
        echo "🎉 All services are healthy!"
        echo "=========================="
        break
    fi
    
    echo "($healthy_count/${#SERVICES[@]} services healthy)"
    echo ""
    
    # Wait before next check
    sleep $CHECK_INTERVAL
done

# Show final status
echo "Service Status:"
echo "==============="
for service_info in "${SERVICES[@]}"; do
    IFS=':' read -r service_name port health_url <<< "$service_info"
    echo "✅ $service_name: $health_url"
done

echo ""
echo "🚀 DeLong Protocol is ready!"
echo "=========================="
echo "Gateway API: http://localhost:8080"
echo "Health checks:"
echo "  - Gateway: http://localhost:8080/health"
echo "  - Core: http://localhost:8081/health"
echo "  - Secure: http://localhost:8082/health"
echo ""
echo "You can now run integration tests or start using the API." 