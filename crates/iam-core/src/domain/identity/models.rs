use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// UserType represents the role of a user in the energy ecosystem (on-chain).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub enum UserType {
    Prosumer = 0,
    Consumer = 1,
}

/// UserWallet represents a blockchain wallet linked to a user.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserWallet {
    pub id: Uuid,
    pub user_id: Uuid,
    pub wallet_address: String,
    pub label: Option<String>,
    pub is_primary: bool,
    pub verified: bool,
    pub blockchain_registered: bool,
    pub user_account_pda: Option<String>,
    pub shard_id: Option<u8>,
    pub blockchain_tx_signature: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// User represents the core identity entity.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub wallet_address: Option<String>,
    pub is_active: bool,
}

/// User profile with password hash (internal use).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserWithHash {
    pub user: User,
    pub password_hash: String,
}

/// AuthResult contains the result of a successful authentication.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthResult {
    pub access_token: String,
    pub expires_in: i64,
    pub user: User,
}

/// RegistrationResult contains the result of a successful registration.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegistrationResult {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub message: String,
}

/// VerifyEmailResult contains the result of an email verification.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VerifyEmailResult {
    pub success: bool,
    pub message: String,
    pub wallet_address: Option<String>,
    pub auth: Option<AuthResult>,
}

/// OnChainOnboardingResult contains the result of an on-chain onboarding.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OnChainOnboardingResult {
    pub success: bool,
    pub message: String,
    pub transaction_signature: Option<String>,
}
