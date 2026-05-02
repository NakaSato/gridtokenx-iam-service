//! Domain models for identity management.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// UserType represents the role of a user in the energy ecosystem (on-chain).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub enum UserType {
    /// User who both produces and consumes energy.
    Prosumer = 0,
    /// User who only consumes energy.
    Consumer = 1,
}

/// UserWallet represents a blockchain wallet linked to a user.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserWallet {
    /// Unique wallet ID.
    pub id: Uuid,
    /// Owner user ID.
    pub user_id: Uuid,
    /// Solana wallet address.
    pub wallet_address: String,
    /// User-defined label for the wallet.
    pub label: Option<String>,
    /// Whether this is the primary wallet.
    pub is_primary: bool,
    /// Whether the wallet address has been verified.
    pub verified: bool,
    /// Whether the wallet is registered on-chain.
    pub blockchain_registered: bool,
    /// Derived Solana PDA for the user account (optional).
    pub user_account_pda: Option<String>,
    /// Shard ID for the user's market data (optional).
    pub shard_id: Option<u8>,
    /// Transaction signature of the on-chain registration (optional).
    pub blockchain_tx_signature: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// User represents the core identity entity.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct User {
    /// Unique user ID.
    pub id: Uuid,
    /// Unique username.
    pub username: String,
    /// User email address.
    pub email: String,
    /// User role (e.g., user, admin).
    pub role: String,
    /// User first name (optional).
    pub first_name: Option<String>,
    /// User last name (optional).
    pub last_name: Option<String>,
    /// Primary Solana wallet address (optional).
    pub wallet_address: Option<String>,
    /// Whether the user account is active/verified.
    pub is_active: bool,
    /// Whether the user is registered on-chain.
    pub blockchain_registered: bool,
    /// Type of user participating on-chain (optional).
    pub user_type: Option<UserType>,
    /// Latitude coordinate for location-based services (optional).
    pub latitude: Option<f64>,
    /// Longitude coordinate for location-based services (optional).
    pub longitude: Option<f64>,
}

/// User profile with password hash (internal use).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserWithHash {
    /// Inner user domain model.
    pub user: User,
    /// Bcrypt hash of the user's password.
    pub password_hash: String,
}

/// AuthResult contains the result of a successful authentication.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthResult {
    /// JWT access token.
    pub access_token: String,
    /// Token expiration time in seconds.
    pub expires_in: i64,
    /// User profile information.
    pub user: User,
}

/// RegistrationResult contains the result of a successful registration.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegistrationResult {
    /// Unique user ID.
    pub id: Uuid,
    /// Unique username.
    pub username: String,
    /// User email address.
    pub email: String,
    /// User first name (optional).
    pub first_name: Option<String>,
    /// User last name (optional).
    pub last_name: Option<String>,
    /// Status message.
    pub message: String,
}

/// VerifyEmailResult contains the result of an email verification.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VerifyEmailResult {
    /// Whether verification was successful.
    pub success: bool,
    /// Status message.
    pub message: String,
    /// Assigned Solana wallet address (optional).
    pub wallet_address: Option<String>,
    /// Authentication info if auto-login is performed (optional).
    pub auth: Option<AuthResult>,
}

/// OnChainOnboardingResult contains the result of an on-chain onboarding.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OnChainOnboardingResult {
    /// Whether onboarding was successful.
    pub success: bool,
    /// Status message.
    pub message: String,
    /// Solana transaction signature (optional).
    pub transaction_signature: Option<String>,
}
