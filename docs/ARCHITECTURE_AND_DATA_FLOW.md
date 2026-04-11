# GridTokenX IAM Service - Architecture & Data Flow Documentation

## Table of Contents
1. [Service Overview](#1-service-overview)
2. [System Architecture](#2-system-architecture)
3. [Database Schema](#3-database-schema)
4. [Authentication & Authorization Protocols](#4-authentication--authorization-protocols)
5. [Data Flow Diagrams](#5-data-flow-diagrams)
6. [API Endpoints](#6-api-endpoints)
7. [Configuration](#7-configuration)
8. [Observability](#8-observability)

---

## 1. Service Overview

**GridTokenX IAM Service** is a dual-protocol Identity and Access Management microservice built in Rust, providing both REST and gRPC interfaces for authentication, authorization, and user management within the GridTokenX P2P energy trading platform.

### Key Characteristics
- **Dual Protocol**: REST (Axum) + gRPC (ConnectRPC via buffa)
- **Authentication**: JWT (HS256) + API Keys (SHA256)
- **Password Hashing**: Argon2 (primary) + Bcrypt (legacy support)
- **Authorization**: Role-Based Access Control (RBAC)
- **Database**: PostgreSQL with SQLx (async, compile-time checked)
- **Observability**: OpenTelemetry tracing + Prometheus metrics
- **Ports**: REST (default 8081), gRPC (default 8091)

### Core Responsibilities
- User registration and login
- JWT token generation and validation
- API key verification
- Email verification
- Role-based authorization
- User profile management
- Audit logging

---

## 2. System Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        GridTokenX Platform                          │
└─────────────────────────────────────────────────────────────────────┘
                                     │
                                     │ gRPC / REST
                                     ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      IAM Service (Port 8081/8091)                   │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    Protocol Layer                              │  │
│  │  ┌─────────────────────┐    ┌──────────────────────────────┐  │  │
│  │  │   REST API (Axum)   │    │  gRPC (ConnectRPC/Buffa)     │  │  │
│  │  │   Port: 8081        │    │  Port: 8091                  │  │  │
│  │  └─────────────────────┘    └──────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                             │                                        │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                  Middleware Stack                              │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌─────────────────────┐ │  │
│  │  │ OTel Tracing │→ │ Tower Trace  │→ │  Prometheus Metrics │ │  │
│  │  └──────────────┘  └──────────────┘  └─────────────────────┘ │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                             │                                        │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    Service Layer                               │  │
│  │  ┌─────────────────────────────────────────────────────────┐  │  │
│  │  │           AuthService (Core Business Logic)             │  │  │
│  │  │  - User Registration                                   │  │  │
│  │  │  - Login/Authentication                                │  │  │
│  │  │  - Email Verification                                  │  │  │
│  │  │  - API Key Verification                                │  │  │
│  │  └─────────────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                             │                                        │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                  Domain Services                               │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌─────────────────────┐ │  │
│  │  │ JwtService   │  │ PasswordSvc  │  │  ApiKeyService      │ │  │
│  │  │ (HS256 JWT)  │  │ (Argon2)     │  │  (SHA256 Hashing)   │ │  │
│  │  └──────────────┘  └──────────────┘  └─────────────────────┘ │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                             │                                        │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │               Infrastructure Layer (SQLx)                      │  │
│  │                      PostgreSQL Pool                           │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                                     │
                                     │ SQL (async)
                                     ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        PostgreSQL Database                          │
│                 (Users, Orders, Settlements, etc.)                  │
└─────────────────────────────────────────────────────────────────────┘
```

### Module Structure

```
src/
├── main.rs                 # Entry point, config loading, signal handling
├── lib.rs                  # Module exports
├── startup.rs              # Service bootstrap (DB, migrations, servers)
├── telemetry.rs            # OpenTelemetry + Prometheus init
│
├── api/
│   ├── mod.rs
│   ├── identity_grpc.rs    # gRPC service implementation
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── auth.rs         # REST: register, login, verify
│   │   └── types.rs        # Request/Response DTOs
│   └── middleware/
│       ├── mod.rs
│       ├── metrics.rs      # Prometheus metrics
│       └── otel_tracing.rs # Distributed tracing
│
├── core/
│   ├── mod.rs
│   ├── config.rs           # Environment configuration
│   └── error/
│       ├── mod.rs
│       ├── codes.rs        # Error codes (AUTH_1001, etc.)
│       ├── types.rs        # ApiError enum
│       └── helpers.rs      # Error helpers
│
├── domain/
│   ├── mod.rs
│   └── identity/
│       ├── mod.rs
│       ├── auth.rs         # Claims, ApiKey structs
│       ├── jwt.rs          # JWT service (encode/decode)
│       ├── password.rs     # Password hashing (Argon2/Bcrypt)
│       └── roles.rs        # RBAC: Role, Permission
│
├── services/
│   ├── mod.rs
│   └── auth_service.rs     # Core auth business logic
│
└── utils/
    ├── mod.rs
    └── numeric.rs          # Numeric utilities
```

### Startup Sequence

```
┌─ main.rs ──────────────────────────────────────────────────────┐
│                                                                 │
│  1. Load .env file (dotenvy)                                   │
│  2. Initialize Config from environment variables               │
│  3. Initialize Telemetry (OTel + Prometheus)                   │
│  4. Call startup::run()                                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─ startup.rs ─────────────────────────────────────────────────┐
│                                                               │
│  5. Create PostgreSQL connection pool (PgPoolOptions)        │
│     - Max connections: 5                                     │
│     - Run migrations from ./migrations/                      │
│                                                               │
│  6. Initialize Domain Services:                              │
│     ├─ JwtService (JWT_SECRET, JWT_EXPIRATION)               │
│     ├─ ApiKeyService (API_KEY_SECRET)                        │
│     └─ AuthService (pool, jwt_svc, apikey_svc)               │
│                                                               │
│  7. Build REST Router (Axum):                                │
│     ├─ Middleware: OTel Tracing                               │
│     ├─ Middleware: Tower Trace                                │
│     ├─ Middleware: Prometheus Metrics                         │
│     └─ Routes: /api/v1/auth/*, /verify, /metrics              │
│                                                               │
│  8. Build gRPC Server (ConnectRPC via buffa):                │
│     └─ IdentityService (VerifyToken, Authorize, etc.)        │
│                                                               │
│  9. Spawn Concurrent Servers:                                │
│     ├─ REST Server (tokio::spawn)                            │
│     └─ gRPC Server (tokio::spawn)                            │
│                                                               │
│  10. Wait for CancellationToken (graceful shutdown)           │
│      - Handle SIGINT/SIGTERM                                 │
│      - Close database pool                                   │
│      - Shutdown servers                                      │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

---

## 3. Database Schema

### Entity Relationship Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                           USERS                                     │
├─────────────────────────────────────────────────────────────────────┤
│ PK  id                  UUID                                        │
│     email               VARCHAR(255) UNIQUE                         │
│     username            VARCHAR(100) UNIQUE                         │
│     password_hash       TEXT                                        │
│     wallet_address      VARCHAR(100)                                │
│     role                ENUM (user, admin)                          │
│     first_name          VARCHAR(100)                                │
│     last_name           VARCHAR(100)                                │
│     is_active           BOOLEAN DEFAULT FALSE                       │
│     email_verified      BOOLEAN DEFAULT FALSE                       │
│     email_verification_token VARCHAR(255)                           │
│     created_at          TIMESTAMPTZ DEFAULT NOW()                   │
│     updated_at          TIMESTAMPTZ DEFAULT NOW()                   │
└─────────────────────────────────────────────────────────────────────┘
         │
         │ 1:N
         ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        TRADING_ORDERS                               │
├─────────────────────────────────────────────────────────────────────┤
│ PK  id                  UUID                                        │
│ FK  user_id             REFERENCES users(id)                        │
│ FK  epoch_id            REFERENCES market_epochs(id)                │
│     order_type          ENUM (buy, sell)                            │
│     energy_amount       DECIMAL                                     │
│     price_per_kwh       DECIMAL                                     │
│     filled_amount       DECIMAL DEFAULT 0                           │
│     status              ENUM (pending, active, filled, etc.)        │
│     expires_at          TIMESTAMPTZ                                 │
│     created_at          TIMESTAMPTZ DEFAULT NOW()                   │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                         API_KEYS                                    │
├─────────────────────────────────────────────────────────────────────┤
│ PK  id                  UUID                                        │
│     key_hash            VARCHAR(64) UNIQUE  (SHA256)                │
│     name                VARCHAR(100)                                │
│     role                VARCHAR(50)                                 │
│     permissions         TEXT[] (Array)                              │
│     is_active           BOOLEAN DEFAULT TRUE                        │
│     created_at          TIMESTAMPTZ DEFAULT NOW()                   │
│     last_used_at        TIMESTAMPTZ                                 │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                       MARKET_EPOCHS                                 │
├─────────────────────────────────────────────────────────────────────┤
│ PK  id                  UUID                                        │
│     epoch_number        INTEGER UNIQUE                              │
│     start_time          TIMESTAMPTZ                                 │
│     end_time            TIMESTAMPTZ                                 │
│     status              ENUM (pending, active, cleared, settled)    │
│     clearing_price      DECIMAL                                     │
│     total_volume        DECIMAL                                     │
│     created_at          TIMESTAMPTZ DEFAULT NOW()                   │
└─────────────────────────────────────────────────────────────────────┘
         │
         │ 1:N
         ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       ORDER_MATCHES                                 │
├─────────────────────────────────────────────────────────────────────┤
│ PK  id                  UUID                                        │
│ FK  epoch_id            REFERENCES market_epochs(id)                │
│ FK  buy_order_id        REFERENCES trading_orders(id)               │
│ FK  sell_order_id       REFERENCES trading_orders(id)               │
│     matched_amount      DECIMAL                                     │
│     match_price         DECIMAL                                     │
│     status              ENUM (pending, settled, failed)             │
│     settlement_id       UUID                                        │
│     created_at          TIMESTAMPTZ DEFAULT NOW()                   │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                        SETTLEMENTS                                  │
├─────────────────────────────────────────────────────────────────────┤
│ PK  id                  UUID                                        │
│ FK  epoch_id            REFERENCES market_epochs(id)                │
│ FK  buyer_id            REFERENCES users(id)                        │
│ FK  seller_id           REFERENCES users(id)                        │
│     energy_amount       DECIMAL                                     │
│     price_per_kwh       DECIMAL                                     │
│     total_amount        DECIMAL                                     │
│     fee_amount          DECIMAL                                     │
│     net_amount          DECIMAL                                     │
│     status              VARCHAR(50)                                 │
│     transaction_hash    VARCHAR(255)                                │
│     created_at          TIMESTAMPTZ DEFAULT NOW()                   │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                       METER_READINGS                                │
├─────────────────────────────────────────────────────────────────────┤
│ PK  id                  UUID                                        │
│     meter_id            VARCHAR(100)                                │
│     wallet_address      VARCHAR(100)                                │
│ FK  user_id             REFERENCES users(id)                        │
│     timestamp           TIMESTAMPTZ                                 │
│     energy_generated    DECIMAL                                     │
│     energy_consumed     DECIMAL                                     │
│     energy_surplus      DECIMAL                                     │
│     energy_deficit      DECIMAL                                     │
│     battery_level       DECIMAL                                     │
│     temperature         DECIMAL                                     │
│     voltage             DECIMAL                                     │
│     current             DECIMAL                                     │
│     kwh_amount          DECIMAL                                     │
│     minted              BOOLEAN DEFAULT FALSE                       │
│     mint_signature      VARCHAR(255)                                │
│     mint_tx_signature   VARCHAR(255)                                │
│     reading_timestamp     TIMESTAMPTZ                               │
│     submitted_at        TIMESTAMPTZ DEFAULT NOW()                   │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                     ERC_CERTIFICATES                                │
├─────────────────────────────────────────────────────────────────────┤
│ PK  id                  UUID                                        │
│     certificate_id      VARCHAR(100) UNIQUE                         │
│     wallet_address      VARCHAR(100)                                │
│ FK  user_id             REFERENCES users(id)                        │
│     energy_amount       DECIMAL                                     │
│     kwh_amount          DECIMAL                                     │
│     certificate_type    ENUM (REC, ERC, IREC)                       │
│     issuance_date       DATE                                        │
│     issue_date          DATE                                        │
│     expiry_date         DATE                                        │
│     status              VARCHAR(50)                                 │
│     energy_source       VARCHAR(100)                                │
│     vintage_year        INTEGER                                     │
│     issuer_wallet       VARCHAR(100)                                │
│     blockchain_tx_signature VARCHAR(255)                            │
│     metadata            JSONB                                       │
│     created_at          TIMESTAMPTZ DEFAULT NOW()                   │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                 ERC_CERTIFICATE_TRANSFERS                           │
├─────────────────────────────────────────────────────────────────────┤
│ PK  id                  UUID                                        │
│     certificate_id      VARCHAR(100)                                │
│ FK  from_user_id        REFERENCES users(id)                        │
│ FK  to_user_id          REFERENCES users(id)                        │
│     transfer_date       DATE                                        │
│     from_wallet       VARCHAR(100)                                │
│     to_wallet         VARCHAR(100)                                │
│     blockchain_tx_signature VARCHAR(255)                            │
│     created_at          TIMESTAMPTZ DEFAULT NOW()                   │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                   BLOCKCHAIN_TRANSACTIONS                           │
├─────────────────────────────────────────────────────────────────────┤
│ PK  id                  UUID                                        │
│     signature           VARCHAR(255)                                │
│ FK  user_id             REFERENCES users(id)                        │
│     program_id          VARCHAR(100)                                │
│     instruction_name    VARCHAR(100)                                │
│     status              ENUM (pending, confirmed, failed)           │
│     fee                 BIGINT                                      │
│     compute_units_consumed BIGINT                                   │
│     created_at          TIMESTAMPTZ DEFAULT NOW()                   │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                        AUDIT_LOGS                                   │
├─────────────────────────────────────────────────────────────────────┤
│ PK  id                  UUID                                        │
│     event_type          VARCHAR(100)                                │
│ FK  user_id             REFERENCES users(id)                        │
│     ip_address          INET                                        │
│     event_data          JSONB                                       │
│     created_at          TIMESTAMPTZ DEFAULT NOW()                   │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                      USER_ACTIVITIES                                │
├─────────────────────────────────────────────────────────────────────┤
│ PK  id                  UUID                                        │
│ FK  user_id             REFERENCES users(id)                        │
│     activity_type       VARCHAR(100)                                │
│     description         TEXT                                        │
│     ip_address          INET                                        │
│     user_agent          TEXT                                        │
│     metadata            JSONB                                       │
│     created_at          TIMESTAMPTZ DEFAULT NOW()                   │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 4. Authentication & Authorization Protocols

### 4.1 User Registration Protocol

**Purpose**: Create a new user account with validated credentials

**Protocol Type**: Synchronous REST API (POST)

**Steps**:

```
┌──────────┐                                    ┌──────────────────┐
│  Client  │                                    │  IAM Service     │
│          │                                    │                  │
│  POST /api/v1/auth/register                  │                  │
│  { username, email, password }               │                  │
│─────────────────────────────────────────────>│                  │
│          │                                    │  1. Validate     │
│          │                                    │     input data   │
│          │                                    │                  │
│          │                                    │  2. Check        │
│          │                                    │     duplicates   │
│          │                                    │     (email, user)│
│          │                                    │                  │
│          │                                    │  3. Hash password│
│          │                                    │     (Argon2)     │
│          │                                    │                  │
│          │                                    │  4. Insert into  │
│          │                                    │     users table  │
│          │                                    │     role="user"  │
│          │                                    │     is_active=T  │
│          │                                    │                  │
│          │  201 Created                       │                  │
│          │  { id, username, email, message }  │                  │
│<─────────────────────────────────────────────│                  │
│          │                                    │                  │
└──────────┘                                    └──────────────────┘
```

**Password Requirements**:
- Length: 8-128 characters
- At least one uppercase letter
- At least one lowercase letter
- At least one digit
- At least one special character
- No common patterns (e.g., "password123")

**Database Operations**:
```sql
INSERT INTO users (
    id, email, username, password_hash,
    role, is_active, email_verified,
    email_verification_token, created_at, updated_at
) VALUES (
    gen_random_uuid(), $1, $2, $3,
    'user', true, false,
    NULL, NOW(), NOW()
) RETURNING id, username, email, first_name, last_name;
```

**Security Considerations**:
- Password validated before hashing
- Argon2 hashing runs in blocking thread (spawn_blocking) to avoid blocking async runtime
- No JWT token issued at registration (user must login separately)
- Email not verified at registration (requires separate verification step)

---

### 4.2 User Login Protocol

**Purpose**: Authenticate user and issue JWT access token

**Protocol Type**: Synchronous REST API (POST)

**Steps**:

```
┌──────────┐                                    ┌──────────────────┐
│  Client  │                                    │  IAM Service     │
│          │                                    │                  │
│  POST /api/v1/auth/token                     │                  │
│  { username/email, password }                │                  │
│─────────────────────────────────────────────>│                  │
│          │                                    │  1. Query DB:    │
│          │                                    │     SELECT *     │
│          │                                    │     FROM users   │
│          │                                    │     WHERE        │
│          │                                    │     (username OR │
│          │                                    │      email) = $1 │
│          │                                    │     AND          │
│          │                                    │     is_active=T  │
│          │                                    │                  │
│          │                                    │  2. Verify       │
│          │                                    │     password     │
│          │                                    │     (Argon2 or   │
│          │                                    │      Bcrypt)     │
│          │                                    │     [spawn_block]│
│          │                                    │                  │
│          │                                    │  3. Generate JWT │
│          │                                    │     (HS256, 24h) │
│          │                                    │                  │
│          │                                    │  4. Build        │
│          │                                    │     response     │
│          │                                    │                  │
│          │  200 OK                            │                  │
│          │  { access_token, expires_in, user }│                  │
│<─────────────────────────────────────────────│                  │
│          │                                    │                  │
└──────────┘                                    └──────────────────┘
```

**JWT Claims Structure**:
```json
{
  "sub": "<user_id>",
  "username": "<username>",
  "role": "<user|admin>",
  "email": "<email>",
  "iat": 1234567890,
  "exp": 1234654290,
  "iss": "gridtokenx-iam-service"
}
```

**Password Verification Logic**:
```
IF hash starts with "$argon2" → Use Argon2 verification
IF hash starts with "$2"     → Use Bcrypt verification (legacy)
ELSE                          → Return error
```

**Database Query**:
```sql
SELECT id, email, username, password_hash, role,
       first_name, last_name, wallet_address, is_active
FROM users
WHERE (username = $1 OR email = $1) AND is_active = true;
```

**Error Scenarios**:
- User not found → `AUTH_1001`: "Invalid credentials"
- Password mismatch → `AUTH_1001`: "Invalid credentials"
- Account inactive → `AUTH_1003`: "Account is not active"
- Hash verification failure → `AUTH_1005`: "Password verification failed"

---

### 4.3 Email Verification Protocol

**Purpose**: Verify user email address and activate account

**Protocol Type**: Synchronous REST API (GET)

**Token Format**: `verify_{email}` (simplified for E2E testing)

**Steps**:

```
┌──────────┐                                    ┌──────────────────┐
│  Client  │                                    │  IAM Service     │
│          │                                    │                  │
│  GET /api/v1/verify?token=verify_user@ex.com │                  │
│─────────────────────────────────────────────>│                  │
│          │                                    │  1. Extract email│
│          │                                    │     from token   │
│          │                                    │     (strip       │
│          │                                    │      "verify_")  │
│          │                                    │                  │
│          │                                    │  2. Query DB:    │
│          │                                    │     UPDATE users │
│          │                                    │     SET          │
│          │                                    │     is_active=T, │
│          │                                    │     email_ver=T, │
│          │                                    │     wallet_addr  │
│          │                                    │     = COALESCE(  │
│          │                                    │       wallet,    │
│          │                                    │       mock_addr) │
│          │                                    │     WHERE        │
│          │                                    │     email = $1   │
│          │                                    │                  │
│          │                                    │  3. Generate JWT │
│          │                                    │     token (24h)  │
│          │                                    │                  │
│          │  200 OK                            │                  │
│          │  { success, message, wallet, auth }│                  │
│<─────────────────────────────────────────────│                  │
│          │                                    │                  │
└──────────┘                                    └──────────────────┘
```

**Wallet Address Generation** (if not provided):
```
Format: mock_{uuid}@wallet.gridtokenx.local
Example: mock_a1b2c3d4-e5f6-7890-abcd-ef1234567890@wallet.gridtokenx.local
```

**Database Update**:
```sql
UPDATE users
SET is_active = true,
    email_verified = true,
    wallet_address = COALESCE(wallet_address, $1),
    updated_at = NOW()
WHERE email = $2
RETURNING id, email, wallet_address;
```

**Note**: This is a simplified E2E testing implementation. Production should use:
- Cryptographically secure verification tokens
- Separate `email_verification_tokens` table
- Token expiration and single-use constraints

---

### 4.4 JWT Token Verification Protocol (gRPC)

**Purpose**: Validate JWT tokens for other microservices

**Protocol Type**: Synchronous gRPC (ConnectRPC)

**Steps**:

```
┌──────────┐                                    ┌──────────────────┐
│  Client  │                                    │  IAM Service     │
│  (e.g.,  │                                    │                  │
│   API    │                                    │                  │
│   Gtwy)  │                                    │                  │
│          │                                    │                  │
│  VerifyToken RPC                             │                  │
│  TokenRequest { token: "<jwt>" }             │                  │
│─────────────────────────────────────────────>│                  │
│          │                                    │  1. Decode JWT   │
│          │                                    │     - Validate   │
│          │                                    │       signature  │
│          │                                    │       (HS256)    │
│          │                                    │     - Check      │
│          │                                    │       expiration │
│          │                                    │     - Verify     │
│          │                                    │       issuer     │
│          │                                    │                  │
│          │                                    │  2. Extract      │
│          │                                    │     claims       │
│          │                                    │                  │
│          │  ClaimsResponse                    │                  │
│  { valid, user_id, username, role, error }   │                  │
│<─────────────────────────────────────────────│                  │
│          │                                    │                  │
└──────────┘                                    └──────────────────┘
```

**Validation Checks**:
1. **Signature**: HMAC-SHA256 with JWT_SECRET
2. **Expiration**: `exp` claim must be in the future
3. **Issuer**: `iss` claim must equal "gridtokenx-iam-service"
4. **Required Claims**: `sub`, `username`, `role`, `exp`, `iss`

**Response Scenarios**:
- Valid token → `valid: true`, claims populated
- Expired token → `valid: false`, error: "Token expired"
- Invalid signature → `valid: false`, error: "Invalid token signature"
- Missing claims → `valid: false`, error: "Missing required claims"

---

### 4.5 API Key Verification Protocol

**Purpose**: Validate API keys for machine-to-machine authentication (e.g., AMI systems)

**Protocol Type**: Synchronous gRPC (ConnectRPC)

**API Key Generation**:
```
key_hash = SHA256(api_key + API_KEY_SECRET)
```

**Steps**:

```
┌──────────┐                                    ┌──────────────────┐
│  Client  │                                    │  IAM Service     │
│  (AMI)   │                                    │                  │
│          │                                    │                  │
│  VerifyApiKey RPC                            │                  │
│  ApiKeyRequest { key: "<raw_api_key>" }      │                  │
│─────────────────────────────────────────────>│                  │
│          │                                    │  1. Hash key:    │
│          │                                    │     SHA256(key + │
│          │                                    │           secret)│
│          │                                    │                  │
│          │                                    │  2. Query DB:    │
│          │                                    │     SELECT *     │
│          │                                    │     FROM api_keys│
│          │                                    │     WHERE        │
│          │                                    │     key_hash=$1  │
│          │                                    │     AND          │
│          │                                    │     is_active=T  │
│          │                                    │                  │
│          │                                    │  3. Update       │
│          │                                    │     last_used_at │
│          │                                    │                  │
│          │  ApiKeyResponse                    │                  │
│  { valid, role, error_message }              │                  │
│<─────────────────────────────────────────────│                  │
│          │                                    │                  │
└──────────┘                                    └──────────────────┘
```

**Database Operations**:
```sql
-- Lookup
SELECT id, role, permissions, is_active
FROM api_keys
WHERE key_hash = $1 AND is_active = true;

-- Update last used
UPDATE api_keys
SET last_used_at = NOW()
WHERE key_hash = $1;
```

**Security Considerations**:
- Raw API key never stored in database
- Only SHA256 hash stored (one-way)
- `last_used_at` updated on every successful verification
- Permissions stored as PostgreSQL array (e.g., `['meter:read', 'meter:write']`)

---

### 4.6 Role-Based Authorization Protocol

**Purpose**: Check if user has permission to perform an action

**Protocol Type**: Synchronous gRPC (ConnectRPC)

**Roles & Permissions**:

```
┌──────────────────────────────────────────────────────────────────┐
│                        ROLE HIERARCHY                            │
└──────────────────────────────────────────────────────────────────┘

┌─────────────────┐         ┌─────────────────┐
│     Admin       │         │      User       │
│                 │         │                 │
│  Permissions:   │         │  Permissions:   │
│  - admin:*      │         │  - user:*       │
│  - user:*       │         │  - energy:read  │
│  - energy:*     │         │  - order:*      │
│  - order:*      │         │  - meter:read   │
│  - meter:*      │         │                 │
│  - system:*     │         │  BLOCKED:       │
│                 │         │  - admin:*      │
└─────────────────┘         └─────────────────┘

Note: Database currently limits roles to "user" and "admin" only.
      Code defines 6 roles: User, Admin, AMI, Producer, Consumer, Operator
```

**Permission Format**: `"resource:action"`

**Examples**:
- `admin:*` - All admin actions
- `energy:read` - Read energy data
- `order:create` - Create orders
- `meter:write` - Write meter readings
- `user:profile` - Access user profile

**Authorization Flow**:

```
┌──────────┐                                    ┌──────────────────┐
│  Client  │                                    │  IAM Service     │
│          │                                    │                  │
│  Authorize RPC                               │                  │
│  { token, required_permission }              │                  │
│─────────────────────────────────────────────>│                  │
│          │                                    │  1. Decode JWT   │
│          │                                    │     (HS256)      │
│          │                                    │                  │
│          │                                    │  2. Extract role │
│          │                                    │     from claims  │
│          │                                    │                  │
│          │                                    │  3. Check perms: │
│          │                                    │                  │
│          │                                    │     IF role=admin│
│          │                                    │       → authorized│
│          │                                    │       (all perms)│
│          │                                    │                  │
│          │                                    │     IF role=user │
│          │                                    │       → block if │
│          │                                    │       perm starts│
│          │                                    │       with "adm" │
│          │                                    │                  │
│          │                                    │     ELSE         │
│          │                                    │       → check    │
│          │                                    │       role perms │
│          │                                    │                  │
│          │  AuthorizeResponse                 │                  │
│  { authorized, error_message }               │                  │
│<─────────────────────────────────────────────│                  │
│          │                                    │                  │
└──────────┘                                    └──────────────────┘
```

**Authorization Logic** (pseudo-code):
```rust
fn authorize(role: Role, required_permission: &str) -> bool {
    match role {
        Role::Admin => true,  // Admin has all permissions
        
        Role::User => {
            // Users blocked from admin permissions
            if required_permission.starts_with("admin:") {
                return false;
            }
            // Check user permissions
            self.permissions().contains(required_permission)
                || self.permissions().contains("user:*")
        }
        
        _ => {
            // Other roles: check specific permissions
            self.permissions().contains(required_permission)
                || self.permissions().contains(&format!("{}:*", resource))
        }
    }
}
```

---

### 4.7 Get User Info Protocol

**Purpose**: Retrieve user profile from JWT token

**Protocol Type**: Synchronous gRPC (ConnectRPC)

**Steps**:

```
┌──────────┐                                    ┌──────────────────┐
│  Client  │                                    │  IAM Service     │
│          │                                    │                  │
│  GetUserInfo RPC                             │                  │
│  TokenRequest { token: "<jwt>" }             │                  │
│─────────────────────────────────────────────>│                  │
│          │                                    │  1. Decode JWT   │
│          │                                    │     (validate    │
│          │                                    │      sig, exp)   │
│          │                                    │                  │
│          │                                    │  2. Extract      │
│          │                                    │     claims       │
│          │                                    │                  │
│          │  UserInfoResponse                  │                  │
│  { id, username, email, role,                │                  │
│    first_name, last_name, wallet_address }   │                  │
│<─────────────────────────────────────────────│                  │
│          │                                    │                  │
└──────────┘                                    └──────────────────┘
```

**Note**: All user info extracted from JWT claims only. No database query performed.

---

## 5. Data Flow Diagrams

### 5.1 Complete Authentication Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    END-TO-END AUTHENTICATION FLOW                       │
└─────────────────────────────────────────────────────────────────────────┘

  User Browser/Mobile App
         │
         │ 1. POST /api/v1/auth/register
         │    { username, email, password }
         ▼
  ┌─────────────────┐
  │   IAM Service   │──2. Validate Password (Argon2 strength check)
  │   (REST API)    │──3. Hash Password (Argon2, blocking thread)
  └────────┬────────┘──4. INSERT INTO users (role='user', is_active=true)
           │
           │ 5. Return 201 Created { id, username, email }
           ▼
  ┌─────────────────┐
  │   User Email    │──6. Receive confirmation (external system)
  └────────┬────────┘
           │
           │ 7. Click verification link
           │    GET /api/v1/verify?token=verify_user@example.com
           ▼
  ┌─────────────────┐
  │   IAM Service   │──8. Extract email from token
  │   (REST API)    │──9. UPDATE users SET is_active=true, email_verified=true
  │                 │──10. Generate wallet_address (if null)
  │                 │──11. Generate JWT token (HS256, 24h)
  └────────┬────────┘
           │
           │ 12. Return { success, wallet_address, auth: { access_token } }
           ▼
  ┌─────────────────┐
  │   User Client   │──13. Store JWT token (localStorage/secure cookie)
  └────────┬────────┘
           │
           │ 14. POST /api/v1/auth/token (future logins)
           │     { username/email, password }
           ▼
  ┌─────────────────┐
  │   IAM Service   │──15. Query users WHERE (username OR email) = $1
  │   (REST API)    │──16. Verify password (Argon2/Bcrypt, blocking thread)
  │                 │──17. Generate JWT token
  └────────┬────────┘
           │
           │ 18. Return { access_token, expires_in, user }
           ▼
  ┌─────────────────┐
  │   User Client   │──19. Use JWT in subsequent requests
  │                 │    Authorization: Bearer <jwt>
  └─────────────────┘
```

### 5.2 Cross-Service Authorization Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                MICROSERVICE AUTHORIZATION FLOW                          │
└─────────────────────────────────────────────────────────────────────────┘

  Client Request
     │
     │ Authorization: Bearer <jwt>
     ▼
┌──────────────────┐
│  API Gateway     │──1. Extract JWT from Authorization header
│  (gridtokenx-api)│
└────────┬─────────┘
         │
         │ 2. gRPC: VerifyToken { token }
         ▼
┌──────────────────┐
│  IAM Service     │──3. Decode JWT (validate signature, expiration)
│  (gRPC 8091)     │──4. Return ClaimsResponse { valid, user_id, role }
└────────┬─────────┘
         │
         │ 5. If valid, continue
         │
         │ 6. gRPC: Authorize { token, required_permission }
         ▼
┌──────────────────┐
│  IAM Service     │──7. Decode JWT again (or cache)
│  (gRPC 8091)     │──8. Check role-based permissions
│                  │──9. Return AuthorizeResponse { authorized }
└────────┬─────────┘
         │
         │ 10. If authorized, continue
         │
         ▼
┌──────────────────┐
│  API Gateway     │──11. Process request
│  (gridtokenx-api)│──12. Return response to client
└──────────────────┘


Alternative: API Key Authentication (for AMI systems)
     │
     │ X-API-Key: <api_key>
     ▼
┌──────────────────┐
│  API Gateway     │──1. Extract API key from header
│  (gridtokenx-api)│
└────────┬─────────┘
         │
         │ 2. gRPC: VerifyApiKey { key }
         ▼
┌──────────────────┐
│  IAM Service     │──3. Hash key: SHA256(key + secret)
│  (gRPC 8091)     │──4. Lookup in api_keys table
│                  │──5. Update last_used_at
│                  │──6. Return ApiKeyResponse { valid, role }
└────────┬─────────┘
         │
         │ 7. If valid, continue
         ▼
┌──────────────────┐
│  API Gateway     │──8. Process request with API key role
│  (gridtokenx-api)│
└──────────────────┘
```

### 5.3 Password Hashing & Verification Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                   PASSWORD SECURITY FLOW                                │
└─────────────────────────────────────────────────────────────────────────┘

  REGISTRATION:
  ┌──────────┐         ┌──────────────┐         ┌──────────────┐
  │  Client  │         │ IAM Service  │         │  PostgreSQL  │
  │          │         │              │         │              │
  │  POST    │         │ 1. Validate  │         │              │
  │  { pwd } │────────>│    strength  │         │              │
  │          │         │    - length  │         │              │
  │          │         │    - upper   │         │              │
  │          │         │    - lower   │         │              │
  │          │         │    - digit   │         │              │
  │          │         │    - special │         │              │
  │          │         │              │         │              │
  │          │         │ 2. Hash      │         │              │
  │          │         │    [spawn_blocking]    │              │
  │          │         │    Argon2    │         │              │
  │          │         │    (default) │         │              │
  │          │         │              │         │              │
  │          │         │ 3. INSERT    │────────>│ 4. Store     │
  │          │         │              │         │    hash      │
  │          │         │              │<────────│              │
  │<─────────│         │              │         │              │
  │  201 OK  │         │              │         │              │
  └──────────┘         └──────────────┘         └──────────────┘


  LOGIN:
  ┌──────────┐         ┌──────────────┐         ┌──────────────┐
  │  Client  │         │ IAM Service  │         │  PostgreSQL  │
  │          │         │              │         │              │
  │  POST    │         │              │         │              │
  │  { pwd } │────────>│ 1. SELECT    │────────>│ 2. Return    │
  │          │         │    user row  │<────────│    hash      │
  │          │         │              │         │              │
  │          │         │ 3. Check     │         │              │
  │          │         │    prefix:   │         │              │
  │          │         │    $argon2 → │         │              │
  │          │         │      Argon2  │         │              │
  │          │         │    $2 →      │         │              │
  │          │         │      Bcrypt  │         │              │
  │          │         │    [spawn_blocking]    │              │
  │          │         │              │         │              │
  │          │         │ 4. Generate  │         │              │
  │          │         │    JWT       │         │              │
  │          │         │              │         │              │
  │<─────────│         │              │         │              │
  │  200 OK  │         │              │         │              │
  │  { jwt } │         │              │         │              │
  └──────────┘         └──────────────┘         └──────────────┘


  Hash Format Detection:
  ┌──────────────────────────────────────────────────────────────┐
  │  if hash.starts_with("$argon2")  →  Argon2 verification      │
  │  if hash.starts_with("$2")       →  Bcrypt verification       │
  │  else                            →  Error (unknown format)    │
  └──────────────────────────────────────────────────────────────┘
```

### 5.4 Database Query Flow for Trading Operations

```
┌─────────────────────────────────────────────────────────────────────────┐
│              TRADING-RELATED DATABASE RELATIONSHIPS                     │
└─────────────────────────────────────────────────────────────────────────┘

  users
    │
    │ 1:N
    ├───────────> trading_orders
    │               │
    │               │ N:1
    │               └────┐
    │                    ▼
    │              market_epochs
    │                    │
    │                    │ 1:N
    │                    ▼
    │              order_matches ────────┐
    │                    │               │
    │                    │ N:1           │
    │                    ▼               │
    │              settlements <─────────┘
    │                    │
    │                    │ N:1
    │                    ├───────────> users (buyer)
    │                    └───────────> users (seller)
    │
    │ 1:N
    ├───────────> meter_readings
    │               │
    │               │ Triggers
    │               ▼
    │         erc_certificates
    │               │
    │               │ 1:N
    │               ▼
    │         erc_certificate_transfers
    │
    │ 1:N
    ├───────────> blockchain_transactions
    │
    │ 1:N
    ├───────────> audit_logs
    │
    │ 1:N
    └───────────> user_activities


  Example: Complete Order Lifecycle

  1. User creates order
     ┌─────────┐      ┌──────────────┐      ┌──────────────┐
     │  User   │      │  API Gateway │      │  IAM Service │
     │         │      │              │      │              │
     │  POST   │      │ 1. Verify    │      │              │
     │  /orders│─────>│    JWT ──────│─────>│ Return valid │
     │         │      │              │      │              │
     │         │      │ 2. INSERT    │      │              │
     │         │      │    INTO      │      │              │
     │         │      │    trading_  │      │              │
     │         │      │    orders    │      │              │
     └─────────┘      └──────────────┘      └──────────────┘

  2. Order matched and settled
     ┌──────────────┐      ┌──────────────┐      ┌──────────────┐
     │  Trading Svc │      │  API Gateway │      │  Blockchain  │
     │              │      │              │      │              │
     │  Match buy   │      │              │      │              │
     │  + sell      │      │              │      │              │
     │  INSERT      │      │              │      │              │
     │  order_match │      │              │      │              │
     │              │      │ 3. Create    │      │              │
     │              │      │    settlement│      │              │
     │              │      │    + tx      │─────>│  Confirm on │
     │              │      │              │      │  Solana     │
     └──────────────┘      └──────────────┘      └──────────────┘
```

### 5.5 Observability Data Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                  OBSERVABILITY & MONITORING FLOW                        │
└─────────────────────────────────────────────────────────────────────────┘

  ┌─────────────────┐
  │  IAM Service    │
  │                 │
  │  Every HTTP     │
  │  Request        │
  └────────┬────────┘
           │
           ├─────────────────────────────────┐
           │                                 │
           ▼                                 ▼
  ┌──────────────────┐              ┌──────────────────┐
  │  OTel Tracing    │              │  Prometheus      │
  │  Middleware      │              │  Metrics         │
  │                  │              │  Middleware      │
  │  Auto-trace:     │              │                  │
  │  - method        │              │  Record:         │
  │  - route         │              │  - duration      │
  │  - status code   │              │  - auth attempts │
  │  - duration      │              │  - JWT ops       │
  │  - client IP     │              │  - API key ops   │
  └────────┬─────────┘              └────────┬─────────┘
           │                                 │
           │ OTLP Export                     │ HTTP /metrics
           │ (gRPC 4317)                     │ (scrape)
           ▼                                 ▼
  ┌──────────────────┐              ┌──────────────────┐
  │  SigNoz / OTel   │              │  Prometheus      │
  │  Collector       │              │  Server          │
  │                  │              │                  │
  │  - Traces        │              │  - Metrics       │
  │  - Logs          │              │  - Alerts        │
  │  - Metrics       │              │  - Dashboards    │
  └──────────────────┘              └──────────────────┘


  Trace Span Structure:
  ┌─────────────────────────────────────────────────────────────────────┐
  │  HTTP Request Span                                                  │
  ├─────────────────────────────────────────────────────────────────────┤
  │  Span Name: "HTTP {method} {route}"                                 │
  │  Attributes:                                                        │
  │    - http.method: "POST"                                            │
  │    - http.route: "/api/v1/auth/token"                               │
  │    - http.status_code: 200                                          │
  │    - http.duration_ms: 45.2                                         │
  │    - http.client_ip: "192.168.1.100"                                │
  │    - service.name: "gridtokenx-iam"                                 │
  │    - deployment.environment: "development"                          │
  │                                                                     │
  │  Child Spans (if any):                                              │
  │    ├─ database.query (SQLx)                                         │
  │    │   └─ Attributes: query, duration, rows_affected                │
  │    └─ password.verify (blocking thread)                             │
  │        └─ Attributes: algorithm, duration                            │
  └─────────────────────────────────────────────────────────────────────┘
```

---

## 6. API Endpoints

### 6.1 REST API Endpoints

| Method | Path | Handler | Auth | Description |
|--------|------|---------|------|-------------|
| `POST` | `/api/v1/auth/register` | `register` | None | Register new user |
| `POST` | `/api/v1/auth/token` | `login` | None | Authenticate and get JWT |
| `GET` | `/api/v1/verify` | `verify_email` | None | Verify email address |
| `GET` | `/metrics` | `get_metrics` | None | Prometheus metrics |

### 6.2 gRPC Endpoints (ConnectRPC)

| RPC Method | Request | Response | Auth | Description |
|------------|---------|----------|------|-------------|
| `VerifyToken` | `TokenRequest` | `ClaimsResponse` | None | Validate JWT and extract claims |
| `Authorize` | `AuthorizeRequest` | `AuthorizeResponse` | None | Check role-based permissions |
| `GetUserInfo` | `TokenRequest` | `UserInfoResponse` | None | Get user profile from JWT |
| `VerifyApiKey` | `ApiKeyRequest` | `ApiKeyResponse` | None | Validate API key |

### 6.3 Proto Definition

**File**: `proto/identity.proto`

```protobuf
syntax = "proto3";

package identity;

// Token verification
service IdentityService {
  rpc VerifyToken(TokenRequest) returns (ClaimsResponse);
  rpc Authorize(AuthorizeRequest) returns (AuthorizeResponse);
  rpc GetUserInfo(TokenRequest) returns (UserInfoResponse);
  rpc VerifyApiKey(ApiKeyRequest) returns (ApiKeyResponse);
}

message TokenRequest {
  string token = 1;
}

message ClaimsResponse {
  bool valid = 1;
  string user_id = 2;
  string username = 3;
  string role = 4;
  string error_message = 5;
}

message AuthorizeRequest {
  string token = 1;
  string required_permission = 2;
}

message AuthorizeResponse {
  bool authorized = 1;
  string error_message = 2;
}

message UserInfoResponse {
  string id = 1;
  string username = 2;
  string email = 3;
  string role = 4;
  string first_name = 5;
  string last_name = 6;
  string wallet_address = 7;
}

message ApiKeyRequest {
  string key = 1;
}

message ApiKeyResponse {
  bool valid = 1;
  string role = 2;
  string error_message = 3;
}
```

---

## 7. Configuration

### 7.1 Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | **Yes** | - | PostgreSQL connection string |
| `REDIS_URL` | **Yes** | - | Redis connection string (reserved) |
| `IAM_PORT` | No | `8081` | REST API port |
| `JWT_SECRET` | No | `supersecretjwtkey` | HMAC-SHA256 secret for JWT signing |
| `JWT_EXPIRATION` | No | `86400` | Token TTL in seconds (24 hours) |
| `ENCRYPTION_SECRET` | No | `supersecretencryptionkey` | General encryption key |
| `API_KEY_SECRET` | No | `supersecretapikey` | Salt for API key hashing |
| `LOG_LEVEL` | No | `info` | Tracing log level |
| `ENVIRONMENT` | No | `development` | Environment name |
| `TEST_MODE` | No | `false` | Test mode flag |
| `OTEL_ENABLED` | No | `true` | Enable OpenTelemetry tracing |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | No | `http://otel-collector:4317` | OTLP collector endpoint |
| `OTEL_SERVICE_NAME` | No | `gridtokenx-iam` | Service name for traces |

### 7.2 Configuration Loading

```
┌─────────────────────────────────────────────────────────────────────┐
│                    CONFIG LOADING FLOW                              │
└─────────────────────────────────────────────────────────────────────┘

  Environment Variables / .env file
         │
         ▼
  ┌──────────────────┐
  │  Config::load()  │
  │                  │
  │  1. Load .env    │
  │     (dotenvy)    │
  │                  │
  │  2. Read env     │
  │     vars         │
  │                  │
  │  3. Validate     │
  │     required:    │
  │     - DATABASE_URL│
  │     - REDIS_URL  │
  │                  │
  │  4. Apply        │
  │     defaults     │
  │     for optional │
  └────────┬─────────┘
           │
           │ Config struct (immutable)
           ▼
  ┌──────────────────┐
  │  Arc<Config>     │── Shared across all request handlers
  │                  │── Thread-safe, lock-free reads
  └──────────────────┘
```

### 7.3 Port Configuration

```
Default Port Allocation:
┌─────────────────────────────────────────────┐
│  IAM Service                                │
│  ├─ REST API:  8081 (IAM_PORT)             │
│  └─ gRPC:      8091 (IAM_PORT + 10)        │
└─────────────────────────────────────────────┘

Platform-Wide Port Map:
┌─────────────────────────────────────────────┐
│  API Gateway:    4000 (REST), 4001 (metrics)│
│  IAM Service:    8080/8081 (REST), 8090/8091│
│  Trading Svc:    8092 (REST), 8093 (gRPC)   │
│  Oracle Bridge:  4010                       │
│  Smart Meter:    8082                       │
│  PostgreSQL:     5434                       │
│  Redis:          6379                       │
│  InfluxDB:       8086                       │
│  Kafka:          9092                       │
│  Prometheus:     9090                       │
│  Grafana:        3001                       │
│  SigNoz:         3030                       │
│  Solana RPC:     8899                       │
└─────────────────────────────────────────────┘
```

---

## 8. Observability

### 8.1 OpenTelemetry Tracing

**Initialization Flow**:
```
┌─────────────────────────────────────────────────────────────────────┐
│                  OTEL INITIALIZATION                                │
└─────────────────────────────────────────────────────────────────────┘

  Service Startup
         │
         ▼
  ┌──────────────────────┐
  │  InitTracer()        │
  │                      │
  │  IF OTEL_ENABLED=true│
  │    ├─ Create OTLP    │
  │    │   exporter      │
  │    │   (gRPC 4317)   │
  │    │                 │
  │    ├─ Build tracer   │
  │    │   provider      │
  │    │                 │
  │    ├─ Set global     │
  │    │   tracer        │
  │    │                 │
  │    └─ Return success │
  │                      │
  │  ELSE                │
  │    └─ Return early   │
  │        (no-op)       │
  └──────────────────────┘
```

**Auto-Traced HTTP Attributes**:
- `http.method` - Request method (GET, POST, etc.)
- `http.route` - Matched route pattern
- `http.status_code` - Response status code
- `http.duration_ms` - Request duration in milliseconds
- `http.client_ip` - Client IP address
- `service.name` - Service identifier
- `deployment.environment` - Environment (development, production)

### 8.2 Prometheus Metrics

**Metrics Endpoint**: `GET /metrics`

**Custom Metrics**:

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `http_request_duration_seconds` | Histogram | `method`, `route`, `status` | HTTP request duration |
| `auth_login_attempts_total` | Counter | `result` (success/failure) | Total login attempts |
| `jwt_operations_total` | Counter | `operation`, `result` | JWT encode/decode operations |
| `api_key_operations_total` | Counter | `operation`, `result` | API key verification operations |

**Histogram Buckets** (for request duration):
```
0.001s, 0.005s, 0.01s, 0.025s, 0.05s, 0.1s, 0.25s, 0.5s, 1.0s, 2.5s, 5.0s, 10.0s
```

### 8.3 Error Codes

**Format**: `{CATEGORY}_{NUMERIC_CODE}`

| Error Code | Category | Description |
|------------|----------|-------------|
| `AUTH_1001` | Authentication | Invalid credentials |
| `AUTH_1002` | Authentication | Token expired |
| `AUTH_1003` | Authentication | Account not active |
| `AUTH_1004` | Authentication | Invalid token signature |
| `AUTH_1005` | Authentication | Password verification failed |
| `AUTH_1006` | Authentication | Missing required claims |
| `AUTH_1007` | Authentication | Invalid API key |
| `VAL_3001` | Validation | Input validation error |
| `VAL_3002` | Validation | Password strength error |
| `DB_7001` | Database | Database operation failed |
| `DB_7002` | Database | Record not found |
| `DB_7003` | Database | Unique constraint violation |
| `INTERNAL_9001` | Internal | Internal server error |

### 8.4 Logging

**Log Format** (development):
```json
{
  "timestamp": "2024-11-18T12:00:00.000000Z",
  "level": "INFO",
  "message": "User logged in",
  "fields": {
    "user_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "username": "admin",
    "ip_address": "127.0.0.1"
  },
  "target": "gridtokenx_iam_service::api::handlers::auth",
  "span": {
    "name": "HTTP POST /api/v1/auth/token",
    "trace_id": "abc123..."
  }
}
```

**Log Levels**:
- `ERROR` - System failures, unrecoverable errors
- `WARN` - Suspicious activity, degraded functionality
- `INFO` - Important business events (login, registration)
- `DEBUG` - Detailed diagnostic information
- `TRACE` - Very fine-grained debugging

---

## Appendix A: Security Considerations

### Password Security
- **Hashing Algorithm**: Argon2 (memory-hard, resistant to GPU attacks)
- **Legacy Support**: Bcrypt for backward compatibility
- **Strength Requirements**: 8-128 chars, mixed case, digit, special char
- **Verification**: Runs in blocking thread to avoid async runtime starvation

### JWT Security
- **Algorithm**: HMAC-SHA256 (symmetric)
- **Secret**: Minimum 32 characters recommended
- **Expiration**: 24 hours (configurable)
- **Issuer Validation**: Prevents token forgery from other services
- **Required Claims**: `sub`, `username`, `role`, `exp`, `iss`

### API Key Security
- **Storage**: Only SHA256 hash stored (one-way)
- **Salt**: API_KEY_SECRET appended before hashing
- **Tracking**: `last_used_at` updated on each verification
- **Permissions**: Fine-grained PostgreSQL array storage

### Database Security
- **Connection Pooling**: Max 5 connections (conservative)
- **Parameterized Queries**: SQLx prevents SQL injection
- **Compile-Time Checking**: SQLx validates queries at build time
- **Migrations**: Version-controlled, applied in order

### Known Limitations
- **Email Verification**: Simplified token format (`verify_{email}`) for E2E testing only
- **Redis**: Configured but not actively used (reserved for future session caching)
- **No Rate Limiting**: Not implemented at IAM service level
- **No Brute Force Protection**: No account lockout or CAPTCHA
- **Mock Wallets**: Auto-generated during email verification if not provided

---

## Appendix B: Build & Deployment

### Build Process

```
┌─────────────────────────────────────────────────────────────────────┐
│                      BUILD PIPELINE                                 │
└─────────────────────────────────────────────────────────────────────┘

  cargo build
       │
       ▼
  ┌──────────────────┐
  │  build.rs        │──1. Compile identity.proto
  │                  │──2. Generate Rust code (buffa)
  │                  │──3. Output to $OUT_DIR/_identity_include.rs
  └────────┬─────────┘
           │
           ▼
  ┌──────────────────┐
  │  cargo check     │──4. Compile Rust source
  │                  │──5. SQLx query validation (if DB available)
  └────────┬─────────┘
           │
           ▼
  ┌──────────────────┐
  │  cargo test      │──6. Run unit tests
  │                  │──7. Run integration tests
  └──────────────────┘
```

### Docker Deployment

```dockerfile
FROM lukemathwalker/cargo-chef:latest AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin gridtokenx-iam-service

FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/gridtokenx-iam-service /usr/local/bin/
COPY --from=builder /app/migrations /app/migrations
EXPOSE 8081 8091
CMD ["gridtokenx-iam-service"]
```

---

## Appendix C: Migration History

| Migration | Description |
|-----------|-------------|
| `20241101000001` | Initial schema (users, orders, settlements) |
| `20241102000002` | Add email verification columns |
| `20241114000003` | Schema fixes (constraints, indexes) |
| `20241118000004` | Add missing tables (meter_readings, certificates) |
| `20241118000005` | Add mint transaction signature columns |
| `20241118000006` | Fix schema mismatches |
| `20241118000007` | Add final columns |
| `20241118000008` | Add issuer wallet columns |
| `20241118100001` | Convert status columns to PostgreSQL enums |
| `20241118100002` | Add blockchain transaction signature |
| `20241118120001` | Simplify user roles (user/admin only) |

---

**Document Version**: 1.0  
**Last Updated**: 2024-11-18  
**Maintainer**: GridTokenX Engineering Team
