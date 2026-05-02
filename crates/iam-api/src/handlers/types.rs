//! API request and response types for the IAM service.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};
use iam_core::error::{ApiError, Result as ApiResult};
use iam_core::domain::identity::Claims;

/// Extractor for authenticated user information from JWT claims.
pub struct AuthenticatedUser(pub Claims);

impl FromRequestParts<iam_logic::AuthService> for AuthenticatedUser
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &iam_logic::AuthService) -> ApiResult<Self> {
        tracing::info!("📥 [IAM] Request headers: {:?}", parts.headers);
        let auth_header = parts.headers
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| {
                tracing::warn!("⚠️ [IAM] Missing Authorization header in request");
                ApiError::Unauthorized("Missing Authorization header".to_string())
            })?;

        if !auth_header.starts_with("Bearer ") {
            return Err(ApiError::Unauthorized("Invalid Authorization header format".to_string()));
        }

        let token = &auth_header[7..];
        
        let jwt_service: &iam_logic::JwtService = state.jwt_service();
        
        let claims = jwt_service.decode_token(token).map_err(|e| {
            tracing::error!("Token decoding failed: {}", e);
            e
        })?;
        
        // ── Semantic Logging ──────────────────────────────────────────
        // Record user_id in the current tracing span for all subsequent logs
        tracing::Span::current().record("user_id", &claims.sub.to_string());
        
        Ok(AuthenticatedUser(claims))
    }
}

/// Request for user login.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoginRequest {
    /// Username or email address.
    pub username: String,
    /// User password.
    pub password: String,
}

/// Successful authentication response.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AuthResponse {
    /// JWT access token.
    pub access_token: String,
    /// Token expiration time in seconds.
    pub expires_in: i64,
    /// User profile information.
    pub user: UserResponse,
}

/// User profile information returned in API responses.
#[derive(Debug, Serialize, Deserialize, ToSchema, Default)]
pub struct UserResponse {
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
    /// Account status (e.g. verified, pending_verification).
    pub status: String,
}

/// Request for new user registration.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegistrationRequest {
    /// Unique username.
    pub username: String,
    /// User email address.
    pub email: String,
    /// User password.
    pub password: String,
    /// User first name (optional).
    #[serde(default)]
    pub first_name: Option<String>,
    /// User last name (optional).
    #[serde(default)]
    pub last_name: Option<String>,
}

/// Response for a successful registration.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegistrationResponse {
    /// Unique user ID.
    pub id: Uuid,
    /// Unique username.
    pub username: String,
    /// User email address.
    pub email: String,
    /// Status of the account (e.g. pending_verification).
    pub status: String,
    /// Status message (optional).
    pub message: Option<String>,
}

/// Request for email verification.
#[derive(Debug, Deserialize, ToSchema, serde::Serialize)]
pub struct VerifyEmailRequest {
    /// Verification token sent via email.
    pub token: String,
}

/// Response for email verification.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VerifyEmailResponse {
    /// Whether verification was successful.
    pub success: bool,
    /// Status message.
    pub message: String,
    /// Assigned Solana wallet address (optional).
    pub wallet_address: Option<String>,
    /// Authentication info if auto-login is performed (optional).
    pub auth: Option<AuthResponse>,
}

impl VerifyEmailResponse {
    /// Creates a simple verification response without auth info.
    pub fn simple(success: bool, message: &str) -> Self {
        Self {
            success,
            message: message.to_string(),
            wallet_address: None,
            auth: None,
        }
    }
}

/// Details of a user's Solana wallet.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserWallet {
    /// Unique wallet ID.
    pub id: Uuid,
    /// Owner user ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Uuid>,
    /// Solana wallet address.
    pub wallet_address: String,
    /// User-defined label for the wallet.
    pub label: Option<String>,
    /// Whether this is the primary wallet.
    pub is_primary: bool,
    /// Current status of the wallet (e.g. unverified, verified).
    pub status: String,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
    
    // Internal fields omitted from simple views if needed
    #[serde(skip_serializing)]
    pub verified: bool,
    #[serde(skip_serializing)]
    pub blockchain_registered: bool,
}

/// Request to link a new Solana wallet.
#[derive(Debug, Deserialize, ToSchema)]
pub struct LinkWalletRequest {
    /// Solana wallet address.
    pub wallet_address: String,
    /// Optional label for the wallet.
    pub label: Option<String>,
    /// Whether to set this wallet as primary.
    pub is_primary: bool,
}

/// Response after linking a wallet.
// Note: In the redesign, this returns the wallet object fields directly.
pub type LinkWalletResponse = UserWallet;

/// Classification of users for on-chain participation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UserType {
    /// User who both produces and consumes energy.
    Prosumer = 0,
    /// User who only consumes energy.
    Consumer = 1,
}

/// Geographic location for onboarding.
#[derive(Debug, Deserialize, ToSchema)]
pub struct LocationRequest {
    /// Latitude coordinate (multiplied by 1e7).
    pub lat_e7: i32,
    /// Longitude coordinate (multiplied by 1e7).
    pub long_e7: i32,
}

/// Request for on-chain onboarding.
#[derive(Debug, Deserialize, ToSchema)]
pub struct OnChainOnboardingRequest {
    /// Type of user participating on-chain.
    pub user_type: UserType,
    /// User location.
    pub location: LocationRequest,
    /// H3 geospatial index (optional).
    pub h3_index: Option<u64>,
    /// Preferred shard ID (optional).
    pub shard_id: Option<u8>,
}

/// Response after on-chain onboarding.
#[derive(Debug, Serialize, ToSchema)]
pub struct OnChainOnboardingResponse {
    /// Current status (e.g. processing).
    pub status: String,
    /// Solana transaction signature (optional).
    pub transaction_signature: Option<String>,
    /// Status message.
    pub message: String,
}

/// Response containing a list of user wallets.
#[derive(Debug, Serialize, ToSchema)]
pub struct WalletListResponse {
    /// List of wallets.
    pub wallets: Vec<UserWallet>,
}

/// Empty request body for setting primary wallet.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetPrimaryWalletRequest {
    // body intentionally empty — wallet_id comes from path
}

/// Response after deleting a wallet.
#[derive(Debug, Serialize, ToSchema)]
pub struct DeleteWalletResponse {
    /// Status message.
    pub message: String,
}

/// Request to initiate password reset.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ForgotPasswordRequest {
    /// User email address.
    pub email: String,
}

/// Response after initiating password reset.
#[derive(Debug, Serialize, ToSchema)]
pub struct ForgotPasswordResponse {
    /// Status message.
    pub message: String,
}

/// Request to reset password with a token.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ResetPasswordRequest {
    /// Reset token sent via email.
    pub token: String,
    /// New password.
    pub new_password: String,
}

/// Response after password reset.
#[derive(Debug, Serialize, ToSchema)]
pub struct ResetPasswordResponse {
    /// Status message.
    pub message: String,
}

/// Response containing global system configuration for frontends.
#[derive(Debug, Serialize, ToSchema)]
pub struct SystemConfigResponse {
    /// Deployment environment.
    pub environment: String,
    /// Solana RPC endpoint.
    pub solana_rpc_url: String,
    /// Solana cluster.
    pub solana_cluster: String,
    /// Registry program ID.
    pub registry_program_id: String,
    /// Oracle program ID.
    pub oracle_program_id: String,
    /// Governance program ID.
    pub governance_program_id: String,
    /// Energy Token program ID.
    pub energy_token_program_id: String,
    /// Trading program ID.
    pub trading_program_id: String,
    /// Energy Token mint address.
    pub energy_token_mint: String,
    /// Currency Token mint address.
    pub currency_token_mint: String,
}
