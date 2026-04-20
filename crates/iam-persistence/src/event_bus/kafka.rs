use anyhow::Result;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::time::Duration;
use tracing::{error, info};
use crate::event_bus::Event;

/// Kafka topic configuration for IAM events
#[derive(Debug, Clone)]
pub struct KafkaTopics {
    pub user_events: String,
    pub kyc_events: String,
    pub audit_events: String,
}

impl KafkaTopics {
    pub fn with_prefix(prefix: &str) -> Self {
        Self {
            user_events: format!("{}.user.events", prefix),
            kyc_events: format!("{}.kyc.events", prefix),
            audit_events: format!("{}.audit.events", prefix),
        }
    }
}

impl Default for KafkaTopics {
    fn default() -> Self {
        Self::with_prefix("iam")
    }
}

/// Kafka-backed event bus for IAM high-throughput event sourcing.
/// Part of the Hybrid Messaging Architecture transition.
#[derive(Clone)]
pub struct KafkaEventBus {
    producer: FutureProducer,
    pub topics: KafkaTopics,
}

impl KafkaEventBus {
    pub async fn new(bootstrap_servers: &str, topic_prefix: Option<&str>) -> Result<Self> {
        info!("Initializing IAM Kafka Producer (brokers: {})", bootstrap_servers);

        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", bootstrap_servers)
            .set("message.timeout.ms", "5000")
            .set("acks", "all") // High durability for IAM/identity events
            .set("retries", "3")
            .create()
            .map_err(|e| anyhow::anyhow!("Failed to create Kafka producer: {}", e))?;

        let topics = match topic_prefix {
            Some(prefix) => KafkaTopics::with_prefix(prefix),
            None => KafkaTopics::default(),
        };

        Ok(Self { producer, topics })
    }

    /// Publish an IAM domain event to Kafka.
    pub async fn publish(&self, event: &Event) -> Result<String> {
        self.publish_raw(event).await
    }

    /// Publish a batch of IAM domain events to Kafka.
    pub async fn publish_batch(&self, events: &[Event]) -> Result<()> {
        for event in events {
            self.publish_raw(event).await?;
        }
        Ok(())
    }

    async fn publish_raw(&self, event: &Event) -> Result<String> {
        let payload = serde_json::to_vec(event)?;
        
        // Route events based on type
        let topic = match event.event_type.as_str() {
            "UserRegistered" | "UserLoggedIn" | "EmailVerified" | "PasswordResetRequested" | "UserOnboarded" | "UserWalletLinked" => &self.topics.user_events,
            "KycSubmitted" | "KycVerified" | "KycRejected" => &self.topics.kyc_events,
            _ => &self.topics.audit_events,
        };

        // Partition by user_id if available in payload, otherwise auto-partition
        let user_id = event.data.as_ref()
            .and_then(|d| d.get("user_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let record = FutureRecord::to(topic)
            .key(user_id)
            .payload(&payload);

        match self.producer.send(record, Duration::from_secs(5)).await {
            Ok((partition, offset)) => {
                let id = format!("{}:{}:{}", topic, partition, offset);
                info!("Event {} published to Kafka topic {} (p:{}, o:{})", event.event_type, topic, partition, offset);
                Ok(id)
            }
            Err((e, _)) => {
                error!("Failed to publish IAM event {} to Kafka: {}", event.event_type, e);
                Err(anyhow::anyhow!("Kafka produce error: {}", e))
            }
        }
    }
}
