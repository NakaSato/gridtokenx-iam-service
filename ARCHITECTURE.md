# ARCHITECTURE — gridtokenx-iam-service

This service is the **Identity and Access Management (IAM)** backbone of the GridTokenX platform. It is structured as a **Modular Monolith** Cargo workspace following the "Sync Core, Async Edges" architectural principle.

## 🏗️ Layered Architecture

The workspace enforces strict downward dependency flow. Higher-level adapters never leak into the domain core.

```mermaid
graph TD
    subgraph "Adapters (Top)"
        Server[bin/iam-service]
        API[crates/iam-api]
    end

    subgraph "Domain Logic"
        Logic[crates/iam-logic]
    end

    subgraph "Infrastructure & Persistence"
        Persistence[crates/iam-persistence]
    end

    subgraph "Contracts & Primitives"
        Protocol[crates/iam-protocol]
        Core[crates/iam-core]
    end

    Server --> API
    Server --> Logic
    Server --> Persistence
    API --> Logic
    API --> Protocol
    Logic --> Core
    Persistence --> Core
    API --> Core
```

## 📦 Crate Inventory

| Crate | Layer | Responsibility |
|-------|-------|----------------|
| **[iam-service](file:///bin/iam-service)** | Adapter | Entry point, configuration loading, and **Dependency Injection** (orchestration of all layers). |
| **[iam-api](file:///crates/iam-api)** | Adapter | ConnectRPC (gRPC) and REST (Axum) handlers. High-concurrency async edge. |
| **[iam-logic](file:///crates/iam-logic)** | Domain | Core business rules: Auth orchestration, OTP verification, and blockchain provider logic. |
| **[iam-persistence](file:///crates/iam-persistence)** | Infrastructure | Implementations for SQLx (PostgreSQL), Redis (Cache/Events), and SMTP (Email). |
| **[iam-protocol](file:///crates/iam-protocol)** | Contract | Buffa-generated ConnectRPC definitions and domain-agnostic messaging types. |
| **[iam-core](file:///crates/iam-core)** | Primitives | Domain models, **Trait definitions**, and shared error types. Zero-dependency heart. |

## 🛠️ Key Design Decisions

### 1. Unified Identity Model
The IAM service manages a unified identity that bridges Web2 (email/password) and Web3 (Solana wallets). The `User` entity is the primary anchor for all platform interactions.

### 2. Trait-Based Dependency Injection (DI)
The Logic layer communicates with Infrastructure ONLY through traits defined in `iam-core`.
- **Decoupling**: Business logic remains 100% agnostic of the underlying database (SQLx) or message broker.
- **Mockability**: Every infrastructure dependency can be swapped with a mock during unit testing.

### 3. Sync Core, Async Edges
- **Sync Core**: Domain models and simple logic are synchronous and deterministic.
- **Async Edges**: I/O-bound operations (API handlers, Database workers) use Tokio's async runtime.
- **Trait Resolution**: Async traits are used sparingly to avoid complex lifetime issues, favoring manual `BoxFuture` for performance-critical or cross-crate shared interfaces.

## 🧪 Testing & Quality

The service uses a native Rust testing strategy to ensure rapid feedback cycles:

### Unit Testing (Mock-based)
- **Crate Isolation**: `iam-logic` is tested using `mockall`.
- **Mocks Feature**: `iam-core` provides a `mocks` feature that exports automocked traits (e.g., `MockUserRepositoryTrait`) to other packages without bloating the production binary.
- **Deterministic Logic**: We aim for 100% unit test coverage on the `AuthService` logic.

### Persistence Testing (Integration)
- **Database Tests**: `iam-persistence` uses `sqlx::test` to run integration tests against a real PostgreSQL instance (via Docker).
- **Idempotency**: All database writes are designed to be idempotent to allow safe retries in the event of partial failures.

## ⚙️ Advanced Technical Patterns

### manual `BoxFuture` for Traits
To resolve complex `async_trait` lifetime issues (`E0195`) when sharing traits across crate boundaries (e.g., `BlockchainTrait`), we use the manual `BoxFuture` pattern:

```rust
pub trait BlockchainTrait: Send + Sync {
    fn register_user_on_chain(
        &self,
        authority: Pubkey,
        // ...
    ) -> BoxFuture<'static, Result<Signature>>;
}
```
This ensures guaranteed `dyn` compatibility and stable compilation across the modular monolith workspace.

### Safe Password Handling
Passwords are never stored in plain text or logged. We use **Argon2id** (via `bcrypt` or specialized crates) for state-of-the-art hashing resistance.

## ⚡ Concurrency & CPU Safety

To maintain high throughput and low latency, the service rigorously separates I/O-bound tasks from CPU-bound tasks to avoid **Tokio Worker Starvation**.

### 1. Offloading CPU-Bound Work
Heavy compute tasks (e.g., Password hashing/verification, complex cryptography) MUST NOT run directly on Tokio threads.
- **Pattern**: Use `tokio::task::spawn_blocking` for standard blocking/CPU tasks.
- **Current usage**: Applied in `AuthService` for all password operations.

### 2. Guidance for Rayon Integration
If parallel iterators (`rayon`) are introduced in the future for batch processing:
- **Decoupled Pools**: Rayon and Tokio thread pools must be configured with explicit thread budgets to avoid CFS throttling in containerized environments.
- **Budgeting**: Set `num_threads` based on CPU **requests** (baseline), not limits (burst).
- **Bridge via Oneshot**: Dispatch work to Rayon using `tokio::sync::oneshot` to bridge the async/sync boundary without blocking the executor.
- **Thresholds**: Only use parallel processing for datasets exceeding a measured threshold (e.g., >100 items) to avoid coordination overhead.
