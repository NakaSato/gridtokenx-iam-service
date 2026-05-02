//! Core trait definitions for the IAM service.
//! These traits define the interfaces for repositories, services, and infrastructure components,
//! allowing for easy mocking and decoupled implementations.

use async_trait::async_trait;
use futures::future::BoxFuture;
use uuid::Uuid;
use crate::error::Result;
use crate::domain::identity::{User, UserWallet, ApiKey, UserWithHash};
use serde_json::Value;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use gridtokenx_blockchain_core::rpc::instructions::UserType;

/// Trait for user data access and persistence operations.
#[async_trait]
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
pub trait UserRepositoryTrait: Send + Sync {
    /// Finds a user by their username or email address.
    async fn find_by_username_or_email(&self, identity: &str) -> Result<Option<UserWithHash>>;
    /// Finds a user by their unique user ID.
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>>;
    /// Creates a new user record in the repository.
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
    /// Marks a user's email as verified and assigns a mock wallet.
    async fn verify_email(&self, email: &str, mock_wallet: &str) -> Result<Option<User>>;
    /// Finds the email address associated with a verification token.
    async fn find_email_by_token(&self, token: &str) -> Result<Option<String>>;
    /// Updates a user's password hash.
    async fn update_password(&self, email: &str, password_hash: &str) -> Result<u64>;
    /// Marks a user as successfully onboarded on the blockchain.
    async fn mark_user_onboarded(
        &self,
        user_id: Uuid,
        user_type: &str,
        lat: f64,
        long: f64,
        pda: &str,
        signature: &str,
    ) -> Result<()>;
    /// Performs a health check on the underlying repository.
    async fn health_check(&self) -> Result<()>;
}

/// Trait for wallet data access and persistence operations.
#[async_trait]
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
pub trait WalletRepositoryTrait: Send + Sync {
    /// Lists all wallets associated with a specific user.
    async fn list_by_user_id(&self, user_id: Uuid) -> Result<Vec<UserWallet>>;
    /// Finds a specific wallet by ID and user ID.
    async fn find_by_id_and_user_id(&self, id: Uuid, user_id: Uuid) -> Result<Option<UserWallet>>;
    /// Sets a wallet as the primary wallet for its owner.
    async fn set_primary(&self, user_id: Uuid, id: Uuid) -> Result<Option<UserWallet>>;
    /// Deletes a wallet if it is not the primary wallet.
    async fn delete_if_not_primary(&self, user_id: Uuid, id: Uuid) -> Result<bool>;
    /// Checks if a wallet exists for a given user.
    async fn exists(&self, user_id: Uuid, id: Uuid) -> Result<bool>;
    /// Checks if a user has any wallets registered.
    async fn has_any_wallet(&self, user_id: Uuid) -> Result<bool>;
    /// Clears the primary flag for all wallets of a given user.
    async fn clear_primary(&self, user_id: Uuid) -> Result<()>;
    /// Inserts a new wallet record.
    async fn insert(
        &self,
        user_id: Uuid,
        address: &str,
        label: Option<&str>,
        is_primary: bool,
    ) -> Result<UserWallet>;
    /// Finds the primary wallet address for a user.
    async fn find_primary_address(&self, user_id: Uuid) -> Result<Option<String>>;
    /// Marks a wallet as registered on-chain with a transaction signature.
    async fn mark_registered(&self, user_id: Uuid, address: &str, signature: &str) -> Result<()>;
}

/// Trait for API key data access and lifecycle management.
#[async_trait]
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
pub trait ApiKeyRepositoryTrait: Send + Sync {
    /// Finds an API key record by its cryptographic hash.
    async fn find_by_hash(&self, hash: &str) -> Result<Option<ApiKey>>;
    /// Updates the last used timestamp for an API key.
    async fn update_last_used(&self, id: Uuid) -> Result<()>;
}

/// Trait for low-level caching operations.
#[async_trait]
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
pub trait CacheTrait: Send + Sync {
    /// Checks connectivity to the cache server.
    async fn ping(&self) -> Result<()>;
    /// Retrieves a JSON value from the cache.
    async fn get_value(&self, key: &str) -> Result<Option<Value>>;
    /// Stores a JSON value in the cache with an optional TTL.
    async fn set_value(&self, key: &str, value: Value, ttl_secs: Option<u64>) -> Result<()>;
    /// Deletes a value from the cache.
    async fn delete(&self, key: &str) -> Result<()>;
    /// Atomically increments a numeric value in the cache.
    async fn increment(&self, key: &str) -> Result<u64>;
    /// Checks if a key exists in the cache.
    async fn exists(&self, key: &str) -> Result<bool>;
}

/// Trait for sending email notifications.
#[async_trait]
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
pub trait EmailTrait: Send + Sync {
    /// Sends a password reset link to the specified email address.
    async fn send_password_reset(&self, email: &str, reset_url: &str) -> Result<()>;
}

/// Trait for publishing domain events.
#[async_trait]
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
pub trait EventBusTrait: Send + Sync {
    /// Publishes a single domain event.
    async fn publish(&self, event: &crate::domain::identity::Event) -> Result<()>;
    /// Publishes a batch of domain events efficiently.
    async fn publish_batch(&self, events: &[crate::domain::identity::Event]) -> Result<()>;
}

/// Trait for high-level blockchain interactions.
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
pub trait BlockchainTrait: Send + Sync {
    /// Registers a user on the Solana blockchain by calling the Registry program.
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
