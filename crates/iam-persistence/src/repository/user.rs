use sqlx::PgPool;
use uuid::Uuid;
use iam_core::error::{ApiError, Result};

#[derive(Debug, sqlx::FromRow, serde::Serialize, serde::Deserialize, Clone)]
pub struct UserRow {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub wallet_address: Option<String>,
    pub blockchain_registered: bool,
    pub user_type: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

use async_trait::async_trait;
use iam_core::traits::UserRepositoryTrait;
use iam_core::domain::identity::{User, UserWithHash, UserType};

impl UserRow {
    pub fn into_domain(self) -> User {
        let ut = self.user_type.and_then(|s| match s.as_str() {
            "Prosumer" | "0" => Some(UserType::Prosumer),
            "Consumer" | "1" => Some(UserType::Consumer),
            _ => None,
        });

        User {
            id: self.id,
            username: self.username,
            email: self.email,
            role: self.role,
            first_name: self.first_name,
            last_name: self.last_name,
            wallet_address: self.wallet_address,
            is_active: true, // Rows fetched via find/verify are active
            blockchain_registered: self.blockchain_registered,
            user_type: ut,
            latitude: self.latitude,
            longitude: self.longitude,
        }
    }

    pub fn into_domain_with_hash(self) -> UserWithHash {
        UserWithHash {
            password_hash: self.password_hash.clone(),
            user: self.into_domain(),
        }
    }
}

pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepositoryTrait for UserRepository {
    async fn find_by_username_or_email(&self, identity: &str) -> Result<Option<UserWithHash>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, username, email, password_hash, role::text as role, first_name, last_name, wallet_address,
                    blockchain_registered, user_type, latitude, longitude
             FROM users
             WHERE (username = $1 OR email = $1) AND is_active = true
             LIMIT 1",
        )
        .bind(identity)
        .fetch_optional(&self.pool)
        .await
        .map_err(ApiError::from)?;

        Ok(row.map(|r| r.into_domain_with_hash()))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, username, email, password_hash, role::text as role, first_name, last_name, wallet_address,
                    blockchain_registered, user_type, latitude, longitude
             FROM users WHERE id = $1 AND is_active = true LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(ApiError::from)?;

        Ok(row.map(|r| r.into_domain()))
    }

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
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash, role, first_name, last_name, is_active, email_verification_token)
             VALUES ($1, $2, $3, $4, $5::text::user_role, $6, $7, false, $8)",
        )
        .bind(id)
        .bind(username)
        .bind(email)
        .bind(password_hash)
        .bind(role)
        .bind(first_name)
        .bind(last_name)
        .bind(verification_token)
        .execute(&self.pool)
        .await
        .map_err(ApiError::from)?;
        Ok(())
    }

    async fn verify_email(&self, email: &str, mock_wallet: &str) -> Result<Option<User>> {
        let row = sqlx::query_as::<_, UserRow>(
            "UPDATE users
             SET is_active = true,
                 email_verified = true,
                 email_verified_at = NOW(),
                 wallet_address = COALESCE(wallet_address, $2)
             WHERE email = $1
             RETURNING id, username, email, password_hash, role::text as role, first_name, last_name, wallet_address,
                       blockchain_registered, user_type, latitude, longitude",
        )
        .bind(email)
        .bind(mock_wallet)
        .fetch_optional(&self.pool)
        .await
        .map_err(ApiError::from)?;

        Ok(row.map(|r| r.into_domain()))
    }

    async fn find_email_by_token(&self, token: &str) -> Result<Option<String>> {
        let row = sqlx::query_scalar::<_, String>(
            "SELECT email FROM users WHERE email_verification_token = $1 LIMIT 1",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .map_err(ApiError::from)?;

        Ok(row)
    }

    async fn update_password(&self, email: &str, password_hash: &str) -> Result<u64> {
        sqlx::query(
            "UPDATE users SET password_hash = $1 WHERE lower(email) = lower($2) AND is_active = true",
        )
        .bind(password_hash)
        .bind(email)
        .execute(&self.pool)
        .await
        .map(|r| r.rows_affected())
        .map_err(ApiError::from)
    }

    async fn mark_user_onboarded(
        &self,
        user_id: Uuid,
        user_type: &str,
        lat: f64,
        long: f64,
        pda: &str,
        signature: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE users
             SET blockchain_registered = true,
                 user_type = $1,
                 latitude = $2,
                 longitude = $3,
                 user_account_pda = $4,
                 blockchain_tx_signature = $5,
                 blockchain_confirmed_at = NOW(),
                 blockchain_status = 'confirmed'
             WHERE id = $6",
        )
        .bind(user_type)
        .bind(lat)
        .bind(long)
        .bind(pda)
        .bind(signature)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(ApiError::from)?;

        Ok(())
    }

    async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(ApiError::from)
    }
}
