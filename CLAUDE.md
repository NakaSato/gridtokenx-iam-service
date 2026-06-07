# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> Scope: the **IAM Service** submodule. The superproject `CLAUDE.md` (one level up) covers platform-wide rules, the knowledge-graph MCP tooling, and `just` recipes — read it too. This file documents what is specific to IAM and not derivable from the source tree.

## What this service is

Identity & Access Management for GridTokenX: user lifecycle, auth (email/password + Solana wallets), JWT/API-key issuance, RBAC, and on-chain user registration via the Solana Registry program. **Modular monolith** — one Cargo workspace, 6 crates, no root `Cargo.toml` above it. Edition **2024**.

## Build, run, test

```bash
cargo check                       # fast feedback across workspace
cargo test                        # all crates
cargo test -p iam-logic           # one crate
cargo test -p iam-persistence     # integration tests — needs live Postgres
cargo build -p gridtokenx-iam-service   # the binary crate (note name)

./start.sh                        # build if needed, run with dev env, poll /health on :4010
```

- The binary crate lives at `bin/iam-service` but its **package and binary name is `gridtokenx-iam-service`** — use that with `-p` / `cargo run`. Docs/diagrams that say `iam-server` are stale; the entry point is `bin/iam-service/src/{main.rs,startup.rs}`.
- `cargo` requires `DATABASE_URL` (or `IAM_DATABASE_URL`) reachable at compile time — SQLx queries are **compile-time checked** (`sqlx::query_as!`). If you change a query, run `cargo sqlx prepare` so offline builds work.
- Workspace lints are strict: `unsafe_code = "deny"`, `clippy::unwrap_used = "deny"`, `clippy::pedantic = "warn"`, `missing_docs = "warn"`. `.unwrap()` will fail the build outside `#[cfg(test)]` — use `?`/`.context()`/`.expect("…")`.

## Layering (dependency direction)

```
bin/iam-service ──► iam-api ──► iam-logic ──► iam-core
        └──────────► iam-persistence ──► iam-core
iam-api ──► iam-protocol (ConnectRPC contract)
```

Never reverse. `iam-core` is the zero-I/O heart: domain models (`domain/identity/`), **trait definitions** (`traits.rs`), error types (`error/`), and `Config`.

| Crate | Role |
|-------|------|
| `iam-core` | Domain models, traits (the DI contracts), errors, `Config::from_env`. Has a `mocks` feature gating `mockall` automocks. |
| `iam-protocol` | `proto/identity.proto` → codegen via `buffa-build`/`connectrpc-build` (`build.rs`). |
| `iam-persistence` | Trait *implementations*: SQLx repos (`repository/{user,wallet,api_key}.rs`), Redis `cache.rs`, `event_bus/` (Redis + Kafka). |
| `iam-logic` | Business services: `AuthService`, `JwtService`, `ApiKeyService`, `BlockchainProvider`, `password.rs`. Pulls `iam-core` **with `mocks` feature** for tests. |
| `iam-api` | Axum REST handlers (`handlers/`), ConnectRPC impl (`identity_grpc.rs`), middleware (`request_id`, `metrics`, `rate_limit`). |
| `bin/iam-service` | Composition root — `startup.rs` builds the pool, runs migrations, wires every trait `Arc<dyn …>`, starts REST + gRPC. |

### How DI actually wires (startup.rs)

`startup::run` is the single place dependencies are constructed: build `PgPool` → `sqlx::migrate!("../../migrations")` → construct concrete repos as `Arc<dyn UserRepositoryTrait>` etc. → build `CacheService`/`EventBus`/`JwtService`/`ApiKeyService` → build `BlockchainService` from `gridtokenx-blockchain-core` (talks to Chain Bridge, **never Solana RPC directly**) → assemble `AuthService` (it owns all the trait objects) → mount routes → run REST and gRPC servers concurrently via `tokio::join!` with a shared `CancellationToken` for graceful shutdown.

`AuthService` is the Axum `State` and the gRPC service's dependency — most request paths flow through it.

## Async/sync split & CPU safety

"Sync Core, Async Edges." Password hashing/verification is CPU-bound and MUST run on `spawn_blocking` (already done in `AuthService`) — never on Tokio worker threads. `AUTH_CPU_SEMAPHORE_LIMIT` bounds concurrent hashing. Cross-crate traits (e.g. `BlockchainTrait`) use manual `BoxFuture<'static, …>` instead of `#[async_trait]` to dodge `dyn`-compat/lifetime errors (E0195) — match that pattern when adding shared traits.

## Surfaces

- **REST** on `IAM_PORT` (4010): `/api/v1/auth/{register,login,verify,forgot-password,reset-password}` (rate-limited), `/api/v1/users/me[/wallets…]`, `/api/v1/system/config`, `/health`, `/health/ready` (checks Postgres + Redis), `/health/live`, `/metrics` (Prometheus).
- **gRPC/ConnectRPC** on `IAM_GRPC_PORT` (4020, defaults to `IAM_PORT + 10`): `IdentityService` — `VerifyToken`, `Authorize`, `GetUserInfo`. This is how Trading/gateways verify identities. Contract: `crates/iam-protocol/proto/identity.proto`.
- **Observability** init via the shared `gridtokenx-telemetry` workspace crate, wrapped by the local `telemetry` module: `main.rs` calls `telemetry::init_telemetry("gridtokenx-iam")` (tracing/OTel) before `startup::run`. Don't hand-roll a `tracing-subscriber` here — extend the shared crate.

## Migrations — read before touching

`migrations/` holds **90+ migrations covering the entire platform schema**, not just IAM tables — trading orders, meters/telemetry, settlements, VPP, carbon credits, AMM, outbox, etc. IAM owns the migration runner but the DB is shared. Implications:
- Migrations run automatically on service start via `sqlx::migrate!` in `startup.rs`.
- Naming is timestamp-prefixed (`YYYYMMDDHHMMSS_*` for recent ones). Use `just migrate-new name:X` from the superproject; add migrations, never edit applied ones.
- A new schema change for *any* platform table tends to land here.

## Config (env vars)

`Config::from_env` (`iam-core/src/config.rs`) is the source of truth. Required: `IAM_DATABASE_URL` **or** `DATABASE_URL` (no default — startup fails without it). Notable: `IAM_PORT`/`IAM_GRPC_PORT`, `REDIS_URL`, `JWT_SECRET`/`JWT_EXPIRATION`, `ENCRYPTION_SECRET` (32+ chars, wallet AES-256-GCM), `API_KEY_SECRET`, `MASTER_SECRET`, `CHAIN_BRIDGE_URL` (+ `CHAIN_BRIDGE_INSECURE` for dev), `SOLANA_*_PROGRAM_ID`, `AUTH_CPU_SEMAPHORE_LIMIT`, `TOKIO_WORKER_THREADS`. Messaging (`KAFKA_CMD_BROKERS`, `RABBITMQ_URL`) is optional — absent = disabled. `.env.example` and `start.sh` disagree on the dev DB port (5434 vs 7001) and on `IAM_DATABASE_URL` vs `DATABASE_URL`; `start.sh` is the working local recipe.

## Testing conventions

- `iam-logic` unit-tests `AuthService` against `mockall` mocks — `iam-core`'s `mocks` feature exports `Mock*Trait` types (kept out of the production binary). Build mocks via that feature, never hand-roll fakes.
- `iam-persistence` uses real Postgres (`sqlx::test`) — integration, needs a DB up. Writes are designed idempotent for safe retry.
- Inline `#[cfg(test)]` modules (`*_tests.rs` files) per crate; shell-based end-to-end scripts live in `tests/*.sh` (`api_test.sh`, `auth_flow_test.sh`, etc.) with `tests/api-ref.md` as the endpoint reference.
