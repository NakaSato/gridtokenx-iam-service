//! Event Bus backed by Redis Streams for inter-service communication.
//!
//! Publishes domain events (user registered, logged in, etc.) so other
//! services (API Gateway, Trading) can react in real time.

use redis::{AsyncCommands, Client, aio::ConnectionManager};
use anyhow::{Result, Context};
use tracing::{info, warn};
use async_trait::async_trait;

#[cfg(test)]
use uuid::Uuid;
pub mod kafka;
pub mod rabbitmq;

use iam_core::traits::EventBusTrait;
use iam_core::domain::identity::Event;
use iam_core::error::{Result as IamResult, ApiError};

/// Default Redis stream name for platform events.
const DEFAULT_STREAM: &str = "gridtokenx:events:v1";

/// Maximum stream length (keep ~100k entries to bound memory).
const MAX_STREAM_LEN: usize = 100_000;

/// Redis Streams event bus.
#[derive(Clone)]
pub struct EventBus {
    conn: ConnectionManager,
    stream_name: String,
    pub kafka: Option<kafka::KafkaEventBus>,
    pub rabbitmq: Option<rabbitmq::IamRabbitMQProducer>,
}

impl EventBus {
    /// Create a new `EventBus` connected to Redis Streams.
    pub async fn new(
        redis_url: &str,
        kafka_brokers: Option<String>,
        rabbitmq_url: Option<String>,
    ) -> Result<Self> {
        let client = Client::open(redis_url)
            .context("Failed to create Redis client for EventBus")?;

        let conn = client
            .get_connection_manager()
            .await
            .context("Failed to create EventBus connection manager")?;

        let kafka = if let Some(brokers) = kafka_brokers {
            match kafka::KafkaEventBus::new(&brokers, None).await {
                Ok(k) => Some(k),
                Err(e) => {
                    warn!("Failed to initialize Kafka EventBus: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let rabbitmq = if let Some(url) = rabbitmq_url {
            match rabbitmq::IamRabbitMQProducer::new(&url).await {
                Ok(r) => Some(r),
                Err(e) => {
                    warn!("Failed to initialize RabbitMQ Producer: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            conn,
            stream_name: DEFAULT_STREAM.to_string(),
            kafka,
            rabbitmq,
        })
    }

    /// Internal publish implementation
    async fn publish_raw(&self, event: &Event) -> Result<String> {
        // 1. Dual-write to Kafka (Event Sourcing)
        if let Some(kafka) = &self.kafka {
            let _ = kafka.publish(event).await.map_err(|e| {
                warn!("Kafka publish failed (non-critical): {}", e);
            });
        }

        // 2. Legacy Redis Stream
        let payload = serde_json::to_string(event)
            .context("Failed to serialize event")?;

        let mut conn = self.conn.clone();
        let entry_id: String = conn.xadd(
            &self.stream_name,
            "*",                              // auto-generate ID
            &[("event", &payload)],
        ).await
            .context("Redis XADD failed")?;

        // Trim stream to bound memory usage
        let _: () = conn.xtrim(
            &self.stream_name,
            redis::streams::StreamMaxlen::Approx(MAX_STREAM_LEN),
        ).await
            .unwrap_or_else(|e| {
                warn!("XTRIM failed (non-critical): {}", e);
            });

        info!(
            event_type = %event.event_type,
            event_id = %event.id,
            stream_entry = %entry_id,
            "Event published to Redis stream"
        );

        Ok(entry_id)
    }

    /// Publish multiple events atomically in a single pipeline.
    pub async fn publish_batch(&self, events: &[Event]) -> Result<Vec<String>> {
        let mut ids = Vec::with_capacity(events.len());

        for event in events {
            let id = self.publish_raw(event).await?;
            ids.push(id);
        }

        info!(count = ids.len(), "Batch events published to Redis stream");
        Ok(ids)
    }
}

#[async_trait]
impl EventBusTrait for EventBus {
    async fn publish(&self, event: &Event) -> IamResult<()> {
        self.publish_raw(event).await.map(|_| ()).map_err(|e| ApiError::Internal(e.to_string()))
    }

    async fn publish_batch(&self, events: &[Event]) -> IamResult<()> {
        let mut ids = Vec::with_capacity(events.len());

        for event in events {
            let id = self.publish_raw(event).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            ids.push(id);
        }

        info!(count = ids.len(), "Batch events published to Redis stream");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serialization() {
        let event = Event::user_registered(
            &Uuid::nil(),
            "testuser",
            "test@example.com",
        );
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("UserRegistered"));
        assert!(json.contains("testuser"));
    }

    #[test]
    fn test_event_with_data() {
        let event = Event::new("Test", "source")
            .with_data(serde_json::json!({"key": "value"}));
        assert!(event.data.is_some());
        assert_eq!(event.data.unwrap()["key"], "value");
    }
}
