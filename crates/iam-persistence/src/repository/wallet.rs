use sqlx::PgPool;
use uuid::Uuid;
use iam_core::error::{ApiError, Result};
use chrono::{DateTime, Utc};
use async_trait::async_trait;

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct UserWalletRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub wallet_address: String,
    pub label: Option<String>,
    pub is_primary: bool,
    pub verified: bool,
    pub blockchain_registered: bool,
    pub user_account_pda: Option<String>,
    pub shard_id: Option<i16>,
    pub blockchain_tx_signature: Option<String>,
    pub created_at: DateTime<Utc>,
}

use iam_core::traits::WalletRepositoryTrait;
use iam_core::domain::identity::UserWallet;

impl UserWalletRow {
    pub fn into_domain(self) -> UserWallet {
        UserWallet {
            id: self.id,
            user_id: self.user_id,
            wallet_address: self.wallet_address,
            label: self.label,
            is_primary: self.is_primary,
            verified: self.verified,
            blockchain_registered: self.blockchain_registered,
            user_account_pda: self.user_account_pda,
            shard_id: self.shard_id.map(|v| v as u8),
            blockchain_tx_signature: self.blockchain_tx_signature,
            created_at: self.created_at,
        }
    }
}

pub struct WalletRepository {
    pool: PgPool,
}

impl WalletRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WalletRepositoryTrait for WalletRepository {
    async fn list_by_user_id(&self, user_id: Uuid) -> Result<Vec<UserWallet>> {
        let rows = sqlx::query_as::<_, UserWalletRow>(
            "SELECT id, user_id, wallet_address, label, is_primary, verified, blockchain_registered,
                    user_account_pda, shard_id, blockchain_tx_signature, created_at
             FROM user_wallets WHERE user_id = $1 ORDER BY is_primary DESC, created_at ASC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)?;

        Ok(rows.into_iter().map(|r| r.into_domain()).collect())
    }

    async fn find_by_id_and_user_id(&self, id: Uuid, user_id: Uuid) -> Result<Option<UserWallet>> {
        let row = sqlx::query_as::<_, UserWalletRow>(
            "SELECT id, user_id, wallet_address, label, is_primary, verified, blockchain_registered,
                    user_account_pda, shard_id, blockchain_tx_signature, created_at
             FROM user_wallets WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(ApiError::from)?;

        Ok(row.map(|r| r.into_domain()))
    }

    async fn set_primary(&self, user_id: Uuid, wallet_id: Uuid) -> Result<Option<UserWallet>> {
        let mut tx = self.pool.begin().await.map_err(ApiError::from)?;

        sqlx::query("UPDATE user_wallets SET is_primary = false WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::from)?;

        let w = sqlx::query_as::<_, UserWalletRow>(
            "UPDATE user_wallets SET is_primary = true
             WHERE id = $1 AND user_id = $2
             RETURNING id, user_id, wallet_address, label, is_primary, verified, blockchain_registered,
                       user_account_pda, shard_id, blockchain_tx_signature, created_at",
        )
        .bind(wallet_id)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(ApiError::from)?;

        tx.commit().await.map_err(ApiError::from)?;

        Ok(w.map(|r| r.into_domain()))
    }

    async fn delete_if_not_primary(&self, user_id: Uuid, wallet_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM user_wallets WHERE id = $1 AND user_id = $2 AND is_primary = false",
        )
        .bind(wallet_id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(ApiError::from)?;

        Ok(result.rows_affected() > 0)
    }

    async fn exists(&self, user_id: Uuid, wallet_id: Uuid) -> Result<bool> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM user_wallets WHERE id = $1 AND user_id = $2)",
        )
        .bind(wallet_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(ApiError::from)?;
        Ok(exists)
    }

    async fn has_any_wallet(&self, user_id: Uuid) -> Result<bool> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM user_wallets WHERE user_id = $1)",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(ApiError::from)?;
        Ok(exists)
    }

    async fn clear_primary(&self, user_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE user_wallets SET is_primary = false WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(ApiError::from)?;
        Ok(())
    }

    async fn insert(&self, user_id: Uuid, wallet_address: &str, label: Option<&str>, is_primary: bool) -> Result<UserWallet> {
        let row = sqlx::query_as::<_, UserWalletRow>(
            "INSERT INTO user_wallets (id, user_id, wallet_address, label, is_primary, verified, blockchain_registered)
             VALUES ($1, $2, $3, $4, $5, false, false)
             RETURNING id, user_id, wallet_address, label, is_primary, verified, blockchain_registered,
                       user_account_pda, shard_id, blockchain_tx_signature, created_at",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(wallet_address)
        .bind(label)
        .bind(is_primary)
        .fetch_one(&self.pool)
        .await
        .map_err(ApiError::from)?;

        Ok(row.into_domain())
    }

    async fn find_primary_address(&self, user_id: Uuid) -> Result<Option<String>> {
        let address = sqlx::query_scalar::<_, String>(
            "SELECT wallet_address FROM user_wallets WHERE user_id = $1 AND is_primary = true LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(ApiError::from)?;

        Ok(address)
    }

    async fn mark_registered(&self, user_id: Uuid, address: &str, signature: &str) -> Result<()> {
        sqlx::query(
            "UPDATE user_wallets SET blockchain_registered = true, blockchain_tx_signature = $1 WHERE user_id = $2 AND wallet_address = $3"
        )
        .bind(signature)
        .bind(user_id)
        .bind(address)
        .execute(&self.pool)
        .await
        .map_err(ApiError::from)?;

        Ok(())
    }
}
