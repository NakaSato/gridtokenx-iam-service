# Phase 3: IAM Service Dockerfile Alignment
FROM rust:1.88-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    cmake \
    clang \
    git \
    curl \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the workspace manifest and lockfile
COPY gridtokenx-iam-service/Cargo.toml ./
COPY gridtokenx-iam-service/Cargo.lock ./

# Copy all workspace members
COPY gridtokenx-iam-service/crates crates/
COPY gridtokenx-iam-service/bin bin/
COPY gridtokenx-iam-service/proto proto/

# Build in release mode
# Use the workspace bin name
RUN cargo build --release --bin gridtokenx-iam-service

# -----------------------------------------------------------------------------
# Stage 2: Runtime (Minimal Debian)
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    tzdata \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -g 1000 appgroup && \
    useradd -u 1000 -g appgroup -s /bin/sh appuser

WORKDIR /app

# Copy binary from builder stage
# Binaries are in target/release/
COPY --from=builder /app/target/release/gridtokenx-iam-service /app/iam-service

# Ensure appuser owns the binary
RUN chown appuser:appgroup /app/iam-service

# Use non-root user
USER appuser

# Expose ports (HTTP: 8080, gRPC: 8090 - based on docker-compose mapping)
EXPOSE 8080 8090

# Run the binary
ENTRYPOINT ["/app/iam-service"]
