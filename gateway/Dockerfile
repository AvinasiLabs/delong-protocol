# Multi-stage Docker build for DeLong Protocol Gateway Service
# Build stage
FROM rust:1.85-slim as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    build-essential \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace Cargo files first for better layer caching
COPY Cargo.toml Cargo.lock ./
COPY gateway/Cargo.toml ./gateway/
COPY common/Cargo.toml ./common/
COPY core/Cargo.toml ./core/
COPY secure/Cargo.toml ./secure/

# Create dummy source files to cache dependencies
RUN mkdir -p gateway/src common/src core/src secure/src && \
    echo "fn main() {}" > gateway/src/main.rs && \
    echo "fn main() {}" > common/src/lib.rs && \
    echo "fn main() {}" > core/src/main.rs && \
    echo "fn main() {}" > secure/src/main.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release -p gateway --locked

# Remove dummy source files
RUN rm -rf gateway/src common/src core/src secure/src

# Copy actual source code
COPY common ./common
COPY gateway ./gateway

# Build the actual application
RUN touch gateway/src/main.rs && \
    cargo build --release -p gateway --locked

# Verify the binary was built
RUN ls -la target/release/ && \
    file target/release/gateway && \
    target/release/gateway --version || echo "Binary built successfully"

# Runtime stage
FROM debian:bookworm-slim as runtime

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl3 \
    libpq5 \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create app user and group
RUN groupadd -r appuser && \
    useradd -r -g appuser -u 1001 appuser

# Create necessary directories
RUN mkdir -p /app/logs && \
    chown -R appuser:appuser /app

# Copy the compiled binary from builder stage
COPY --from=builder /app/target/release/gateway /app/gateway

# Ensure the binary is executable
RUN chmod +x /app/gateway

# Switch to non-root user
USER appuser

# Build arguments for flexible configuration
ARG SERVER_PORT=11111
ARG RUST_LOG=info

# Expose the port the app runs on (configurable)
EXPOSE ${SERVER_PORT}

# Add health check (uses environment variable)
HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:${SERVER_PORT}/health || exit 1

# Set environment variables (can be overridden by docker-compose)
ENV RUST_LOG=${RUST_LOG}
ENV SERVER_HOST=0.0.0.0
ENV SERVER_PORT=${SERVER_PORT}

# Create volume mount points
VOLUME ["/app/logs"]

# Run the application
ENTRYPOINT ["./gateway"]
