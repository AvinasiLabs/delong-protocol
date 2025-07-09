#!/bin/bash

# Secure Service Integration Test Runner
# This script runs all integration tests for the Secure Service with proper setup and cleanup

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
TEST_DB_NAME="test_secure"
TEST_DB_USER="test"
TEST_DB_PASSWORD="test"
TEST_DB_HOST="localhost"
TEST_DB_PORT="5432"

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check if PostgreSQL is running
check_postgres() {
    print_status "Checking PostgreSQL connection..."
    if ! pg_isready -h $TEST_DB_HOST -p $TEST_DB_PORT -U $TEST_DB_USER -d postgres > /dev/null 2>&1; then
        print_error "PostgreSQL is not running or not accessible"
        print_status "Please start PostgreSQL and ensure it's accessible at $TEST_DB_HOST:$TEST_DB_PORT"
        exit 1
    fi
    print_success "PostgreSQL is running"
}

# Function to setup test database
setup_test_database() {
    print_status "Setting up test database..."
    
    # Drop test database if it exists
    if psql -h $TEST_DB_HOST -p $TEST_DB_PORT -U $TEST_DB_USER -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname='$TEST_DB_NAME'" | grep -q 1; then
        print_status "Dropping existing test database..."
        dropdb -h $TEST_DB_HOST -p $TEST_DB_PORT -U $TEST_DB_USER $TEST_DB_NAME
    fi
    
    # Create test database
    print_status "Creating test database..."
    createdb -h $TEST_DB_HOST -p $TEST_DB_PORT -U $TEST_DB_USER $TEST_DB_NAME
    
    # Run migrations if they exist
    if [ -d "migrations" ]; then
        print_status "Running database migrations..."
        export DATABASE_URL="postgresql://$TEST_DB_USER:$TEST_DB_PASSWORD@$TEST_DB_HOST:$TEST_DB_PORT/$TEST_DB_NAME"
        sqlx migrate run
    fi
    
    print_success "Test database setup complete"
}

# Function to cleanup test database
cleanup_test_database() {
    print_status "Cleaning up test database..."
    if psql -h $TEST_DB_HOST -p $TEST_DB_PORT -U $TEST_DB_USER -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname='$TEST_DB_NAME'" | grep -q 1; then
        dropdb -h $TEST_DB_HOST -p $TEST_DB_PORT -U $TEST_DB_USER $TEST_DB_NAME
        print_success "Test database cleaned up"
    fi
}

# Function to set environment variables
setup_environment() {
    print_status "Setting up test environment variables..."
    
    export TEST_DATABASE_URL="postgresql://$TEST_DB_USER:$TEST_DB_PASSWORD@$TEST_DB_HOST:$TEST_DB_PORT/$TEST_DB_NAME"
    export DATABASE_URL="$TEST_DATABASE_URL"
    export RUST_LOG=debug
    export TEST_MODE=true
    export BLOCKCHAIN_ENABLED=false
    export TEE_ENABLED=false
    export IPFS_MOCK_MODE=true
    export MOCK_EXTERNAL_SERVICES=true
    
    print_success "Environment variables set"
}

# Function to run specific test modules
run_test_module() {
    local module=$1
    print_status "Running $module tests..."
    
    if cargo test --test lib --features test-utils -- $module --nocapture; then
        print_success "$module tests passed"
        return 0
    else
        print_error "$module tests failed"
        return 1
    fi
}

# Function to run all tests
run_all_tests() {
    print_status "Running all integration tests..."
    
    local failed_tests=()
    
    # Test modules to run
    local test_modules=(
        "integration_test"
        "tee_test"
        "dataset_test"
        "algorithm_test"
        "blockchain_test"
        "e2e_test"
    )
    
    for module in "${test_modules[@]}"; do
        if ! run_test_module "$module"; then
            failed_tests+=("$module")
        fi
    done
    
    if [ ${#failed_tests[@]} -eq 0 ]; then
        print_success "All integration tests passed!"
        return 0
    else
        print_error "Failed test modules: ${failed_tests[*]}"
        return 1
    fi
}

# Function to show usage
show_usage() {
    echo "Usage: $0 [OPTIONS] [TEST_MODULE]"
    echo ""
    echo "Options:"
    echo "  -h, --help          Show this help message"
    echo "  -s, --setup-only    Only setup test environment, don't run tests"
    echo "  -c, --cleanup-only  Only cleanup test environment"
    echo "  -k, --keep-db       Keep test database after tests"
    echo "  --no-setup          Skip test database setup"
    echo ""
    echo "Test Modules:"
    echo "  integration_test    HTTP endpoint integration tests"
    echo "  tee_test           TEE functionality tests"
    echo "  dataset_test       Dataset service tests"
    echo "  algorithm_test     Algorithm execution tests"
    echo "  blockchain_test    Blockchain synchronization tests"
    echo "  e2e_test          End-to-end workflow tests"
    echo ""
    echo "Examples:"
    echo "  $0                 Run all tests"
    echo "  $0 integration_test Run only integration tests"
    echo "  $0 -s              Setup test environment only"
    echo "  $0 -c              Cleanup test environment only"
}

# Main execution
main() {
    local setup_only=false
    local cleanup_only=false
    local keep_db=false
    local no_setup=false
    local test_module=""
    
    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_usage
                exit 0
                ;;
            -s|--setup-only)
                setup_only=true
                shift
                ;;
            -c|--cleanup-only)
                cleanup_only=true
                shift
                ;;
            -k|--keep-db)
                keep_db=true
                shift
                ;;
            --no-setup)
                no_setup=true
                shift
                ;;
            *)
                if [[ -z "$test_module" ]]; then
                    test_module=$1
                else
                    print_error "Unknown option: $1"
                    show_usage
                    exit 1
                fi
                shift
                ;;
        esac
    done
    
    # Change to secure service directory
    cd "$(dirname "$0")"
    
    print_status "Starting Secure Service Integration Tests"
    print_status "Working directory: $(pwd)"
    
    # Cleanup only mode
    if [ "$cleanup_only" = true ]; then
        cleanup_test_database
        exit 0
    fi
    
    # Setup environment
    if [ "$no_setup" != true ]; then
        check_postgres
        setup_test_database
    fi
    
    setup_environment
    
    # Setup only mode
    if [ "$setup_only" = true ]; then
        print_success "Test environment setup complete"
        exit 0
    fi
    
    # Run tests
    local test_result=0
    
    if [[ -n "$test_module" ]]; then
        run_test_module "$test_module"
        test_result=$?
    else
        run_all_tests
        test_result=$?
    fi
    
    # Cleanup
    if [ "$keep_db" != true ]; then
        cleanup_test_database
    fi
    
    if [ $test_result -eq 0 ]; then
        print_success "Test execution completed successfully!"
    else
        print_error "Test execution failed!"
        exit 1
    fi
}

# Trap to ensure cleanup on exit
trap 'cleanup_test_database' EXIT

# Run main function
main "$@" 