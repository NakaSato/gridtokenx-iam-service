//! Transactional-outbox repository (`iam_outbox_events`).
//!
//! Uses the **runtime** SQLx query API (not the compile-time `query!` macros the
//! rest of this crate uses) so it builds without the new table existing at
//! compile time — the table ships in migration `20260615120000_*`.

use async_trait::async_trait;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use iam_core::domain::identity::{Event, OutboxRecord};
use iam_core::error::{ApiError, Result};
use iam_core::traits::OutboxRepositoryTrait;

/// After this many failed delivery attempts a row is quarantined as `FAILED`
/// instead of being retried forever (poison-pill guard). Bounded so a single
/// undeliverable event can never wedge the worker's batch indefinitely.
const MAX_ATTEMPTS: i32 = 10;

/// Postgres-backed transactional outbox for IAM domain events.
pub struct OutboxRepository {
    pool: PgPool,
}

impl OutboxRepository {
    /// Creates a new repository over the shared connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct OutboxRow {
    id: Uuid,
    event_type: String,
    payload: serde_json::Value,
}

#[async_trait]
impl OutboxRepositoryTrait for OutboxRepository {
    async fn enqueue(&self, event: &Event) -> Result<()> {
        let payload = serde_json::to_value(event)
            .map_err(|e| ApiError::Internal(format!("Failed to serialize outbox event: {e}")))?;

        sqlx::query("INSERT INTO iam_outbox_events (event_type, payload) VALUES ($1, $2)")
            .bind(&event.event_type)
            .bind(payload)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn fetch_pending(&self, limit: i64) -> Result<Vec<OutboxRecord>> {
        let rows = sqlx::query_as::<_, OutboxRow>(
            "SELECT id, event_type, payload FROM iam_outbox_events \
             WHERE status = 'PENDING' ORDER BY created_at ASC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| OutboxRecord {
                id: r.id,
                event_type: r.event_type,
                payload: r.payload,
            })
            .collect())
    }

    async fn mark_processed(&self, id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE iam_outbox_events SET status = 'PROCESSED', last_attempt_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn mark_failed(&self, id: Uuid) -> Result<()> {
        // Keep the row PENDING (so the next tick retries) until it exhausts its
        // attempt budget, then flip to FAILED so a poison row stops being retried.
        sqlx::query(
            "UPDATE iam_outbox_events \
             SET attempts = attempts + 1, \
                 last_attempt_at = NOW(), \
                 status = CASE WHEN attempts + 1 >= $2 THEN 'FAILED' ELSE 'PENDING' END \
             WHERE id = $1",
        )
        .bind(id)
        .bind(MAX_ATTEMPTS)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
