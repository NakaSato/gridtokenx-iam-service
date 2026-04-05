use connectrpc::{Context, ConnectError};
use buffa::view::OwnedView;
use tracing::{info, warn};

use crate::domain::identity::JwtService;
use crate::services::AuthService;

// Generated code from proto
#[allow(clippy::module_inception)]
pub mod identity {
    include!(concat!(env!("OUT_DIR"), "/_identity_include.rs"));
    pub use identity::*;
}

use identity::{IdentityService, TokenRequestView, AuthorizeRequestView, ApiKeyRequestView};
use identity::{ApiKeyResponse, AuthorizeResponse, ClaimsResponse, UserInfoResponse};

pub struct IdentityGrpcService {
    auth_service: AuthService,
    jwt_service: JwtService,
}

impl IdentityGrpcService {
    pub fn new(auth_service: AuthService, jwt_service: JwtService) -> Self {
        Self {
            auth_service,
            jwt_service,
        }
    }
}


impl IdentityService for IdentityGrpcService {
    async fn verify_token(
        &self,
        ctx: Context,
        request: OwnedView<TokenRequestView<'static>>,
    ) -> Result<(ClaimsResponse, Context), ConnectError> {
        info!("🔐 gRPC: VerifyToken request");
        let token = request.token; // Zero-copy &str

        match self.jwt_service.decode_token(token) {
            Ok(claims) => Ok((ClaimsResponse {
                valid: true,
                user_id: claims.sub.to_string(),
                username: claims.username,
                role: claims.role,
                error_message: String::default(),
                ..Default::default()
            }, ctx)),
            Err(e) => {
                warn!("❌ gRPC: Token verification failed: {}", e);
                Ok((ClaimsResponse {
                    valid: false,
                    user_id: String::default(),
                    username: String::default(),
                    role: String::default(),
                    error_message: e.to_string(),
                    ..Default::default()
                }, ctx))
            }
        }
    }

    async fn authorize(
        &self,
        ctx: Context,
        request: OwnedView<AuthorizeRequestView<'static>>,
    ) -> Result<(AuthorizeResponse, Context), ConnectError> {
        info!(
            "🔐 gRPC: Authorize request for permission: {}",
            request.required_permission
        );

        match self.jwt_service.decode_token(request.token) {
            Ok(claims) => {
                let authorized = match claims.role.as_str() {
                    "admin" => true,
                    "user" => !request.required_permission.starts_with("admin:"),
                    _ => false,
                };

                Ok((AuthorizeResponse {
                    authorized,
                    error_message: if authorized {
                        String::default()
                    } else {
                        "Insufficient permissions".to_string()
                    },
                    ..Default::default()
                }, ctx))
            }
            Err(e) => Ok((AuthorizeResponse {
                authorized: false,
                error_message: format!("Invalid token: {}", e),
                ..Default::default()
            }, ctx)),
        }
    }

    async fn get_user_info(
        &self,
        ctx: Context,
        request: OwnedView<TokenRequestView<'static>>,
    ) -> Result<(UserInfoResponse, Context), ConnectError> {
        info!("🔐 gRPC: GetUserInfo request");

        match self.jwt_service.decode_token(request.token) {
            Ok(claims) => Ok((UserInfoResponse {
                id: claims.sub.to_string(),
                username: claims.username,
                email: String::default(),
                role: claims.role,
                first_name: String::default(),
                last_name: String::default(),
                wallet_address: String::default(),
                ..Default::default()
            }, ctx)),
            Err(e) => Err(ConnectError::unauthenticated(e.to_string())),
        }
    }

    async fn verify_api_key(
        &self,
        ctx: Context,
        request: OwnedView<ApiKeyRequestView<'static>>,
    ) -> Result<(ApiKeyResponse, Context), ConnectError> {
        info!("🔐 gRPC: VerifyApiKey request");

        match self.auth_service.verify_api_key(request.key).await {
            Ok(api_key) => Ok((ApiKeyResponse {
                valid: true,
                role: api_key.role,
                error_message: String::default(),
                ..Default::default()
            }, ctx)),
            Err(e) => {
                warn!("❌ gRPC: API Key verification failed: {}", e);
                Ok((ApiKeyResponse {
                    valid: false,
                    role: String::default(),
                    error_message: e.to_string(),
                    ..Default::default()
                }, ctx))
            }
        }
    }
}
