#!/bin/bash

# Algorithm Execution API Test Script
# This script tests the secure service API endpoints

set -e

# Configuration
API_BASE_URL="http://localhost:8090"
CURL_OPTS="--noproxy localhost"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counter
TESTS_PASSED=0
TESTS_FAILED=0

# Function to print test header
print_test_header() {
    echo -e "\n${BLUE}=== $1 ===${NC}"
}

# Function to print success
print_success() {
    echo -e "${GREEN}✓ $1${NC}"
    ((TESTS_PASSED++))
}

# Function to print failure
print_failure() {
    echo -e "${RED}✗ $1${NC}"
    ((TESTS_FAILED++))
}

# Function to make API request
api_request() {
    local method=$1
    local endpoint=$2
    local data=$3
    local expected_status=$4

    if [ -z "$data" ]; then
        response=$(curl $CURL_OPTS -s -w "\n%{http_code}" -X $method "$API_BASE_URL$endpoint")
    else
        response=$(curl $CURL_OPTS -s -w "\n%{http_code}" -X $method \
            -H "Content-Type: application/json" \
            -d "$data" \
            "$API_BASE_URL$endpoint")
    fi

    body=$(echo "$response" | sed '$d')
    status=$(echo "$response" | tail -1)

    if [ "$status" = "$expected_status" ]; then
        print_success "$method $endpoint - Status: $status"
        echo "$body" | jq -C '.' 2>/dev/null || echo "$body"
        return 0
    else
        print_failure "$method $endpoint - Expected: $expected_status, Got: $status"
        echo "$body" | jq -C '.' 2>/dev/null || echo "$body"
        return 1
    fi
}

# Start testing
echo -e "${YELLOW}Starting Algorithm Execution API Tests${NC}"
echo -e "${YELLOW}API Base URL: $API_BASE_URL${NC}"

# Test 1: Health Check
print_test_header "Health Check"
api_request "GET" "/health" "" "200"

# Test 2: Readiness Check
print_test_header "Readiness Check"
api_request "GET" "/ready" "" "200" || true

# Test 3: Metrics
print_test_header "Metrics"
api_request "GET" "/metrics" "" "200"

# Test 4: List Algorithm Executions (empty)
print_test_header "List Algorithm Executions"
api_request "GET" "/api/v1/algoexes" "" "200"

# Test 5: List with pagination
print_test_header "List Algorithm Executions with Pagination"
api_request "GET" "/api/v1/algoexes?page=1&per_page=10" "" "200"

# Test 6: Submit Algorithm Execution - Valid
print_test_header "Submit Algorithm Execution - Valid"
valid_submission=$(cat <<EOF
{
    "scientist_wallet": "0x742d35Cc6634C0532925a3b844Bc9e7595f8b23a",
    "dataset": "test-dataset-$(date +%s)",
    "github_repo": "https://github.com/ethereum/go-ethereum",
    "commit_hash": "v1.10.0"
}
EOF
)
api_request "POST" "/api/v1/algoexes" "$valid_submission" "200" || true

# Test 7: Submit Algorithm Execution - Invalid Wallet
print_test_header "Submit Algorithm Execution - Invalid Wallet"
invalid_wallet=$(cat <<EOF
{
    "scientist_wallet": "invalid-wallet-address",
    "dataset": "test-dataset",
    "github_repo": "https://github.com/test/repo",
    "commit_hash": "abc123"
}
EOF
)
api_request "POST" "/api/v1/algoexes" "$invalid_wallet" "400" || true

# Test 8: Submit Algorithm Execution - Missing Fields
print_test_header "Submit Algorithm Execution - Missing Fields"
missing_fields=$(cat <<EOF
{
    "scientist_wallet": "0x742d35Cc6634C0532925a3b844Bc9e7595f8b23a"
}
EOF
)
api_request "POST" "/api/v1/algoexes" "$missing_fields" "400" || true

# Test 9: Get Non-existent Algorithm Execution
print_test_header "Get Non-existent Algorithm Execution"
api_request "GET" "/api/v1/algoexes/999999" "" "404" || true

# Test 10: Invalid GitHub Repository
print_test_header "Submit Algorithm Execution - Invalid GitHub Repo"
invalid_repo=$(cat <<EOF
{
    "scientist_wallet": "0x742d35Cc6634C0532925a3b844Bc9e7595f8b23a",
    "dataset": "test-dataset",
    "github_repo": "not-a-github-url",
    "commit_hash": "abc123"
}
EOF
)
api_request "POST" "/api/v1/algoexes" "$invalid_repo" "400" || true

# Test 11: Test Algorithm List Endpoint
print_test_header "List Algorithms"
api_request "GET" "/api/v1/algorithms" "" "200" || true

# Test 12: Test Dataset List Endpoint
print_test_header "List Datasets"
api_request "GET" "/api/v1/datasets" "" "200" || true

# Test 13: Test Execution List Endpoint
print_test_header "List Executions"
api_request "GET" "/api/v1/executions" "" "200" || true

# Test 14: Test Committee Members Endpoint
print_test_header "List Committee Members"
api_request "GET" "/api/v1/committee/members" "" "200" || true

# Test 15: Test 404 for Unknown Endpoint
print_test_header "Test 404 Response"
api_request "GET" "/api/v1/unknown-endpoint" "" "404" || true

# Summary
echo -e "\n${YELLOW}=== Test Summary ===${NC}"
echo -e "${GREEN}Passed: $TESTS_PASSED${NC}"
echo -e "${RED}Failed: $TESTS_FAILED${NC}"

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "\n${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "\n${RED}Some tests failed!${NC}"
    exit 1
fi
