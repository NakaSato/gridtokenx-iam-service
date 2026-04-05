use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
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
}

#[derive(Debug, Deserialize, ToSchema)]
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
