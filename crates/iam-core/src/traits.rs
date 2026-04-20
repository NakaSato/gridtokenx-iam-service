use async_trait::async_trait;
use futures::future::BoxFuture;
use uuid::Uuid;
use crate::error::Result;
use crate::domain::identity::{User, UserWallet, ApiKey, UserWithHash};
use serde_json::Value;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use gridtokenx_blockchain_core::rpc::instructions::UserType;

/// Trait for user data access
#[async_trait]
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
pub trait UserRepositoryTrait: Send + Sync {
    async fn find_by_username_or_email(&self, identity: &str) -> Result<Option<UserWithHash>>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>>;
    async fn create(
        &self,
        id: Uuid,
        username: &str,
        email: &str,
        password_hash: &str,
        role: &str,
        first_name: Option<&str>,
        last_name: Option<&str>,
        verification_token: Option<&str>,
    ) -> Result<()>;
    async fn verify_email(&self, email: &str, mock_wallet: &str) -> Result<Option<User>>;
    async fn find_email_by_token(&self, token: &str) -> Result<Option<String>>;
    async fn update_password(&self, email: &str, password_hash: &str) -> Result<u64>;
    async fn mark_user_onboarded(
        &self,
        user_id: Uuid,
        user_type: &str,
        lat: f64,
        long: f64,
        pda: &str,
        signature: &str,
    ) -> Result<()>;
    async fn health_check(&self) -> Result<()>;
}

/// Trait for wallet data access
#[async_trait]
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
pub trait WalletRepositoryTrait: Send + Sync {
    async fn list_by_user_id(&self, user_id: Uuid) -> Result<Vec<UserWallet>>;
    async fn find_by_id_and_user_id(&self, id: Uuid, user_id: Uuid) -> Result<Option<UserWallet>>;
    async fn set_primary(&self, user_id: Uuid, id: Uuid) -> Result<Option<UserWallet>>;
    async fn delete_if_not_primary(&self, user_id: Uuid, id: Uuid) -> Result<bool>;
    async fn exists(&self, user_id: Uuid, id: Uuid) -> Result<bool>;
    async fn has_any_wallet(&self, user_id: Uuid) -> Result<bool>;
    async fn clear_primary(&self, user_id: Uuid) -> Result<()>;
    async fn insert(
        &self,
        user_id: Uuid,
        address: &str,
        label: Option<&str>,
        is_primary: bool,
    ) -> Result<UserWallet>;
    async fn find_primary_address(&self, user_id: Uuid) -> Result<Option<String>>;
    async fn mark_registered(&self, user_id: Uuid, address: &str, signature: &str) -> Result<()>;
}

/// Trait for API key data access
#[async_trait]
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
pub trait ApiKeyRepositoryTrait: Send + Sync {
    async fn find_by_hash(&self, hash: &str) -> Result<Option<ApiKey>>;
    async fn update_last_used(&self, id: Uuid) -> Result<()>;
}

/// Trait for caching operations (Dyn-compatible)
#[async_trait]
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
pub trait CacheTrait: Send + Sync {
    async fn ping(&self) -> Result<()>;
    async fn get_value(&self, key: &str) -> Result<Option<Value>>;
    async fn set_value(&self, key: &str, value: Value, ttl_secs: Option<u64>) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn increment(&self, key: &str) -> Result<u64>;
    async fn exists(&self, key: &str) -> Result<bool>;
}

/// Trait for email notifications
#[async_trait]
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
pub trait EmailTrait: Send + Sync {
    async fn send_password_reset(&self, email: &str, reset_url: &str) -> Result<()>;
}

/// Trait for event publishing
#[async_trait]
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
pub trait EventBusTrait: Send + Sync {
    async fn publish(&self, event: &crate::domain::identity::Event) -> Result<()>;
    async fn publish_batch(&self, events: &[crate::domain::identity::Event]) -> Result<()>;
}

/// Trait for blockchain operations
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
pub trait BlockchainTrait: Send + Sync {
    fn register_user_on_chain(
        &self,
        authority: Pubkey,
        user_type: UserType,
        lat_e7: i32,
        long_e7: i32,
        h3_index: u64,
        shard_id: u8,
    ) -> BoxFuture<'static, Result<Signature>>;
}


