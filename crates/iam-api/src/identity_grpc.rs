use connectrpc::{Context, ConnectError, ErrorCode};
use buffa::view::OwnedView;
use tracing::{info, warn};
use gridtokenx_blockchain_core::auth::ServiceRole;

use iam_logic::JwtService;
use iam_logic::AuthService;

// Generated code from proto
// Integration with generated proto code
pub use iam_protocol::identity;

use identity::{TokenRequestView, AuthorizeRequestView, ApiKeyRequestView};
use identity::{ApiKeyResponse, AuthorizeResponse, ClaimsResponse, UserInfoResponse};

/// gRPC service implementation for the Identity service, using ConnectRPC.
pub struct IdentityGrpcService {
    auth_service: AuthService,
    jwt_service: JwtService,
}

impl IdentityGrpcService {
    /// Creates a new instance of the Identity gRPC service.
    pub fn new(auth_service: AuthService, jwt_service: JwtService) -> Self {
        Self {
            auth_service,
            jwt_service,
        }
    }

    fn extract_role(&self, ctx: &Context) -> ServiceRole {
        ServiceRole::from_headers(&ctx.headers)
    }
}


impl identity::IdentityService for IdentityGrpcService {
    async fn verify_token(
        &self,
        ctx: Context,
        request: OwnedView<TokenRequestView<'static>>,
    ) -> std::result::Result<(ClaimsResponse, Context), ConnectError> {
        let role = self.extract_role(&ctx);
        role.require_any(&[ServiceRole::ApiGateway, ServiceRole::TradingApi, ServiceRole::OracleBridge, ServiceRole::Admin])
            .map_err(|(_, msg)| ConnectError::new(ErrorCode::PermissionDenied, msg))?;

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
                    error_message: format!("{}", e),
                    ..Default::default()
                }, ctx))
            }
        }
    }

    async fn authorize(
        &self,
        ctx: Context,
        request: OwnedView<AuthorizeRequestView<'static>>,
    ) -> std::result::Result<(AuthorizeResponse, Context), ConnectError> {
        let role = self.extract_role(&ctx);
        role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin])
            .map_err(|(_, msg)| ConnectError::new(ErrorCode::PermissionDenied, msg))?;

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
    ) -> std::result::Result<(UserInfoResponse, Context), ConnectError> {
        let role = self.extract_role(&ctx);
        role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin])
            .map_err(|(_, msg)| ConnectError::new(ErrorCode::PermissionDenied, msg))?;

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
            Err(e) => Err(ConnectError::new(ErrorCode::Unauthenticated, format!("{}", e))),
        }
    }

    async fn verify_api_key(
        &self,
        ctx: Context,
        request: OwnedView<ApiKeyRequestView<'static>>,
    ) -> std::result::Result<(ApiKeyResponse, Context), ConnectError> {
        let role = self.extract_role(&ctx);
        role.require_any(&[ServiceRole::ApiGateway, ServiceRole::OracleBridge, ServiceRole::Admin])
            .map_err(|(_, msg)| ConnectError::new(ErrorCode::PermissionDenied, msg))?;

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
                    error_message: format!("{}", e),
                    ..Default::default()
                }, ctx))
            }
        }
    }

    async fn register_user(
        &self,
        ctx: Context,
        request: OwnedView<identity::RegisterUserRequestView<'static>>,
    ) -> std::result::Result<(identity::RegisterUserResponse, Context), ConnectError> {
        let role = self.extract_role(&ctx);
        role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin])
            .map_err(|(_, msg)| ConnectError::new(ErrorCode::PermissionDenied, msg))?;

        info!("📝 gRPC: RegisterUser request for {}", request.username);
        
        match self.auth_service.register(
            request.username.to_string(),
            request.email.to_string(),
            request.password.to_string(),
            (!request.first_name.is_empty()).then(|| request.first_name.to_string()),
            (!request.last_name.is_empty()).then(|| request.last_name.to_string()),
        ).await {
            Ok(res) => Ok((identity::RegisterUserResponse {
                user_id: res.id.to_string(),
                username: res.username,
                email: res.email,
                message: res.message,
                ..Default::default()
            }, ctx)),
            Err(e) => Err(ConnectError::new(ErrorCode::Internal, format!("{}", e))),
        }
    }

    async fn link_wallet(
        &self,
        ctx: Context,
        request: OwnedView<identity::LinkWalletRequestView<'static>>,
    ) -> std::result::Result<(identity::LinkWalletResponse, Context), ConnectError> {
        let role = self.extract_role(&ctx);
        role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin])
            .map_err(|(_, msg)| ConnectError::new(ErrorCode::PermissionDenied, msg))?;

        info!("🔗 gRPC: LinkWallet request for user {}", request.user_id);
        
        let user_id = uuid::Uuid::parse_str(request.user_id)
            .map_err(|e| ConnectError::new(ErrorCode::InvalidArgument, format!("Invalid UUID: {}", e)))?;

        match self.auth_service.link_wallet(
            user_id,
            request.wallet_address.to_string(),
            Some(request.label.to_string()),
            request.is_primary,
        ).await {
            Ok(res) => Ok((identity::LinkWalletResponse {
                wallet_id: res.id.to_string(),
                user_id: res.user_id.to_string(),
                wallet_address: res.wallet_address,
                message: "Wallet linked successfully".to_string(),
                ..Default::default()
            }, ctx)),
            Err(e) => Err(ConnectError::new(ErrorCode::Internal, format!("{}", e))),
        }
    }

    async fn initialize_user_wallet(
        &self,
        ctx: Context,
        request: OwnedView<identity::InitializeWalletRequestView<'static>>,
    ) -> std::result::Result<(identity::InitializeWalletResponse, Context), ConnectError> {
        let role = self.extract_role(&ctx);
        role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin])
            .map_err(|(_, msg)| ConnectError::new(ErrorCode::PermissionDenied, msg))?;

        info!("🌐 gRPC: InitializeUserWallet request for user {}", request.user_id);

        let user_id = uuid::Uuid::parse_str(request.user_id)
            .map_err(|e| ConnectError::new(ErrorCode::InvalidArgument, format!("Invalid UUID: {}", e)))?;

        let result: std::result::Result<String, iam_core::error::ApiError> = self.auth_service.initialize_user_wallet(
            user_id,
            &request.wallet_address,
            request.initial_funding_sol,
        ).await;

        match result {
            Ok(signature) => Ok((identity::InitializeWalletResponse {
                success: true,
                message: "Wallet initialized on-chain".to_string(),
                transaction_signature: signature,
                ..Default::default()
            }, ctx)),
            Err(e) => Err(ConnectError::new(ErrorCode::Internal, format!("{}", e))),
        }
    }
}