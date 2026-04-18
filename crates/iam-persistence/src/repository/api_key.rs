use sqlx::PgPool;
use uuid::Uuid;
use iam_core::error::{ApiError, Result};
use chrono::{DateTime, Utc};

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct ApiKeyRow {
    pub id: Uuid,
    pub key_hash: String,
    pub name: String,
    pub role: String,
    pub permissions: Option<Vec<String>>,
    pub is_active: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
}

use async_trait::async_trait;
use iam_core::traits::ApiKeyRepositoryTrait;
use iam_core::domain::identity::ApiKey;

impl ApiKeyRow {
    pub fn into_domain(self) -> ApiKey {
        ApiKey {
            id: self.id,
            key_hash: self.key_hash,
            name: self.name,
            role: self.role,
            permissions: self.permissions.unwrap_or_default(),
            is_active: self.is_active.unwrap_or(true),
            created_at: self.created_at.unwrap_or_default(),
            last_used_at: self.last_used_at,
        }
    }
}

pub struct ApiKeyRepository {
    pool: PgPool,
}

impl ApiKeyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn get_pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl ApiKeyRepositoryTrait for ApiKeyRepository {
    async fn find_by_hash(&self, hash: &str) -> Result<Option<ApiKey>> {
        let row = sqlx::query_as::<_, ApiKeyRow>(
            "SELECT * FROM api_keys WHERE key_hash = $1 AND (is_active = true OR is_active IS NULL)"
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ApiError::Database(e))?;

        Ok(row.map(|r| r.into_domain()))
    }

    async fn update_last_used(&self, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(ApiError::from)
    }
}
