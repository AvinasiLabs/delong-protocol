# DeLong Protocol - AI Development Context

## 📖 Document Overview

### About This Document

**Purpose**: This document serves as the primary context file for AI assistants working on the DeLong Protocol codebase. It is a **living document** that maintains the most current project architecture information, business logic, and implementation details.

**Important Update Guidelines**:
- ✅ **Replace outdated content** with current information when architecture changes
- ✅ **Keep only the latest design decisions** and implementation approaches
- ❌ **Do NOT accumulate historical changes** or modification logs
- ❌ **Do NOT append update experiences** to existing content

This document should always reflect the **current state** of the project, not its evolution history. When updating, remove obsolete information and write fresh, accurate content that represents the latest architecture.

### Target Audience & Usage

**Target Audience**:
- Primary: AI coding assistants (Claude, GPT, etc.)
- Secondary: New developers onboarding to the project

**Usage**: When working on DeLong Protocol, AI assistants should reference this document to understand:
- Project background and core concepts
- System architecture and component relationships
- Business logic and data flows
- Technical constraints and design decisions
- Integration points between services

---

## 🎯 Project Overview

### Core Mission
DeLong Protocol is a **privacy-preserving computation platform for biomedical research** that combines **blockchain governance** with **Trusted Execution Environment (TEE)** technology. Researchers can analyze sensitive biomedical data without compromising privacy.

### Key Innovation Points
- **Data Privacy**: Raw biomedical data never leaves TEE in unencrypted form
- **Transparent Governance**: Algorithm approval through blockchain-based committee voting
- **Verifiable Computing**: All operations logged on-chain with hardware attestation
- **Research Focus**: Specifically designed for longevity and biomedical research workflows

### Business Logic Flow
1. **Data Contributors** upload sensitive datasets encrypted in TEE
2. **Researchers** submit algorithms for community review
3. **Committee Members** vote on algorithm safety via smart contracts
4. **Approved Algorithms** execute securely within TEE environments
5. **Results** are published with full attribution tracking on blockchain

---

## 🏗️ System Architecture

### High-Level Architecture

```
Client Applications
        ↓
    API Gateway (Rust + Axum) :11111
        ↓
    ┌─────────────────┬─────────────────────┐
    ↓                 ↓                     ↓
Core Service      Secure Service      Data Pipeline
(Non-TEE)         (TEE CVM)          (Python)
:11112            :11113             :8018
    ↓                 ↓                     ↓
PostgreSQL        IPFS + TEE         Analysis Engine
+ Redis           KeyVault           + Sample Gen
```

### Service Architecture Pattern

All web services (Gateway, Core, Secure) follow a consistent **lib.rs pattern** for better testability and code organization:

**Structure**:
```
src/
├── lib.rs          # Public library interface, re-exports modules
├── main.rs         # Binary entry point only, minimal logic
├── handlers/       # HTTP request handlers
├── middleware/     # HTTP middleware components
├── routes/         # Route definitions
├── models/         # Data models with DAO functions (database instance as parameter)
├── services/       # Business logic services (email, proxy, etc.)
├── config/         # Configuration management
└── utils/          # Utility functions
```

**Benefits**:
- **Testability**: Business logic in `lib.rs` can be unit tested without running the binary
- **Reusability**: Other crates can import functionality via `lib.rs`
- **Separation**: `main.rs` handles only application setup and server lifecycle
- **Consistency**: All services follow the same pattern for easier maintenance

### Service Boundaries & Responsibilities

**API Gateway** (`gateway/`) - Request routing and authentication:
- Routes `auth.*` → Core Service (non-sensitive)
- Routes `dataset.*` → Dual routing (static→Secure, dynamic→Core)
- Routes `algo-exe.*`, `committee.*`, `vote.*`, `report.*` → Secure Service (TEE)
- Local processing: `health.*`, `websocket.*`

**Core Service** (`core/`) - Public operations (PostgreSQL + Redis):
- User authentication and API keys
- Dynamic dataset management (local filesystem)
- Public metadata and configuration

**Secure Service** (`secure/`) - Sensitive operations (TEE environment):
- Static dataset encryption/decryption using TEE keys
- Algorithm execution in isolated containers
- Committee voting and blockchain interactions
- IPFS integration for encrypted storage

**Data Pipeline** (`datapipe/`) - Data analysis (Python + FastAPI):
- CSV structure analysis and type detection
- Privacy-preserving sample data generation
- Statistical modeling for synthetic data

### Technology Stack

- **Backend**: Rust (Axum framework, Tokio async runtime)
- **Database**: PostgreSQL + Redis cache (authentication & session caching)
- **Database Access**: SQLx for type-safe SQL queries and migrations
- **API Documentation**: OpenAPI/Swagger UI integration using utopia-util
- **TEE Platform**: Phala Network's Confidential Virtual Machine (CVM)
- **Blockchain**: Ethereum-compatible smart contracts
- **Storage**: IPFS for decentralized content-addressed storage
- **Monitoring**: OpenTelemetry with structured logging
- **Caching**: Redis with connection pooling for JWT/API key validation

---

## 📊 Data Management

### Data Model Architecture

#### Shared Models System
**Business Models**: Business logic models and DAOs remain in their respective service crates (core, secure) following the lib+bin pattern:

- **Location**: Each service defines its models in `{service}/src/models/`
- **Inter-service Access**: Gateway imports models from core and secure services via their lib interfaces
- **Benefits**: Proper encapsulation, business logic stays with data, lib+bin testability

**API-Level Shared Types**: Only API interface types are defined in the `common` crate:

- **Location**: `common/src/models/`
- **Usage**: Request/response formats, pagination utilities, error types
- **Benefits**: Consistent API contracts, shared serialization logic

**Common Model Categories** (API-level only):
- `PaginatedResponse<T>`, `PaginationParams` - Consistent pagination across all endpoints
- `ApiResponse<T>`, `ApiError`, `ResponseCode` - Standardized response formats
- `ValidateApiKeyRequest`, `ValidateApiKeyResponse` - Authentication interface types

**Business Model Examples** (in respective services):
- `core::models::User`, `core::models::ApiKey` - User management and authentication
- `secure::models::AlgoExecution`, `secure::models::Committee` - TEE operations

#### Database vs API Models
- **Database Models**: Defined in individual services using SQLx for type-safe queries
- **API Models**: Shared in `common` crate for consistent request/response formats
- **Conversion**: Each service handles conversion between database and API models

### Data Storage Types

#### Static Datasets (Immutable, TEE-encrypted)
- **Naming**: Prefixed with `__static__` for system identification
- **Storage**: IPFS with TEE encryption using derived keys
- **Features**: Automatic sample data generation, blockchain registration
- **Lifecycle**: Upload → Encrypt → Generate Samples → Store → Register on blockchain

#### Dynamic Datasets (Mutable, local filesystem)
- **Storage**: Local filesystem with symlink-based versioning
- **Features**: Periodic updates, no blockchain registration required
- **Use Case**: Real-time data feeds, updated reference datasets

#### Sample Data (Public access)
- **Purpose**: Enable algorithm testing without sensitive data access
- **Generation**: Statistical modeling preserves original data characteristics
- **Access**: Public HTTP endpoints at `/api/sample/{cid}`
- **Privacy**: Synthetic data generated by data pipeline service

### Database Architecture

#### SQLx Integration
- **Query Builder**: Type-safe SQL queries with compile-time verification
- **Migrations**: Version-controlled database schema changes
- **Connection Pooling**: Efficient database connection management
- **Type Mapping**: Automatic conversion between Rust types and PostgreSQL types

#### Data Access Patterns
- **Repository Pattern**: Database access abstracted behind service-specific repositories
- **Transaction Management**: ACID compliance for multi-step operations
- **Query Optimization**: Prepared statements and connection reuse
- **Error Handling**: SQLx errors converted to application-specific error types

### Data Pipeline Integration

#### Column Type Detection Algorithm
1. **Numeric**: Try parsing as f64, >80% success rate → numeric type
2. **Boolean**: Check for true/false/0/1 patterns, >80% → boolean type
3. **String**: Default type for everything else

#### Sample Data Generation Strategy
- **Numeric**: Generate normal distribution based on mean/std of original
- **Boolean**: Preserve true/false ratio from original dataset
- **String**: Random sampling or pattern-based generation from original values

---

## 🔒 Security & Privacy

### TEE Security Model

#### Key Derivation Process
1. TEE maintains hardware-protected master key
2. Each dataset gets unique key via HKDF(master_key, dataset_hash, context)
3. Files encrypted with AES-256-GCM within TEE enclave
4. Decryption keys never leave TEE environment

#### Data Processing Flow
1. File uploaded to Gateway → SHA-256 hash computed
2. File transferred to Secure Service in TEE
3. TEE encrypts file with derived key
4. Encrypted file uploaded to IPFS
5. Sample data generated by pipeline service
6. Metadata registered on blockchain

### Authentication & Authorization

#### Access Control Matrix
| Operation | Auth Required | Authorization | TEE Required | Cache Strategy |
|-----------|---------------|---------------|--------------|----------------|
| Upload Static Dataset | JWT | User Role | ✅ | JWT cached 5min |
| Download Sample Data | None | Public | ❌ | No cache |
| Execute Algorithm | JWT | Committee Approval | ✅ | JWT cached 5min |
| Committee Voting | JWT | Committee Member | ✅ | JWT cached 5min |
| API Key Management | JWT | Admin Role | ❌ | API key cached 10min |

#### Authentication Methods
- **JWT Tokens**: User session authentication
- **API Keys**: Service-to-service authentication with scope-based permissions

#### Caching Strategy

**Redis Cache Integration**:
- **JWT Tokens**: Cached for 5 minutes after successful validation
- **API Keys**: Cached for 10 minutes after successful lookup
- **Cache Keys**: SHA-256 hash of token/key for security
- **Fallback**: Graceful degradation when Redis unavailable
- **Invalidation**: Automatic expiration + manual invalidation on revocation

**Performance Benefits**:
- Reduces database queries for repeated authentication requests
- Sub-millisecond cache lookup vs. cryptographic JWT validation
- Configurable TTL per authentication method
- Connection pooling prevents Redis connection overhead

---

## 🔌 API Design

### API Interface Standards

#### Authentication
- **JWT Tokens**: User session authentication
- **API Keys**: Service-to-service authentication with scope-based permissions

#### Unified Response Format

**🚨 CRITICAL DESIGN PRINCIPLE**: All API responses MUST use HTTP 200 OK status code. The actual business status is indicated in the response body's `code` field.

```json
{
  "code": "SUCCESS|BAD_REQUEST|UNAUTHORIZED|FORBIDDEN|NOT_FOUND|INTERNAL_SERVER_ERROR|TIMEOUT|TOO_MANY_REQUESTS",
  "data": {...},
  "request_id": "req_123456"
}
```

**HTTP Status Code Standard**:
- ✅ **Always use `200 OK`** for all business responses (success or error)
- ✅ **Use `code` field** to indicate actual business status
- ❌ **Never use `400`, `401`, `403`, `404`, `500`, etc.** for business logic errors
- ❌ **Only use non-200 codes** for infrastructure/protocol errors (network issues, malformed requests, etc.)

**Implementation**: This format is now unified across all services via the `common::ApiResponse<T>` type. All services use consistent response codes and error handling.

**⚠️ Current Implementation Status**:
- **Gateway Service**: ✅ Follows this standard
- **Core Service**: ❌ Currently using various HTTP status codes (needs refactoring)
- **Secure Service**: ❌ Currently using various HTTP status codes (needs refactoring)

**Design Philosophy**:
- Frontend handles all user-facing error messages and internationalization
- Backend provides only business logic status codes
- Detailed error information is recorded in backend logs using structured logging
- Response format is minimal and focused on essential information
- No `message` field: Frontend maps status codes to user-friendly messages
- No `timestamp` field: Client can record timestamps locally if needed

#### Implementation Guidance

**Correct Implementation Pattern**:
```rust
// ✅ CORRECT: Always return HTTP 200 OK
pub async fn login_user(
    // ... parameters
) -> Result<Json<ApiResponse<AuthResponse>>, Json<ApiResponse<()>>> {
    // Business logic here

    match validation_result {
        Ok(user) => Ok(Json(ApiResponse::success(user))),
        Err(_) => Err(Json(ApiResponse::error(ResponseCode::BadRequest))),
    }
}
```

**❌ INCORRECT Current Pattern (needs refactoring)**:
```rust
// ❌ WRONG: Using HTTP status codes for business logic
pub async fn login_user(
    // ... parameters
) -> Result<Json<ApiResponse<AuthResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    match validation_result {
        Ok(user) => Ok(Json(ApiResponse::success(user))),
        Err(_) => Err((StatusCode::BAD_REQUEST, Json(ApiResponse::error(ResponseCode::BadRequest)))),
    }
}
```

**Migration Guidelines**:
1. Remove all `StatusCode::*` from return types except infrastructure errors
2. All business logic responses should return `Json<ApiResponse<T>>`
3. Use `ResponseCode` enum in the response body instead of HTTP status codes
4. Infrastructure errors (network, parsing, etc.) can still use appropriate HTTP codes

### Key API Endpoints

#### Core Endpoints
- `POST /api/static-datasets` - Upload encrypted dataset (multipart form)
- `GET /api/sample/{cid}` - Download public sample data (no auth)
- `POST /api/algo-exes` - Submit algorithm for execution
- `POST /api/committee` - Committee member management
- `GET /ws/tx-status/{hash}` - WebSocket transaction status

#### AI Audit Endpoints
- `POST /api/ai-audit` - Submit AI audit request for GitHub repositories
- `GET /api/ai-audit` - Get AI audit reports with pagination
- `GET /api/ai-audit/report` - Get specific AI audit report by ID/execution_id/algorithm_id
- `GET /api/ai-audit/list` - List AI audit reports (alias for pagination)

### API Documentation

The system provides comprehensive API documentation through OpenAPI/Swagger integration:

#### Documentation Access
- **Swagger UI**: `http://localhost:11111/docs` - Interactive API documentation interface
- **Scalar UI**: `http://localhost:11111/scalar` - Alternative documentation interface
- **OpenAPI JSON**: `http://localhost:11111/docs/openapi.json` - Machine-readable API specification
- **Health Check**: `http://localhost:11111/health` - Service health status

#### Implementation Benefits
- **API Consistency**: Documentation is automatically synchronized with code changes
- **Client Generation**: OpenAPI specification can be used to generate client libraries
- **Testing**: Interactive documentation facilitates API testing and development
- **Integration**: External services can easily integrate using the OpenAPI specification

---

## ⛓️ Blockchain Integration

### Smart Contract Architecture

#### AlgorithmReview Contract
Decentralized algorithm governance:
- `submitAlgorithm()`: Researchers submit algorithms for review
- `vote()`: Committee members vote on algorithm safety
- `resolve()`: Finalize voting and update approval status
- Events: `ExecutionSubmitted`, `VoteCasted`, `AlgorithmResolved`

#### DataContribution Contract
Usage tracking and attribution:
- `registerData()`: Record new dataset contributions
- `recordUsage()`: Log algorithm executions using datasets
- Events: `DataRegistered`, `DataUsed`

### Event-Driven Synchronization

**Why Events Instead of State**: Better scalability, complete audit trail, lower gas costs
**Sync Strategy**: Incremental event processing + replay mechanism for recovery

---

## 👨‍💻 Development Guidelines

### Code Quality Standards

#### Error Handling
- Use Rust's `Result<T, E>` pattern consistently
- **Unified Error Types**: All services use `common::ApiError` and `common::ApiResponse<T>` for consistent error handling
- **Response Codes**: Use `common::ResponseCode` enum for standardized status codes
- **Error Conversion**: `ApiError` automatically converts to appropriate HTTP status codes
- Convert errors to appropriate HTTP status codes at service boundaries
- Log errors with structured tracing for debugging
- **Common Crate**: Provides `ApiResponse::success()`, `ApiResponse::error()`, and convenience methods like `ApiResponse::bad_request()`

#### Async Patterns
- All I/O operations must be async/await
- Use connection pooling for database operations
- Implement proper timeout and retry logic for external services

### Common Crate Architecture

#### Shared Data Models
The `common` crate centralizes all data structures used across services to eliminate code duplication:

**Structure**:
```
common/src/
├── lib.rs           # Main library entry point
├── utils.rs         # Shared utility functions
├── middleware/      # Common middleware components
├── server/          # Server utilities and helpers
└── models/          # Shared data models
    ├── mod.rs           # Re-exports and module documentation
    ├── algo_exe.rs      # Algorithm execution models
    ├── auth.rs          # Authentication models
    ├── committee.rs     # Committee management models
    ├── contract.rs      # Smart contract models
    ├── dataset.rs       # Dataset management models
    ├── pagination.rs    # Pagination utilities
    ├── report.rs        # Report generation models
    ├── vote.rs          # Voting system models
    └── websocket.rs     # WebSocket communication models
```

#### Unified Error Handling
The `common` crate provides standardized error handling and API response types:

**Core Types**:
- `ApiResponse<T>`: Standardized response wrapper with code, data, message, request_id, and timestamp
- `ResponseCode`: Enum for all standard HTTP response codes (SUCCESS, BAD_REQUEST, etc.)
- `ApiError`: Enum for different error types that automatically convert to appropriate response codes
- `ApiResult<T>`: Type alias for `Result<T, ApiError>`

**Usage Examples**:
```rust
// Success responses
let response = ApiResponse::success(data);
let response = ApiResponse::success_with_id(data, request_id);

// Error responses
let response = ApiResponse::bad_request();
let response = ApiResponse::bad_request_with_id(request_id);
let response = ApiError::NotFound.to_response();
let response = ApiError::NotFound.to_response_with_id(request_id);
```

### Testing Strategy

#### Testing Approaches
- Unit tests for business logic and data transformations
- Integration tests for service-to-service communication
- Mock implementations for TEE-dependent functionality during testing
- **Cache Testing**: Use `create_test_router()` for disabled cache, `create_auth_test_router()` for auth-enabled tests
- Redis cache tests with disabled connections for CI/CD environments

#### Caching Strategy
- Redis integration with graceful fallback when unavailable
- Cache authentication data to reduce validation overhead
- Use SHA-256 hashed cache keys for security
- Implement configurable TTL values per data type
- Connection manager with automatic reconnection

### Dependency Management

#### Workspace Pattern
- **Workspace Inheritance**: All crates must inherit dependencies from workspace `Cargo.toml` whenever possible
- **Centralized Dependencies**: Common dependencies (tokio, serde, axum, etc.) are defined in workspace and inherited by all crates
- **Version Consistency**: Use `{ workspace = true }` for shared dependencies to ensure version consistency across all crates
- **Latest Versions**: When adding new dependencies, always check online for the latest version and use it in workspace
- **Feature Flags**: Specify required features for each crate while inheriting the base dependency from workspace
- **Local Dependencies**: Internal crates (common, secure, etc.) use path-based dependencies
- **Workspace Pattern**: Only declare version-specific dependencies in individual crates when absolutely necessary for different feature requirements

**Example**:
```toml
# In workspace Cargo.toml
[workspace.dependencies]
tokio = { version = "1.45.1", features = ["full"] }
serde = { version = "1.0" }

# In individual crate Cargo.toml
[dependencies]
tokio = { workspace = true }  # ✓ Correct
serde = { workspace = true, features = ["derive"] }  # ✓ Correct - adds features
tokio = "1.45.1"  # ✗ Incorrect - should inherit from workspace
```

---

## 🚀 Deployment & Operations

### Deployment Architecture

#### Split Deployment Model
DeLong Protocol uses a **separated deployment model** to isolate TEE operations from traditional services:

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

#### Port Configuration
- **Gateway Service**: Port 11111 (main API entry point)
- **Core Service**: Port 11112 (non-TEE operations)
- **Secure Service**: Port 11113 (TEE operations)
- **Data Pipeline**: Port 8018 (Python service, unchanged)

### Configuration Management

#### Environment Configuration System
All services use unified `.env` configuration with hierarchical loading:

**Environment File Loading Priority**:
1. **`.env.local`** - Local overrides (git-ignored)
2. **`.env.{environment}`** - Environment-specific (e.g., `.env.development`, `.env.production`)
3. **`.env`** - Base configuration file

**Environment Variable Priority**:
1. **System Environment Variables** (highest priority)
2. **Service Prefix** - `GATEWAY_*`, `CORE_*`, `SECURE_*`
3. **Shared Prefix** - `SHARED_*` for cross-service config (database, Redis, JWT)
4. **Plain Variables** - No prefix
5. **Default Values** (lowest priority)

#### Configuration Categories
- **Database**: `DATABASE_URL`, `DATABASE_MAX_CONNECTIONS`, etc.
- **Redis**: `REDIS_URL`, `REDIS_JWT_CACHE_TTL`, `REDIS_API_KEY_CACHE_TTL`
- **JWT**: `JWT_SECRET`, `JWT_ACCESS_TOKEN_EXPIRATION`, `JWT_ISSUER`
- **Logging**: `LOG_LEVEL`, `LOG_JSON_FORMAT`, `RUST_LOG`
- **Services**: Service-specific configs with appropriate prefixes

### Deployment Methods

#### Traditional Services (docker-compose.yml)
```bash
# Deploy Gateway, Core, PostgreSQL, Redis
docker-compose up -d
```
- Standard Docker Compose deployment
- Shared database and caching layer
- Load balancing and service discovery
- Development/staging/production environments

#### TEE Services (docker-compose.secure.yml)
```bash
# Configure TEE environment variables
export CORE_SERVICE_URL="https://core.yourdomain.com"
export GATEWAY_SERVICE_URL="https://api.yourdomain.com"

# Deploy Secure service to Phala Cloud
docker-compose -f docker-compose.secure.yml up -d
```
- **Phala Cloud TEE**: Hardware attestation and isolated execution
- **IPFS Integration**: Encrypted storage for sensitive datasets
- **External Communication**: HTTPS to main services with JWT auth
- **Independent Scaling**: TEE resources scale separately from main services

#### Docker Compose Integration
```bash
# Main services (Gateway, Core, PostgreSQL, Redis)
docker-compose up -d

# TEE services (Secure, IPFS) - Phala Cloud deployment
# Provide .env file and docker-compose.secure.yml to Phala Cloud
# See: https://docs.phala.network/phala-cloud/phala-cloud-user-guides
```

---

## 🔑 Technical Constraints & Limitations

### Implementation Constraints

1. **TEE Limitations**: Code running in TEE must be deterministic and minimal
2. **IPFS Integration**: All content-addressed storage must preserve immutability
3. **Blockchain Events**: State synchronization relies on event ordering and replay capability
4. **Privacy Requirements**: Raw data must never be logged or transmitted unencrypted
5. **Performance**: Gateway must handle 10k+ requests/second with <200ms latency
6. **Compatibility**: Maintain API compatibility with existing Go-based clients
7. **Cache Resilience**: Authentication must work even when Redis is unavailable
8. **Security**: Cache keys must be hashed, no sensitive data stored in plain text

### Code Architecture Constraints

9. **Error Handling Consistency**: All services must use unified error types from common crate
10. **Response Format**: All API responses must follow the standardized `ApiResponse<T>` structure
11. **Lib.rs Pattern**: All web services must use lib.rs for business logic, main.rs only for server setup
12. **Frontend Responsibility**: Backend never returns user-facing error messages, only status codes

---

*This document should be updated whenever architectural decisions change or new components are added to the system.*
