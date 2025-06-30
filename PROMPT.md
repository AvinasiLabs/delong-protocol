# DeLong Protocol - AI Development Context

## 📖 About This Document

**Purpose**: This document serves as the primary context file for AI assistants working on the DeLong Protocol codebase. It contains essential project information, architectural decisions, and implementation details that enable AI to understand the full context and write appropriate code.

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

## Project Overview

DeLong Protocol is a **privacy-preserving computation platform for biomedical research** that combines **blockchain governance** with **Trusted Execution Environment (TEE)** technology. Researchers can analyze sensitive biomedical data without compromising privacy.

### Core Innovation
- **Data Privacy**: Raw biomedical data never leaves TEE in unencrypted form
- **Transparent Governance**: Algorithm approval through blockchain-based committee voting
- **Verifiable Computing**: All operations logged on-chain with hardware attestation
- **Research Focus**: Specifically designed for longevity and biomedical research workflows

### Key Business Logic
1. **Data Contributors** upload sensitive datasets encrypted in TEE
2. **Researchers** submit algorithms for community review
3. **Committee Members** vote on algorithm safety via smart contracts
4. **Approved Algorithms** execute securely within TEE environments
5. **Results** are published with full attribution tracking on blockchain

## System Architecture

### Service Architecture
```
Client Applications
        ↓
    API Gateway (Rust + Axum) :8080
        ↓
    ┌─────────────────┬─────────────────────┐
    ↓                 ↓                     ↓
Core Service      Secure Service      Data Pipeline
(Non-TEE)         (TEE CVM)          (Python)
:8081             :8082              :8018
    ↓                 ↓                     ↓
PostgreSQL        IPFS + TEE         Analysis Engine
+ Redis           KeyVault           + Sample Gen
```

### Critical Service Boundaries

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
- **TEE Platform**: Phala Network's Confidential Virtual Machine (CVM)
- **Blockchain**: Ethereum-compatible smart contracts
- **Storage**: IPFS for decentralized content-addressed storage
- **Monitoring**: OpenTelemetry with structured logging
- **Caching**: Redis with connection pooling for JWT/API key validation

## Data Models & Types

### Shared Models Architecture
All data models used across services are defined in the `common` crate to prevent code duplication and ensure consistency:

- **Location**: `common/src/models/`
- **Organization**: Models are grouped by business domain (e.g., `algo_exe.rs`, `pagination.rs`)
- **Usage**: Gateway, Core, and Secure services all import models from `common`
- **Benefits**: Single source of truth, consistent serialization, shared validation logic

**Key Model Categories**:
- `AlgoExeData`, `AlgoExeSubmissionRequest`, `AlgoExeSubmissionResponse` - Algorithm execution
- `PaginatedResponse<T>`, `PaginationParams` - Consistent pagination across all endpoints
- `AlgoExeStatus`, `AlgoReviewStatus` - Type-safe status enumerations

### Database Models vs API Models
- **Database Models**: Defined in individual services using SQLx for type-safe queries
- **API Models**: Shared in `common` crate for consistent request/response formats
- **Conversion**: Each service handles conversion between database and API models

### Static Datasets (Immutable, TEE-encrypted)
- **Naming**: Prefixed with `__static__` for system identification
- **Storage**: IPFS with TEE encryption using derived keys
- **Features**: Automatic sample data generation, blockchain registration
- **Lifecycle**: Upload → Encrypt → Generate Samples → Store → Register on blockchain

### Dynamic Datasets (Mutable, local filesystem)
- **Storage**: Local filesystem with symlink-based versioning
- **Features**: Periodic updates, no blockchain registration required
- **Use Case**: Real-time data feeds, updated reference datasets

### Sample Data (Public access)
- **Purpose**: Enable algorithm testing without sensitive data access
- **Generation**: Statistical modeling preserves original data characteristics
- **Access**: Public HTTP endpoints at `/api/sample/{cid}`
- **Privacy**: Synthetic data generated by data pipeline service

## Security Architecture

### TEE Security Model
**Key Derivation Process**:
1. TEE maintains hardware-protected master key
2. Each dataset gets unique key via HKDF(master_key, dataset_hash, context)
3. Files encrypted with AES-256-GCM within TEE enclave
4. Decryption keys never leave TEE environment

**Data Processing Flow**:
1. File uploaded to Gateway → SHA-256 hash computed
2. File transferred to Secure Service in TEE
3. TEE encrypts file with derived key
4. Encrypted file uploaded to IPFS
5. Sample data generated by pipeline service
6. Metadata registered on blockchain

### Access Control Matrix
| Operation | Auth Required | Authorization | TEE Required | Cache Strategy |
|-----------|---------------|---------------|--------------|----------------|
| Upload Static Dataset | JWT | User Role | ✅ | JWT cached 5min |
| Download Sample Data | None | Public | ❌ | No cache |
| Execute Algorithm | JWT | Committee Approval | ✅ | JWT cached 5min |
| Committee Voting | JWT | Committee Member | ✅ | JWT cached 5min |
| API Key Management | JWT | Admin Role | ❌ | API key cached 10min |

### Authentication Caching Strategy

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

## Blockchain Integration

### Smart Contract Architecture

**AlgorithmReview Contract** - Decentralized algorithm governance:
- `submitAlgorithm()`: Researchers submit algorithms for review
- `vote()`: Committee members vote on algorithm safety  
- `resolve()`: Finalize voting and update approval status
- Events: `ExecutionSubmitted`, `VoteCasted`, `AlgorithmResolved`

**DataContribution Contract** - Usage tracking and attribution:
- `registerData()`: Record new dataset contributions
- `recordUsage()`: Log algorithm executions using datasets
- Events: `DataRegistered`, `DataUsed`

### Event-Driven Synchronization
**Why Events Instead of State**: Better scalability, complete audit trail, lower gas costs
**Sync Strategy**: Incremental event processing + replay mechanism for recovery

## API Interface Design

### Authentication
- **JWT Tokens**: User session authentication
- **API Keys**: Service-to-service authentication with scope-based permissions

### Response Format
```json
{
  "code": "SUCCESS|BAD_REQUEST|UNAUTHORIZED|FORBIDDEN|NOT_FOUND|INTERNAL_SERVER_ERROR|TIMEOUT|TOO_MANY_REQUESTS",
  "data": {...},
  "message": "Human readable message",
  "request_id": "req_123456",
  "timestamp": "2023-01-01T00:00:00Z"
}
```

**Implementation**: This format is now unified across all services via the `common::ApiResponse<T>` type. All services use consistent response codes and error handling.

### Key Endpoints
- `POST /api/static-datasets` - Upload encrypted dataset (multipart form)
- `GET /api/sample/{cid}` - Download public sample data (no auth)
- `POST /api/algo-exes` - Submit algorithm for execution
- `POST /api/committee` - Committee member management
- `GET /ws/tx-status/{hash}` - WebSocket transaction status

## Data Pipeline Service Integration

### Column Type Detection Algorithm
1. **Numeric**: Try parsing as f64, >80% success rate → numeric type
2. **Boolean**: Check for true/false/0/1 patterns, >80% → boolean type  
3. **String**: Default type for everything else

### Sample Data Generation Strategy
- **Numeric**: Generate normal distribution based on mean/std of original
- **Boolean**: Preserve true/false ratio from original dataset
- **String**: Random sampling or pattern-based generation from original values

## Deployment Characteristics

### TEE Deployment (Critical Difference)
**Secure Service** runs independently in Phala Network's TEE environment:
- NOT deployed via traditional Docker Compose
- Deployed as confidential virtual machine on Phala Cloud
- Hardware attestation verifies code integrity
- Isolated from other services by design

### Traditional Services
Gateway, Core, and DataPipe services use standard containerization:
- Docker Compose for development/staging
- Kubernetes for production scaling
- Standard service discovery and load balancing

## Development Guidelines

### Error Handling
- Use Rust's `Result<T, E>` pattern consistently
- **Unified Error Types**: All services use `common::ApiError` and `common::ApiResponse<T>` for consistent error handling
- **Response Codes**: Use `common::ResponseCode` enum for standardized status codes
- **Error Conversion**: `ApiError` automatically converts to appropriate HTTP status codes
- Convert errors to appropriate HTTP status codes at service boundaries
- Log errors with structured tracing for debugging
- **Common Crate**: Provides `ApiResponse::success()`, `ApiResponse::error()`, and convenience methods like `ApiResponse::bad_request()`

### Async Patterns
- All I/O operations must be async/await
- Use connection pooling for database operations
- Implement proper timeout and retry logic for external services

### Caching Strategy
- Redis integration with graceful fallback when unavailable
- Cache authentication data to reduce validation overhead
- Use SHA-256 hashed cache keys for security
- Implement configurable TTL values per data type
- Connection manager with automatic reconnection

### Testing Strategy
- Unit tests for business logic and data transformations
- Integration tests for service-to-service communication
- Mock implementations for TEE-dependent functionality during testing
- **Cache Testing**: Use `create_test_router()` for disabled cache, `create_auth_test_router()` for auth-enabled tests
- Redis cache tests with disabled connections for CI/CD environments

### Configuration Management
- Environment-based configuration with sensible defaults
- Separate configuration for each deployment environment
- Validation of critical configuration parameters at startup
- Redis configuration: URL, pool size, timeouts, TTL values

## Key Implementation Constraints

1. **TEE Limitations**: Code running in TEE must be deterministic and minimal
2. **IPFS Integration**: All content-addressed storage must preserve immutability
3. **Blockchain Events**: State synchronization relies on event ordering and replay capability
4. **Privacy Requirements**: Raw data must never be logged or transmitted unencrypted
5. **Performance**: Gateway must handle 10k+ requests/second with <200ms latency
6. **Compatibility**: Maintain API compatibility with existing Go-based clients
7. **Cache Resilience**: Authentication must work even when Redis is unavailable
8. **Security**: Cache keys must be hashed, no sensitive data stored in plain text
9. **Error Handling Consistency**: All services must use unified error types from common crate
10. **Response Format**: All API responses must follow the standardized `ApiResponse<T>` structure

## Common Crate Architecture

### Shared Data Models
The `common` crate centralizes all data structures used across services to eliminate code duplication:

**Structure**:
```
common/src/models/
├── mod.rs           # Re-exports and module documentation
├── algo_exe.rs      # Algorithm execution models
└── pagination.rs    # Pagination utilities
```

**Key Benefits**:
- **Consistency**: Same data structures across Gateway, Core, and Secure services
- **Maintainability**: Single location for model updates
- **Type Safety**: Shared enums prevent string-based status mismatches
- **Testing**: Comprehensive test coverage for all models

### Unified Error Handling
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
let response = ApiResponse::bad_request("Invalid input");
let response = ApiError::NotFound("Resource missing".to_string()).to_response();
```

**Performance**: Benchmarked at <100ns for response creation and <150ns for serialization.

## Database Architecture

### SQLx Integration
- **Query Builder**: Type-safe SQL queries with compile-time verification
- **Migrations**: Version-controlled database schema changes
- **Connection Pooling**: Efficient database connection management
- **Type Mapping**: Automatic conversion between Rust types and PostgreSQL types

### Data Access Patterns
- **Repository Pattern**: Database access abstracted behind service-specific repositories
- **Transaction Management**: ACID compliance for multi-step operations
- **Query Optimization**: Prepared statements and connection reuse
- **Error Handling**: SQLx errors converted to application-specific error types

---

This document should be updated whenever architectural decisions change or new components are added to the system.