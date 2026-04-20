use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};
use iam_core::error::{ApiError, Result as ApiResult};
use iam_core::domain::identity::Claims;

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
        
        Ok(AuthenticatedUser(claims))
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AuthResponse {
    pub access_token: String,
    pub expires_in: i64,
    pub user: UserResponse,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Default)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub wallet_address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegistrationRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegistrationResponse {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize, ToSchema, serde::Serialize)]
pub struct VerifyEmailRequest {
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VerifyEmailResponse {
    pub success: bool,
    pub message: String,
    pub wallet_address: Option<String>,
    pub auth: Option<AuthResponse>,
}

impl VerifyEmailResponse {
    pub fn simple(success: bool, message: &str) -> Self {
        Self {
            success,
            message: message.to_string(),
            wallet_address: None,
            auth: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
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
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LinkWalletRequest {
    pub wallet_address: String,
    pub label: Option<String>,
    pub is_primary: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LinkWalletResponse {
    pub wallet: UserWallet,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub enum UserType {
    Prosumer = 0,
    Consumer = 1,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct OnChainOnboardingRequest {
    pub user_type: UserType,
    pub lat_e7: i32,
    pub long_e7: i32,
    pub h3_index: Option<u64>,
    pub shard_id: Option<u8>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OnChainOnboardingResponse {
    pub success: bool,
    pub message: String,
    pub transaction_signature: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WalletListResponse {
    pub wallets: Vec<UserWallet>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetPrimaryWalletRequest {
    // body intentionally empty — wallet_id comes from path
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeleteWalletResponse {
    pub message: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ForgotPasswordResponse {
    pub message: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ResetPasswordResponse {
    pub message: String,
}