# Multi-stage build for DeLong Protocol Services
FROM rust:1.85-bullseye AS builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY common/ common/
COPY gateway/ gateway/
COPY core/ core/
COPY secure/ secure/

# Build argument to specify which service to build
ARG SERVICE=gateway
RUN echo "Building service: $SERVICE"

# Build the specified service
RUN cargo build --release --package $SERVICE

FROM debian:bullseye-slim

# Install base runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    wget \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Build argument for service (used in runtime stage)
ARG SERVICE=gateway

# Install service-specific dependencies
RUN if [ "$SERVICE" = "secure" ]; then \
    apt-get update && apt-get install -y \
        python3 \
        python3-pip \
        python3-pandas \
        && pip3 install --no-cache-dir pandas numpy scikit-learn matplotlib seaborn \
        && rm -rf /var/lib/apt/lists/*; \
    fi

# Copy the binary from builder
COPY --from=builder /app/target/release/$SERVICE .

# Create service-specific directories
RUN if [ "$SERVICE" = "core" ]; then \
        mkdir -p /data/datasets; \
    elif [ "$SERVICE" = "secure" ]; then \
        mkdir -p /secure/keys /secure/algorithms /secure/results /tmp/execution_env; \
    fi

# Create non-root user
RUN groupadd -r delong && useradd -r -g delong delong

# Set ownership based on service
RUN if [ "$SERVICE" = "core" ]; then \
        chown -R delong:delong /app /data; \
    elif [ "$SERVICE" = "secure" ]; then \
        chown -R delong:delong /app /secure /tmp/execution_env; \
    else \
        chown -R delong:delong /app; \
    fi

USER delong

# Expose service-specific ports
EXPOSE 8080 8081 8082

# Set service-specific environment variables
ENV SERVICE_NAME=$SERVICE
RUN if [ "$SERVICE" = "secure" ]; then \
        echo "TEE_CLIENT_TYPE=mock" >> /etc/environment && \
        echo "TEE_MASTER_KEY_PATH=/secure/keys/master.key" >> /etc/environment; \
    fi

# Add health check (port will be determined at runtime)
HEALTHCHECK --interval=30s --timeout=10s --start-period=15s --retries=3 \
    CMD if [ "$SERVICE" = "gateway" ]; then curl -f http://localhost:8080/health; \
        elif [ "$SERVICE" = "core" ]; then curl -f http://localhost:8081/health; \
        elif [ "$SERVICE" = "secure" ]; then curl -f http://localhost:8082/health; \
        else exit 1; fi

# Dynamic command based on service
CMD sh -c "./$SERVICE" 