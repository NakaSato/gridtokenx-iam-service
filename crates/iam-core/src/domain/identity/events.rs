use uuid::Uuid;
use chrono::Utc;
use serde::{Serialize, Deserialize};

/// Domain event that can be published to inter-service messaging systems.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Creates a new domain event with the specified type and source.
    pub fn new(event_type: &str, source: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type: event_type.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            data: None,
            source: source.to_string(),
        }
    }

    /// Attaches an arbitrary JSON payload to the event.
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
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

    /// User successfully onboarded on-chain.
    pub fn user_onboarded(
        user_id: &Uuid,
        wallet_address: &str,
        user_account_pda: &str,
        tx_signature: &str,
        user_type: &str,
        shard_id: u8,
    ) -> Self {
        Event::new("UserOnboarded", "gridtokenx-iam")
            .with_data(serde_json::json!({
                "user_id": user_id.to_string(),
                "wallet_address": wallet_address,
                "user_account_pda": user_account_pda,
                "transaction_signature": tx_signature,
                "user_type": user_type,
                "shard_id": shard_id,
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

    /// User wallet linked and registered on-chain.
    pub fn user_wallet_linked(
        user_id: &Uuid,
        wallet_address: &str,
        user_account_pda: &str,
        tx_signature: &str,
        shard_id: u8,
    ) -> Self {
        Event::new("UserWalletLinked", "gridtokenx-iam")
            .with_data(serde_json::json!({
                "user_id": user_id.to_string(),
                "wallet_address": wallet_address,
                "user_account_pda": user_account_pda,
                "transaction_signature": tx_signature,
                "shard_id": shard_id,
            }))
    }

    /// User requested a password reset.
    pub fn password_reset_requested(user_id: &Uuid, email: &str, reset_url: &str) -> Self {
        Event::new("PasswordResetRequested", "gridtokenx-iam")
            .with_data(serde_json::json!({
                "user_id": user_id.to_string(),
                "email": email,
                "reset_url": reset_url,
            }))
    }
}
