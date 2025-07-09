# Secure Service Tests

This directory contains comprehensive tests for the DeLong Protocol Secure Service, including JWT authentication, TEE functionality, dataset operations, algorithm execution, blockchain synchronization, and end-to-end workflows.

## Test Structure

### Core Integration Tests
- `integration_test.rs` - HTTP endpoint tests (without JWT)
- `jwt_integration_test.rs` - JWT authentication and authorization tests
- `tee_test.rs` - Trusted Execution Environment tests
- `runtime_test.rs` - Runtime and async operation tests
- `blockchain_sync_test.rs` - Blockchain synchronization tests

### Test Utilities
- `lib.rs` - Common test utilities and helper functions
- `jwt_generator.rs` - JWT token generation utilities for testing

## JWT Testing and Token Generation

### Quick Start - Generate JWT Tokens
```bash
# From project root directory
./secure/generate_jwt_tokens.sh
```

This script will generate JWT tokens for manual API testing and provide ready-to-use curl commands.

### Manual Token Generation
You can also generate tokens using the test suite:
```bash
# Generate tokens for manual testing
DATABASE_URL="postgresql://delong:delong_test_2025@localhost:5433/delong" \
cargo test -p secure --test jwt_integration_test test_print_jwt_tokens_for_manual_testing -- --nocapture
```

### JWT Test Categories

#### Authentication Tests
- **Missing Token**: Verifies 401 response when no Authorization header is provided
- **Invalid Token**: Tests response to malformed JWT tokens
- **Expired Token**: Validates handling of expired tokens
- **Valid Tokens**: Confirms proper authentication with valid admin/user tokens

#### Authorization Tests
- **Admin-Only Endpoints**: Tests that regular users cannot access admin endpoints
- **Role-Based Access**: Verifies different access levels for admin vs user roles

### Running JWT Tests
```bash
# Run all JWT tests
DATABASE_URL="postgresql://delong:delong_test_2025@localhost:5433/delong" \
cargo test -p secure --test jwt_integration_test

# Run specific JWT test
DATABASE_URL="postgresql://delong:delong_test_2025@localhost:5433/delong" \
cargo test -p secure --test jwt_integration_test test_jwt_valid_admin_token -- --nocapture
```

## Testing with Live Service

### 1. Start the Secure Service with JWT
```bash
DATABASE_URL="postgresql://delong:delong_test_2025@localhost:5433/delong" \
USE_JWT=true \
JWT_SECRET="default_secret" \
cargo run -p secure
```

### 2. Generate JWT Tokens
```bash
./secure/generate_jwt_tokens.sh
```

### 3. Test API Endpoints

#### Health Check (No Auth Required)
```bash
curl http://localhost:8082/health
```

#### Authenticated Endpoint
```bash
curl -H "Authorization: Bearer YOUR_ADMIN_TOKEN" \
     http://localhost:8082/api/static-datasets
```

#### Admin-Only Endpoint
```bash
curl -X POST \
     -H "Authorization: Bearer YOUR_ADMIN_TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"wallet_address":"0x1234567890abcdef1234567890abcdef12345678","is_active":true}' \
     http://localhost:8082/api/committee
```

## Test Configuration

### Environment Variables
- `DATABASE_URL` - PostgreSQL connection string
- `TEST_DATABASE_URL` - Alternative database URL for tests
- `USE_JWT` - Enable JWT authentication (true/false)
- `JWT_SECRET` - Secret key for JWT signing/validation

### Test Database Setup
```bash
# Start PostgreSQL container
docker-compose up -d postgres

# Run database migrations
DATABASE_URL="postgresql://delong:delong_test_2025@localhost:5433/delong" \
sqlx migrate run --source secure/migrations
```

## JWT Configuration Details

### Test JWT Settings
- **Secret**: `default_secret` (for testing only)
- **Algorithm**: HS256
- **Expiry**: 1 hour (3600 seconds) for regular tokens, 2 hours for moderator tokens
- **Issuer**: `delong-protocol`

### Token Roles
- **admin**: Full access to all endpoints including admin-only operations
- **user**: Access to regular endpoints, forbidden from admin operations
- **moderator**: Custom role for extended testing (2-hour expiry)

### Authentication Flow
1. All `/api/*` endpoints require JWT authentication
2. `/health` endpoint is public (no authentication required)
3. Admin-only endpoints (like `POST /api/committee`) require admin role
4. Invalid/expired tokens return 401 Unauthorized with detailed error messages
5. Insufficient privileges return 403 Forbidden

## Error Response Format
All authentication errors follow the standard API response format:
```json
{
  "code": "UNAUTHORIZED" | "FORBIDDEN",
  "message": "Detailed error description",
  "data": null,
  "timestamp": "2025-01-09T04:00:00Z",
  "request_id": null
}
```

## Troubleshooting

### Common Issues
1. **Database Connection**: Ensure PostgreSQL is running on port 5433
2. **JWT Secret Mismatch**: Verify the service uses the same secret as tests (`default_secret`)
3. **Token Expiry**: Tokens expire in 1 hour, regenerate if needed
4. **Port Conflicts**: Secure service runs on port 8082 by default

### Debug Commands
```bash
# Check if database is accessible
nc -z localhost 5433

# Verify secure service compilation
cargo build -p secure

# Run health check
curl -v http://localhost:8082/health

# Test with invalid token to see error format
curl -v -H "Authorization: Bearer invalid_token" http://localhost:8082/api/static-datasets
```

## Development Workflow

### Adding New JWT Tests
1. Add test functions to `jwt_integration_test.rs`
2. Use helper functions: `generate_admin_token()`, `generate_user_token()`, `make_auth_request()`
3. Follow the existing pattern for authentication and authorization tests
4. Run tests to verify functionality

### Updating JWT Configuration
1. Modify constants in test files if needed
2. Update the generation script `generate_jwt_tokens.sh`
3. Ensure consistency between test and service configurations
4. Update documentation

Remember: Test tokens are for development only. Production should use secure, randomly generated secrets and proper key management. 