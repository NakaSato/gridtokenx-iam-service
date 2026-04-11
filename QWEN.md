# GridTokenX IAM Service - Context File

## Project Overview

**GridTokenX IAM Service** is a dual-protocol Identity and Access Management (IAM) microservice built in Rust, part of the GridTokenX P2P energy trading platform on Solana. It provides authentication, authorization, and user management via both REST and gRPC interfaces.

### Key Features
- **Dual Protocol**: REST API (Axum) on port 8081 + gRPC (ConnectRPC/Buffa) on port 8091
- **Authentication**: JWT (HS256) for users, API Keys (SHA256) for AMI/machine-to-machine
- **Password Security**: Argon2 (primary) with Bcrypt legacy support
- **Authorization**: Role-Based Access Control (RBAC) with 6 roles (User, Admin, AMI, Producer, Consumer, Operator)
- **Database**: PostgreSQL with SQLx (async, compile-time verified queries)
- **Observability**: OpenTelemetry tracing (SigNoz) + Prometheus metrics
- **Auto-Migrations**: Runs SQL migrations on startup

### Platform Topology

```
┌─────────────────────────────────────────────────────────────┐
│                      FRONTEND LAYER                          │
│  Trading UI      Explorer UI      Portal UI                 │
│  (:3000)         (:3001)         (:3002)                    │
└────────┬──────────────┬──────────────┬──────────────────────┘
         │              │              │
         └──────────────┼──────────────┘
                        ▼
              ┌─────────────────────┐
              │  Kong (:4000)       │
              │  route / rate-limit │
              └─────────┬───────────┘
                        ▼
              ┌─────────────────────┐
              │  API Gateway        │
              │  :4000 / :4001      │
              │  orchestrator       │
              │                     │
              │  ── PG :5434       │
              │  ── Redis :6379    │
              │  ── Solana :8899   │
              └──┬───────────┬─────┘
           gRPC│           │gRPC
         :8091 │           │:8093
               ▼           ▼
        ┌──────────┐ ┌──────────┐
        │ IAM      │ │ Trading  │
        │ Service  │ │ Service  │
        │ :8081/91 │ │ :8092/93 │
        │          │ │          │
        │ ── PG ──│ │ ── PG ──│
        │ ── Redis│ │ ── Redis│
        └──────────┘ │ ──Solana│
                     └──────────┘


  ── INDEPENDENT (not behind API Gateway) ──

  ┌──────────────┐          ┌──────────────┐
  │ Smart Meter  │──gRPC───>│ Oracle Bridge│
  │ Simulator    │          │              │
  │ :8082        │          │ ── Redis ───│
  │              │          │  (zone       │
  │ ── PG ──────│          │   consumer)  │
  │ ── GIS PG ──│          └──────────────┘
  │ ── Influx ──│
  │ ── Kafka ──▶│ (no consumer in Rust services)
  └──────────────┘
```

### IAM Service Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                  IAM Service (gridtokenx-iam-service)        │
│                                                             │
│  REST API (Axum)         │  gRPC (ConnectRPC/Buffa)         │
│  Port: 8081              │  Port: 8091                      │
│  ├─ POST /register       │  ├─ VerifyToken                  │
│  ├─ POST /token (login)  │  ├─ Authorize                    │
│  ├─ GET /verify          │  ├─ GetUserInfo                  │
│  └─ GET /metrics         │  └─ VerifyApiKey                 │
│                                                             │
│  Middleware Stack: OTel Tracing → Tower Trace → Prometheus  │
│                                                             │
│  Service Layer: AuthService (core business logic)           │
│  Domain Services: JwtService, PasswordService, ApiKeyService│
│  Infrastructure: SQLx PG pool, Redis cache + EventBus       │
│                                                             │
│  Redis Usage:                                               │
│  ├─ CacheService: user profiles, API keys (5min TTL)        │
│  ├─ Rate Limiting: login attempt tracking, account locks    │
│  └─ EventBus: publish events to Redis Streams               │
│     (UserRegistered, UserLoggedIn, EmailVerified, etc.)     │
└─────────────────────────────────────────────────────────────┘
```

### Tech Stack
- **Language**: Rust 2024 Edition
- **Web Framework**: Axum + Tower
- **gRPC**: ConnectRPC + Buffa (code generation from proto files)
- **Database**: PostgreSQL via SQLx (async, compile-time query verification)
- **Cache / Messaging**: Redis 0.32 (ConnectionManager for auto-reconnect, Streams for event bus)
- **Auth**: jsonwebtoken (HS256), argon2, bcrypt
- **Observability**: OpenTelemetry SDK + tracing-opentelemetry + metrics-exporter-prometheus
- **API Docs**: utoipa (OpenAPI spec generation)
- **Config**: dotenvy + config crate

---

## Project Structure

```
gridtokenx-iam-service/
├── Cargo.toml              # Dependencies and build config
├── build.rs                # Protobuf compilation (connectrpc-build)
├── Dockerfile              # Multi-stage Docker build
├── .env.example            # Environment variable template
│
├── proto/
│   └── identity.proto      # gRPC service definition (4 RPCs)
│
├── migrations/             # SQLx migrations (87+ files)
│   ├── 20241101000001_initial_schema.sql
│   ├── ... (87+ migration files)
│   └── 20260402150000_align_meters_schema.sql
│
├── src/
│   ├── main.rs             # Entry point: config, telemetry, signal handling
│   ├── lib.rs              # Module exports
│   ├── startup.rs          # Bootstrap: DB pool, migrations, REST+gRPC servers
│   ├── telemetry.rs        # OpenTelemetry + Prometheus initialization
│   │
│   ├── api/
│   │   ├── mod.rs
│   │   ├── identity_grpc.rs    # gRPC service implementation
│   │   └── handlers/
│   │       ├── mod.rs
│   │       ├── auth.rs         # REST handlers (register, login, verify)
│   │       └── types.rs        # Request/Response DTOs
│   │
│   ├── core/
│   │   ├── mod.rs
│   │   ├── config.rs           # Config struct (from env vars)
│   │   └── error/
│   │       ├── mod.rs
│   │       ├── codes.rs        # ErrorCode enum (AUTH_1001, etc.)
│   │       ├── types.rs        # ApiError enum + HTTP status mapping
│   │       └── helpers.rs      # Error constructors
│   │
│   ├── domain/
│   │   └── identity/
│   │       ├── mod.rs
│   │       ├── auth.rs         # Claims, ApiKey structs
│   │       ├── jwt.rs          # JWT encode/decode/refresh
│   │       ├── password.rs     # Argon2/Bcrypt hashing + validation
│   │       └── roles.rs        # RBAC: Role enum + Permission system
│   │
│   ├── services/
│   │   └── auth_service.rs     # Core auth logic (login, register, verify)
│   │
│   ├── infra/                   # Infrastructure adapters
│   │   ├── cache/
│   │   │   └── mod.rs          # Redis CacheService (get/set/rate-limit/locks)
│   │   └── event_bus/
│   │       └── mod.rs          # Redis Streams event bus (publish events)
│   │
│   └── utils/
│       └── numeric.rs          # Numeric utilities
```

---

## Building and Running

### Prerequisites
- Rust 2024 Edition (toolchain)
- PostgreSQL database
- Protocol Buffers compiler (`protoc`)
- `pkg-config`, `libssl-dev`, `cmake` (system deps)

### Development

```bash
# Build the service
cargo build

# Run locally (requires .env file)
cargo run

# Run with specific log level
RUST_LOG=debug cargo run

# Run tests
cargo test

# Check compilation without running
cargo check

# Format code
cargo fmt

# Lint
cargo clippy
```

### Database Migrations

Migrations run automatically on startup from `./migrations/`. To manage manually:

```bash
# Install SQLx CLI
cargo install sqlx-cli

# Create new migration
sqlx migrate add <migration_name>

# Run migrations
sqlx migrate run

# Revert last migration
sqlx migrate revert
```

### Docker Build

```bash
# Build Docker image
docker build -t gridtokenx-iam-service .

# Run container
docker run -p 8081:8081 -p 8091:8091 \
  -e DATABASE_URL=postgresql://... \
  -e REDIS_URL=redis://... \
  -e JWT_SECRET=your-secret \
  gridtokenx-iam-service
```

### Platform Integration

This service is part of the larger GridTokenX platform. Use the platform management script:

```bash
# From repository root
./scripts/app.sh start        # Start all services
./scripts/app.sh stop         # Stop all services
./scripts/app.sh status       # Check service health
```

---

## Configuration

All configuration via environment variables. Copy `.env.example` to `.env`:

```bash
# Required
DATABASE_URL=postgresql://gridtokenx_user:gridtokenx_password@localhost:5434/gridtokenx
REDIS_URL=redis://localhost:6379

# Optional (with defaults)
IAM_PORT=8081                    # REST API port (gRPC = PORT + 10 = 8091)
JWT_SECRET=dev-jwt-secret-key-minimum-32-characters-long-for-development-2025
JWT_EXPIRATION=86400              # Token TTL in seconds (24 hours)
ENCRYPTION_SECRET=dev-encryption-secret-key-32-chars-long-12345
API_KEY_SECRET=dev-api-key-secret-key-32-chars-long-67890
LOG_LEVEL=info                    # tracing log level
ENVIRONMENT=development           # environment name
TEST_MODE=false                   # test mode flag

# OpenTelemetry (SigNoz)
OTEL_ENABLED=true
OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317
OTEL_SERVICE_NAME=gridtokenx-iam
```

**Config struct** (`src/core/config.rs`):
- Loaded via `Config::from_env()`
- Uses `dotenvy` for `.env` file support
- `DATABASE_URL` and `REDIS_URL` are required (will fail if missing)
- All other values have development defaults

---

## API Endpoints

### REST API

| Method | Path | Handler | Auth | Description |
|--------|------|---------|------|-------------|
| `POST` | `/api/v1/auth/register` | `register` | None | Register new user (username, email, password) |
| `POST` | `/api/v1/auth/token` | `login` | None | Login with username/email + password, get JWT |
| `GET` | `/api/v1/auth/verify` | `verify` | None | Email verification (query param: `token`) |
| `GET` | `/metrics` | `get_metrics` | None | Prometheus metrics endpoint |

**Request/Response Types** (`src/api/handlers/types.rs`):

```rust
// Registration
RegistrationRequest { username, email, password, first_name?, last_name? }
RegistrationResponse { id, username, email, first_name?, last_name?, message }

// Login
LoginRequest { username, password }  // username can be username OR email
AuthResponse { access_token, expires_in, user: UserResponse }

// Email Verification
VerifyEmailRequest { token }         // token format: "verify_{email}"
VerifyEmailResponse { success, message, wallet_address?, auth? }
```

**Password Requirements**:
- Length: 8-128 characters
- At least 1 uppercase, 1 lowercase, 1 digit, 1 special character
- No common weak patterns (password, 123456, qwerty, etc.)

### gRPC API (ConnectRPC)

**Proto file**: `proto/identity.proto`

| RPC Method | Request | Response | Description |
|------------|---------|----------|-------------|
| `VerifyToken` | `TokenRequest { token }` | `ClaimsResponse { valid, user_id, username, role, error_message }` | Validate JWT, extract claims |
| `Authorize` | `AuthorizeRequest { token, required_permission }` | `AuthorizeResponse { authorized, error_message }` | Role-based permission check |
| `GetUserInfo` | `TokenRequest { token }` | `UserInfoResponse { id, username, email, role, first_name, last_name, wallet_address }` | Get user profile from JWT |
| `VerifyApiKey` | `ApiKeyRequest { key }` | `ApiKeyResponse { valid, role, error_message }` | Validate API key for AMI systems |

**Authorization Logic**:
- `admin` role → always authorized
- `user` role → blocked from `admin:*` permissions
- Other roles → check against role's permission set

---

## Database Schema

Managed via SQLx migrations in `migrations/` directory (87+ migration files as of 2026-04-02).

### Core Tables

| Table | Purpose |
|-------|---------|
| `users` | User accounts (email, username, password_hash, wallet_address, role enum) |
| `api_keys` | API keys for AMI/machine-to-machine auth (SHA256 hashed) |
| `market_epochs` | 15-minute trading windows |
| `trading_orders` | Energy trading orders (buy/sell) |
| `order_matches` | Matched buy/sell order pairs |
| `settlements` | Settlement records for matched orders |
| `meter_readings` | Smart meter IoT data (partitioned by time) |
| `energy_certificates` | RECs/ERCs/IRECs (renewable energy certificates) |
| `blockchain_transactions` | Solana transaction tracking |
| `user_activities` | Audit log |

### User Role Enum

```sql
CREATE TYPE user_role AS ENUM ('user', 'admin', 'ami', 'producer', 'consumer', 'operator');
```

**Note**: Database currently constrains to `user` and `admin` only via CHECK constraint. Code supports all 6 roles.

---

## Development Conventions

### Error Handling

- Use `ApiError` enum (`src/core/error/types.rs`) for all application errors
- Error codes defined in `ErrorCode` enum (`src/core/error/codes.rs`)
- Pattern: `AUTH_1001`, `VAL_3001`, `DB_7001`, `INTERNAL_9001`
- HTTP status codes mapped automatically from error type
- Use `anyhow::Result` for external error propagation, convert to `ApiError` at boundaries

### Password Security

- **Hashing**: Argon2 (default), with Bcrypt legacy support (`$2` prefix detection)
- **Verification**: Runs in `tokio::task::spawn_blocking` to avoid blocking async runtime
- **Strength validation**: Enforced before hashing (length, complexity, pattern checks)

### JWT Implementation

- **Algorithm**: HMAC-SHA256 (symmetric)
- **Claims**: `sub` (UUID), `username`, `role`, `exp`, `iat`, `iss` ("gridtokenx-iam-service")
- **Expiration**: 24 hours (configurable via `JWT_EXPIRATION`)
- **Validation**: Signature, expiration, issuer check

### Code Organization

- **Dependency Injection**: Services initialized in `startup.rs`, passed via `State<T>` (Axum) or struct fields (gRPC)
- **Domain Logic**: In `src/domain/` (pure business logic, no I/O)
- **Services**: In `src/services/` (orchestration, I/O, database access)
- **Handlers**: Thin layer, delegate to services, handle metrics/tracing
- **Error Types**: Rich enum with HTTP status mapping, error codes, structured details

### Testing

- Unit tests in module `#[cfg(test)]` blocks (see `roles.rs`, `auth.rs`)
- Integration tests not present in this service (handled at platform level)
- Use `TEST_MODE=true` for test-specific behavior

### Observability

**OpenTelemetry**:
- Traces exported via OTLP to SigNoz (`http://otel-collector:4317`)
- Fallback to JSON-formatted local logging if OTLP unavailable
- Auto-instrumented HTTP requests (method, route, status, duration, client IP)
- Service name and environment attached to all spans

**Prometheus Metrics**:
- Exposed at `GET /metrics`
- Custom metrics: auth attempts, user operations, JWT operations, API key operations
- Histogram buckets: 0.001s to 10.0s

### Logging

- Structured JSON logging via `tracing-subscriber`
- Log levels: `error`, `warn`, `info`, `debug`, `trace`
- Key events logged: registration, login, verification, errors
- Use `tracing::info!`, `tracing::error!`, `tracing::warn!`, `tracing::debug!`

---

## Common Workflows

### User Registration Flow

```
Client → POST /api/v1/auth/register
       → Validate password strength
       → Hash password (Argon2, blocking thread)
       → INSERT INTO users (role='user', is_active=true)
       → Return { id, username, email, message }
```

### User Login Flow

```
Client → POST /api/v1/auth/token
       → Query users WHERE (username OR email) = $1 AND is_active = true
       → Verify password (Argon2 or Bcrypt, blocking thread)
       → Generate JWT (HS256, 24h expiration)
       → Return { access_token, expires_in, user }
```

### Email Verification Flow

```
Client → GET /api/v1/verify?token=verify_user@example.com
       → Extract email from token (strip "verify_" prefix)
       → UPDATE users SET is_active=true, wallet_address=COALESCE(...)
       → Generate JWT token
       → Return { success, message, wallet_address, auth }
```

**Note**: Email verification uses simplified token format (`verify_{email}`) for E2E testing. Production should use cryptographically secure tokens with expiration.

### Cross-Service Authorization (gRPC)

**User-facing track** (through API Gateway):

```
Browser → Kong :4000 → API Gateway
                              │
                              ├─ gRPC :8091 → IAM VerifyToken
                              │             → Return { valid, user_id, role }
                              │
                              ├─ gRPC :8091 → IAM Authorize
                              │             → Return { authorized }
                              │
                              └─ gRPC :8093 → Trading Service (if authorized)
```

**IoT-facing track** (independent):

```
Smart Meter Simulator → gRPC :50051 → Oracle Bridge
                                      │
                                      └─ gRPC :8091 → IAM VerifyApiKey
                                                    → Return { valid, role }
```

**IAM never calls other services** — it is a pure server (REST + gRPC).
**IAM and Trading never talk to each other.**

---

## Key Implementation Details

### Startup Sequence (`src/startup.rs`)

1. Connect to PostgreSQL (`PgPoolOptions`, max 5 connections)
2. Run all migrations from `./migrations/`
3. Initialize Redis `CacheService` (ConnectionManager, ping verify)
4. Initialize Redis `EventBus` (Streams, trim policy)
5. Initialize `JwtService`, `ApiKeyService`, `AuthService`
6. Build REST router with middleware layers
7. Build gRPC server via `buffa` code generation
8. Spawn REST and gRPC servers concurrently
9. Wait for `CancellationToken` (graceful shutdown on SIGINT/SIGTERM)

### gRPC Code Generation (`build.rs`)

```rust
connectrpc_build::Config::new()
    .files(&["proto/identity.proto"])
    .includes(&["proto"])
    .include_file("_identity_include.rs")
    .compile()?;
```

Generated code included via `include!(concat!(env!("OUT_DIR"), "/_identity_include.rs"))` in `identity_grpc.rs`.

### Database Query Patterns

- Use `sqlx::query_as!` for compile-time verified queries (when SQLx cache available)
- Use `sqlx::query_as::<_, T>()` for runtime-checked queries
- Custom enums require `::text` casting: `role::text as role`
- Use `spawn_blocking` for CPU-intensive operations (password hashing/verification)

### Error Conversion

```rust
impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        match err.downcast::<ApiError>() {
            Ok(api_err) => api_err,
            Err(e) => ApiError::Internal(e.to_string()),
        }
    }
}
```

---

## Known Limitations & Technical Debt

1. **Email Verification**: Simplified token format (`verify_{email}`) for E2E testing only. Not suitable for production.
2. **Redis Rate Limiting**: Uses simple counter-based approach (5 attempts → 15-min lock). No sliding window or token bucket yet.
3. **Redis Cache Invalidation**: User profiles cached for 5min TTL. No write-through or cache invalidation on user updates yet.
4. **Event Bus Publisher Only**: IAM publishes events to Redis Streams but doesn't consume from them. Other services (API Gateway) can subscribe.
5. **Mock Wallets**: Auto-generated during email verification if not provided.
6. **User Role Constraint**: Database limits to `user`/`admin` only, but code supports 6 roles.
7. **GetUserInfo Response**: Returns empty strings for `email`, `first_name`, `last_name`, `wallet_address` (not in JWT claims).
8. **Password Reset**: Migrations exist (`20251220000001_add_password_reset.sql`) but no implementation in current code.

---

## Useful Commands

```bash
# Build
cargo build

# Run (requires .env)
cargo run

# Test
cargo test

# Check compilation
cargo check

# Format
cargo fmt

# Lint
cargo clippy -- -D warnings

# Run with debug logging
RUST_LOG=debug cargo run

# Build Docker image
docker build -t gridtokenx-iam-service .

# Database migrations (if SQLx CLI installed)
sqlx migrate run
sqlx migrate add <name>
```

---

## Related Services

### Behind API Gateway (user-facing track)

| Service | Connection to IAM | Purpose |
|---------|-------------------|---------|
| **API Gateway** (`gridtokenx-api`) | gRPC :8091 — `VerifyToken`, `Authorize`, `GetUserInfo`, `VerifyApiKey` | AuthN/AuthZ for every user request |
| **Trading Service** (`gridtokenx-trading-service`) | *(no direct connection)* | Independent — no gRPC between IAM and Trading |

### Independent (IoT-facing track — not behind API Gateway)

| Service | Connection to IAM | Purpose |
|---------|-------------------|---------|
| **Oracle Bridge** (`gridtokenx-oracle-bridge`) | gRPC :8091 — `VerifyApiKey` | Authenticates meter data submissions |
| **Smart Meter Simulator** (`gridtokenx-smartmeter-simulator`) | *(no direct connection)* | Independent — talks to Oracle Bridge via gRPC |

---

## External Resources

- [Axum Documentation](https://docs.rs/axum/)
- [SQLx Documentation](https://github.com/launchbadge/sqlx)
- [ConnectRPC Rust](https://github.com/connectrpc/connect-rs)
- [OpenTelemetry Rust](https://github.com/open-telemetry/opentelemetry-rust)
- [jsonwebtoken Documentation](https://docs.rs/jsonwebtoken/)
- [Argon2 Rust](https://docs.rs/argon2/)

---

*Last updated: 2026-04-06*
