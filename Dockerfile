# Phase 3: IAM Service Dockerfile Alignment
FROM rust:1.89-bookworm AS builder

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

# Copy the whole project to maintain structure for sqlx migrations
COPY gridtokenx-iam-service/ gridtokenx-iam-service/
COPY gridtokenx-blockchain-core/ gridtokenx-blockchain-core/

WORKDIR /app/gridtokenx-iam-service

# Build in release mode
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
COPY --from=builder /app/gridtokenx-iam-service/target/release/gridtokenx-iam-service /app/iam-service
COPY --from=builder /app/gridtokenx-iam-service/migrations /app/migrations

# Ensure appuser owns the binary
RUN chown -R appuser:appgroup /app

# Use non-root user
USER appuser

# Expose ports (HTTP: 8080, gRPC: 8090)
EXPOSE 8080 8090

# Run the binary
ENTRYPOINT ["/app/iam-service"]
