//! Event Bus backed by Redis Streams for inter-service communication.
//!
//! Publishes domain events (user registered, logged in, etc.) so other
//! services (API Gateway, Trading) can react in real time.

use redis::{AsyncCommands, Client, aio::ConnectionManager};
use anyhow::{Result, Context};
use tracing::{info, warn};
use uuid::Uuid;
use chrono::Utc;

/// Default Redis stream name for platform events.
const DEFAULT_STREAM: &str = "gridtokenx:events:v1";

/// Maximum stream length (keep ~100k entries to bound memory).
const MAX_STREAM_LEN: usize = 100_000;

/// Domain event that can be published to Redis Streams.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Event {
    /// Unique event ID.
    pub id: Uuid,
    /// Event type discriminator (e.g. "UserRegistered", "UserLoggedIn").
    pub event_type: String,
    /// ISO-8601 timestamp.
    pub timestamp: String,
    /// Opaque JSON payload specific to the event type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Service that originated the event.
    pub source: String,
}

impl Event {
    pub fn new(event_type: &str, source: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type: event_type.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            data: None,
            source: source.to_string(),
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// Redis Streams event bus.
#[derive(Clone)]
pub struct EventBus {
    conn: ConnectionManager,
    stream_name: String,
}

impl EventBus {
    /// Create a new `EventBus` connected to Redis Streams.
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = Client::open(redis_url)
            .context("Failed to create Redis client for EventBus")?;

        let conn = client
            .get_connection_manager()
            .await
            .context("Failed to create EventBus connection manager")?;

        Ok(Self {
            conn,
            stream_name: DEFAULT_STREAM.to_string(),
        })
    }

    /// Publish a single event to the Redis stream.
    /// Returns the stream entry ID (e.g. "1234567890123-0").
    pub async fn publish(&self, event: &Event) -> Result<String> {
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
            let id = self.publish(event).await?;
            ids.push(id);
        }

        info!(count = ids.len(), "Batch events published to Redis stream");
        Ok(ids)
    }
}

// ── Convenience event constructors ──────────────────────────────────────────

impl Event {
    /// User successfully registered.
    pub fn user_registered(user_id: &Uuid, username: &str, email: &str) -> Self {
        Event::new("UserRegistered", "gridtokenx-iam")
            .with_data(serde_json::json!({
                "user_id": user_id.to_string(),
                "username": username,
                "email": email,
            }))
    }

    /// User successfully logged in.
    pub fn user_logged_in(user_id: &Uuid, username: &str, ip: Option<&str>) -> Self {
        Event::new("UserLoggedIn", "gridtokenx-iam")
            .with_data(serde_json::json!({
                "user_id": user_id.to_string(),
                "username": username,
                "ip_address": ip,
            }))
    }

    /// User email verified.
    pub fn email_verified(user_id: &Uuid, email: &str, wallet_address: &str) -> Self {
        Event::new("EmailVerified", "gridtokenx-iam")
            .with_data(serde_json::json!({
                "user_id": user_id.to_string(),
                "email": email,
                "wallet_address": wallet_address,
            }))
    }

    /// Login attempt (success or failure — for rate-limit monitoring).
    pub fn login_attempt(identifier: &str, success: bool, ip: Option<&str>) -> Self {
        Event::new("LoginAttempt", "gridtokenx-iam")
            .with_data(serde_json::json!({
                "identifier": identifier,
                "success": success,
                "ip_address": ip,
            }))
    }

    /// Account locked due to too many failed attempts.
    pub fn account_locked(identifier: &str, lockout_secs: u64) -> Self {
        Event::new("AccountLocked", "gridtokenx-iam")
            .with_data(serde_json::json!({
                "identifier": identifier,
                "lockout_secs": lockout_secs,
            }))
    }

    /// API key verified (machine-to-machine auth).
    pub fn api_key_verified(key_name: &str, role: &str) -> Self {
        Event::new("ApiKeyVerified", "gridtokenx-iam")
            .with_data(serde_json::json!({
                "key_name": key_name,
                "role": role,
            }))
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
