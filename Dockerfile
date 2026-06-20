# IAM Service — distroless image: binary + its shared libs only.
# No Rust toolchain, no target/ cache, no OS package manager in the final image.
# syntax=docker/dockerfile:1.7
# -----------------------------------------------------------------------------
# Stage 1: Builder (compiles, then collects binary + ldd deps into /out)
# -----------------------------------------------------------------------------
FROM rust:1.89-bookworm AS builder

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    cmake \
    clang \
    git \
    curl \
    protobuf-compiler \
    busybox-static \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the whole project to maintain structure for sqlx migrations
COPY gridtokenx-iam-service/ gridtokenx-iam-service/
COPY gridtokenx-blockchain-core/ gridtokenx-blockchain-core/
COPY gridtokenx-telemetry/ gridtokenx-telemetry/

WORKDIR /app/gridtokenx-iam-service

# Build release binary. target/ stays in this stage only — never copied forward.
RUN cargo build --release --bin gridtokenx-iam-service

# Collect the binary + the non-glibc shared libs it needs into a flat lib/ folder.
# glibc core + the dynamic loader are provided by the distroless/cc base, so skip
# them to avoid clashing with the base image's runtime.
RUN set -eux; \
    BIN=target/release/gridtokenx-iam-service; \
    mkdir -p /out/lib; \
    cp "$BIN" /out/iam-service; \
    cp /bin/busybox /out/busybox; \
    ldd "$BIN" | awk '/=>/{print $3} !/=>/{print $1}' | grep -E '^/' | sort -u | while read -r lib; do \
        case "$lib" in \
            */ld-linux*|*/libc.so*|*/libm.so*|*/libpthread*|*/libdl.so*|*/librt.so*) continue;; \
        esac; \
        cp -Lv "$lib" /out/lib/; \
    done

# -----------------------------------------------------------------------------
# Stage 2: Runtime (distroless — glibc + libgcc/libstdc++, ca-certs, tzdata only)
# -----------------------------------------------------------------------------
FROM gcr.io/distroless/cc-debian12 AS runtime

WORKDIR /app

# Binary, its lib folder, the static busybox (for the healthcheck), and migrations.
COPY --from=builder /out/iam-service /app/iam-service
COPY --from=builder /out/lib/ /app/lib/
COPY --from=builder /out/busybox /usr/bin/busybox
COPY --from=builder /app/gridtokenx-iam-service/migrations /app/migrations

# Expose ports (HTTP: 4010, gRPC: 4020)
EXPOSE 4010 4020

# Default Configuration
ENV LD_LIBRARY_PATH=/app/lib \
    ENVIRONMENT=production \
    IAM_PORT=4010 \
    IAM_GRPC_PORT=4020 \
    LOG_LEVEL=info \
    AUTH_CPU_SEMAPHORE_LIMIT=32 \
    TOKIO_WORKER_THREADS=4 \
    DATABASE_MAX_CONNECTIONS=50 \
    DATABASE_MIN_CONNECTIONS=5

# Run the binary
ENTRYPOINT ["/app/iam-service"]
