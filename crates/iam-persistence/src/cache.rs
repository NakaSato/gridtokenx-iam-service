//! Cache service backed by Redis with automatic reconnection.
//!
//! Uses `ConnectionManager` for transparent reconnect handling.
//! All operations serialize/deserialize via serde JSON.

use redis::{AsyncCommands, Client, aio::ConnectionManager};
use tracing::{warn, info};
use anyhow::{Result, Context};

/// Default TTL for cached entries (5 minutes).
const DEFAULT_TTL_SECS: u64 = 300;

use iam_core::traits::CacheTrait;
use iam_core::error::{ApiError, Result as IamResult};

/// Redis-backed cache service.
#[derive(Clone)]
pub struct CacheService {
    client: Client,
    conn: ConnectionManager,
    default_ttl: u64,
}

impl CacheService {
    /// Create a new `CacheService` and verify the connection.
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = Client::open(redis_url)
            .context("Failed to create Redis client")?;

        let conn = client
            .get_connection_manager()
            .await
            .context("Failed to create Redis connection manager")?;

        // Ping to verify connectivity
        let mut test_conn = conn.clone();
        let _: String = test_conn.ping()
            .await
            .context("Redis ping failed")?;

        info!("✅ Redis cache service connected");

        Ok(Self {
            client,
            conn,
            default_ttl: DEFAULT_TTL_SECS,
        })
    }

    /// Get the underlying Redis client (for creating additional connections).
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Ping Redis to verify connectivity.
    pub async fn ping(&self) -> Result<String> {
        let mut conn = self.conn.clone();
        let pong: String = conn.ping()
            .await
            .context("Redis ping failed")?;
        Ok(pong)
    }

    /// Internal set implementation.
    async fn set_raw<T: serde::Serialize + Send>(&self, key: &str, value: &T, ttl: u64) -> Result<()> {
        let serialized = serde_json::to_string(value)
            .context("Failed to serialize cache value")?;

        let mut conn = self.conn.clone();
        let _: () = conn.set_ex(key, &serialized, ttl)
            .await
            .context("Redis SET failed")?;
        Ok(())
    }

    /// Internal get implementation.
    async fn get_raw<T: serde::de::DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>> {
        let mut conn = self.conn.clone();
        let raw: Option<String> = conn.get(key)
            .await
            .context("Redis GET failed")?;

        match raw {
            Some(val) => {
                let parsed = serde_json::from_str(&val)
                    .context("Failed to deserialize cache value")?;
                Ok(Some(parsed))
            }
            None => Ok(None),
        }
    }

    /// Internal delete implementation.
    async fn delete_raw(&self, key: &str) -> Result<()> {
        let mut conn = self.conn.clone();
        let _: u32 = conn.del(key)
            .await
            .context("Redis DEL failed")?;
        Ok(())
    }

    /// Internal increment implementation.
    async fn increment_raw(&self, key: &str) -> Result<u64> {
        let mut conn = self.conn.clone();
        let val: u64 = conn.incr(key, 1)
            .await
            .context("Redis INCR failed")?;
        Ok(val)
    }

    /// Internal exists implementation.
    async fn exists_raw(&self, key: &str) -> Result<bool> {
        let mut conn = self.conn.clone();
        let exists: bool = conn.exists(key)
            .await
            .context("Redis EXISTS failed")?;
        Ok(exists)
    }

    /// Atomically increment a counter with a TTL. Returns the new value.
    pub async fn increment_with_ttl(&self, key: &str, ttl_secs: u64) -> Result<i64> {
        let mut conn = self.conn.clone();
        let mut pipe = redis::pipe();
        let (val,): (i64,) = pipe
            .atomic()
            .incr(key, 1)
            .expire(key, ttl_secs as i64)
            .ignore()
            .incr(key, 0)
            .query_async(&mut conn)
            .await
            .context("Redis INCR pipeline failed")?;
        Ok(val)
    }

    /// Set a key only if it doesn't exist (NX). Used for distributed locking.
    /// Returns `true` if the lock was acquired.
    pub async fn set_if_not_exists(
        &self,
        key: &str,
        value: &str,
        ttl_secs: u64,
    ) -> Result<bool> {
        let mut conn = self.conn.clone();
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg(value)
            .arg("NX")
            .arg("EX")
            .arg(ttl_secs)
            .query_async(&mut conn)
            .await
            .context("Redis SET NX failed")?;
        Ok(result.is_some())
    }

    /// Release a distributed lock by deleting the key.
    pub async fn release_lock(&self, key: &str) -> Result<()> {
        let mut conn = self.conn.clone();
        let _: () = conn.del(key)
            .await
            .context("Redis lock release failed")?;
        Ok(())
    }

    /// Health check — pings Redis and returns `true` if responsive.
    pub async fn health_check(&self) -> bool {
        let mut conn = self.conn.clone();
        match conn.ping::<String>().await {
            Ok(_) => true,
            Err(e) => {
                warn!("Redis health check failed: {}", e);
                false
            }
        }
    }
}
use async_trait::async_trait;

// ... existing imports ...

#[async_trait]
impl CacheTrait for CacheService {
    async fn ping(&self) -> IamResult<()> {
        self.ping().await.map(|_| ()).map_err(|e| ApiError::Internal(e.to_string()))
    }

    async fn get_value(&self, key: &str) -> IamResult<Option<serde_json::Value>> {
        self.get_raw(key).await.map_err(|e| ApiError::Internal(e.to_string()))
    }

    async fn set_value(&self, key: &str, value: serde_json::Value, ttl_secs: Option<u64>) -> IamResult<()> {
        let ttl = ttl_secs.unwrap_or(self.default_ttl);
        self.set_raw(key, &value, ttl).await.map_err(|e| ApiError::Internal(e.to_string()))
    }

    async fn delete(&self, key: &str) -> IamResult<()> {
        self.delete_raw(key).await.map_err(|e| ApiError::Internal(e.to_string()))
    }

    async fn increment(&self, key: &str) -> IamResult<u64> {
        self.increment_raw(key).await.map_err(|e| ApiError::Internal(e.to_string()))
    }

    async fn exists(&self, key: &str) -> IamResult<bool> {
        self.exists_raw(key).await.map_err(|e| ApiError::Internal(e.to_string()))
    }
}

/// Cache key patterns for IAM service.
pub mod keys {
    /// Login attempt counter for rate limiting.
    pub fn login_attempts(identifier: &str) -> String {
        format!("iam:login_attempts:{}", identifier)
    }

    /// Account lock status after too many failed logins.
    pub fn account_lock(identifier: &str) -> String {
        format!("iam:account_lock:{}", identifier)
    }

    /// Cached user profile (by user ID).
    pub fn user_profile(user_id: &str) -> String {
        format!("iam:user:profile:{}", user_id)
    }

    /// Cached API key lookup (by hash).
    pub fn api_key(key_hash: &str) -> String {
        format!("iam:api_key:{}", key_hash)
    }

    /// Email verification token TTL.
    pub fn email_verification_token(token: &str) -> String {
        format!("iam:email_verify:{}", token)
    }

    /// Password reset token TTL.
    pub fn password_reset_token(token: &str) -> String {
        format!("iam:password_reset:{}", token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_patterns() {
        assert_eq!(
            keys::login_attempts("user@example.com"),
            "iam:login_attempts:user@example.com"
        );
        assert_eq!(
            keys::user_profile("abc-123"),
            "iam:user:profile:abc-123"
        );
        assert_eq!(
            keys::api_key("deadbeef"),
            "iam:api_key:deadbeef"
        );
    }
}
