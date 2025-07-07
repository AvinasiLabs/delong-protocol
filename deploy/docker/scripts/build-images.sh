#!/bin/bash

# DeLong Protocol Docker Image Build Script
# This script builds all service images for the DeLong Protocol

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$(dirname "$DOCKER_DIR")")"

# Default configuration
REGISTRY=""
TAG="latest"
PUSH=false
SERVICES=("gateway" "core" "secure")

# Parse command line arguments
usage() {
    echo "Usage: $0 [options]"
    echo "Options:"
    echo "  -r, --registry REGISTRY  Docker registry to use (optional)"
    echo "  -t, --tag TAG           Tag for the images (default: latest)"
    echo "  -p, --push              Push images to registry after building"
    echo "  -s, --service SERVICE   Build specific service only (gateway|core|secure)"
    echo "  -h, --help              Show this help message"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case $1 in
        -r|--registry)
            REGISTRY="$2"
            shift 2
            ;;
        -t|--tag)
            TAG="$2"
            shift 2
            ;;
        -p|--push)
            PUSH=true
            shift
            ;;
        -s|--service)
            SERVICES=("$2")
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

echo "🚀 DeLong Protocol Docker Image Builder"
echo "======================================="
echo "Registry: ${REGISTRY:-<none>}"
echo "Tag: $TAG"
echo "Push: $PUSH"
echo "Services: ${SERVICES[*]}"
echo "Project root: $PROJECT_ROOT"
echo ""

# Function to build a service image
build_service() {
    local service=$1
    local image_name="delong-$service"
    
    if [ -n "$REGISTRY" ]; then
        image_name="$REGISTRY/$image_name"
    fi
    
    local full_image_name="$image_name:$TAG"
    
    echo "🔨 Building $service service..."
    echo "Image: $full_image_name"
    
    # Build the image
    docker build \
        --build-arg SERVICE="$service" \
        --tag "$full_image_name" \
        --file "$DOCKER_DIR/Dockerfile" \
        "$PROJECT_ROOT"
    
    echo "✅ Built $full_image_name"
    
    # Push if requested
    if [ "$PUSH" = true ]; then
        echo "📤 Pushing $full_image_name..."
        docker push "$full_image_name"
        echo "✅ Pushed $full_image_name"
    fi
    
    echo ""
}

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "❌ Docker is not running. Please start Docker and try again."
    exit 1
fi

# Change to project root
cd "$PROJECT_ROOT"

# Build each service
for service in "${SERVICES[@]}"; do
    case $service in
        gateway|core|secure)
            build_service "$service"
            ;;
        *)
            echo "❌ Unknown service: $service"
            echo "Valid services: gateway, core, secure"
            exit 1
            ;;
    esac
done

echo "🎉 Build Complete!"
echo "=================="

# Show built images
echo "Built images:"
if [ -n "$REGISTRY" ]; then
    for service in "${SERVICES[@]}"; do
        echo "  $REGISTRY/delong-$service:$TAG"
    done
else
    for service in "${SERVICES[@]}"; do
        echo "  delong-$service:$TAG"
    done
fi

echo ""
echo "Next steps:"
echo "1. Test the images locally:"
echo "   docker run --rm delong-gateway:$TAG"
echo "2. Or start the full stack:"
echo "   cd $DOCKER_DIR"
echo "   docker-compose -f docker-compose.local.yml up -d"

if [ "$PUSH" = true ] && [ -n "$REGISTRY" ]; then
    echo "3. Images are available in registry: $REGISTRY"
fi 