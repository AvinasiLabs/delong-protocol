# Build stage
FROM rust:1.75-alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    pkgconfig \
    protobuf-dev \
    clang \
    llvm \
    git

# Set up working directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY secure/Cargo.toml ./secure/

# Copy source code
COPY secure ./secure

# Build the application
WORKDIR /app/secure
RUN cargo build --release

# Runtime stage
FROM alpine:3.19

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    openssl \
    libgcc \
    curl \
    docker-cli \
    postgresql-client

# Create non-root user
RUN addgroup -g 1000 secure && \
    adduser -D -u 1000 -G secure secure

# Copy the binary from builder
COPY --from=builder /app/secure/target/release/secure /usr/local/bin/secure

# Copy ABI files
COPY secure/abi /app/abi

# Create necessary directories
RUN mkdir -p /app/data /app/logs /app/keys && \
    chown -R secure:secure /app

# Switch to non-root user
USER secure

# Set working directory
WORKDIR /app

# Environment variables
ENV RUST_LOG=info \
    CONFIG_PATH=/app/config.toml \
    DATABASE_URL=postgresql://secure:secure@localhost/secure \
    IPFS_API_URL=http://localhost:5001 \
    CHAIN_RPC_URL=http://localhost:8545 \
    TEE_ENDPOINT=/var/run/dstack/dstack.sock

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Expose port
EXPOSE 8080

# Run the application
ENTRYPOINT ["/usr/local/bin/secure"]
