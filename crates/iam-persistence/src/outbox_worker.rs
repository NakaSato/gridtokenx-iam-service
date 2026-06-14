//! Background worker that drains the IAM transactional outbox to Kafka.
//!
//! Polls `iam_outbox_events` for `PENDING` rows, delivers each to Kafka
//! (awaiting the broker ack), and marks the row `PROCESSED` on success or
//! records a failed attempt for retry on failure. This is what turns the
//! previously fire-and-forget Kafka dual-write into an at-least-once delivery:
//! a broker outage now retries instead of silently dropping events.

use std::sync::Arc;
use std::time::Duration;

use tokio::time::sleep;
use tracing::{error, info, warn};

use iam_core::domain::identity::Event;
use iam_core::traits::OutboxRepositoryTrait;

use crate::event_bus::kafka::KafkaEventBus;

/// Drains pending outbox rows and relays them to Kafka.
pub struct OutboxWorker {
    outbox: Arc<dyn OutboxRepositoryTrait>,
    kafka: KafkaEventBus,
    batch_size: i64,
    idle_interval: Duration,
}

impl OutboxWorker {
    /// Creates a worker over the given outbox and Kafka producer.
    pub fn new(outbox: Arc<dyn OutboxRepositoryTrait>, kafka: KafkaEventBus) -> Self {
        Self {
            outbox,
            kafka,
            batch_size: 100,
            idle_interval: Duration::from_millis(500),
        }
    }

    /// Runs the drain loop forever. Sleeps `idle_interval` when the outbox is
    /// empty; backs off longer when the store itself errors. Cancel by dropping
    /// the spawned task (e.g. via `tokio::select!` on a `CancellationToken`).
    pub async fn run(self) {
        info!("Starting IAM OutboxWorker (durable Kafka delivery)");
        loop {
            if self.tick().await == 0 {
                sleep(self.idle_interval).await;
            }
        }
    }

    /// Processes one batch. Returns the number of rows fetched. Never panics —
    /// every per-row failure is logged and the row left for retry so one bad
    /// event cannot kill the loop.
    async fn tick(&self) -> usize {
        let pending = match self.outbox.fetch_pending(self.batch_size).await {
            Ok(rows) => rows,
            Err(e) => {
                error!("OutboxWorker fetch_pending failed: {}", e);
                // Treat as idle so the loop backs off rather than spinning.
                sleep(Duration::from_secs(5)).await;
                return 0;
            }
        };

        let count = pending.len();
        for record in pending {
            let event: Event = match serde_json::from_value(record.payload) {
                Ok(ev) => ev,
                Err(e) => {
                    // Undeserializable payload will never deliver — quarantine it
                    // so it stops blocking the batch.
                    warn!(
                        "OutboxWorker: row {} has an undeserializable payload, quarantining: {}",
                        record.id, e
                    );
                    if let Err(e2) = self.outbox.mark_failed(record.id).await {
                        error!("OutboxWorker: mark_failed({}) failed: {}", record.id, e2);
                    }
                    continue;
                }
            };

            match self.kafka.publish(&event).await {
                Ok(_) => {
                    if let Err(e) = self.outbox.mark_processed(record.id).await {
                        // Delivered but not marked: will redeliver next tick.
                        // Downstream consumers must dedup (events carry a stable id).
                        error!("OutboxWorker: mark_processed({}) failed: {}", record.id, e);
                    }
                }
                Err(e) => {
                    warn!(
                        "OutboxWorker: Kafka delivery failed for {} (row {}), will retry: {}",
                        event.event_type, record.id, e
                    );
                    if let Err(e2) = self.outbox.mark_failed(record.id).await {
                        error!("OutboxWorker: mark_failed({}) failed: {}", record.id, e2);
                    }
                }
            }
        }

        count
    }
}
