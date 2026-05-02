use axum::{
    Json,
    extract::{State, Path},
};
use tracing::instrument;
use uuid::Uuid;

use gridtokenx_blockchain_core::auth::ServiceRole;
use iam_core::error::{ApiError, Result as ApiResult};
use iam_logic::AuthService;
use crate::handlers::types::{
    OnChainOnboardingRequest, OnChainOnboardingResponse,
    LinkWalletRequest, LinkWalletResponse, WalletListResponse, DeleteWalletResponse,
    UserWallet, UserType, AuthenticatedUser,
};

fn map_user_wallet(w: iam_core::domain::identity::UserWallet) -> UserWallet {
    UserWallet {
        id: w.id,
        user_id: Some(w.user_id),
        wallet_address: w.wallet_address,
        label: w.label,
        is_primary: w.is_primary,
        status: if w.verified { "verified" } else { "unverified" }.to_string(),
        created_at: w.created_at,
        verified: w.verified,
        blockchain_registered: w.blockchain_registered,
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/users/me/onchain-profile",
    request_body = OnChainOnboardingRequest,
    responses(
        (status = 200, description = "Onboarding successful", body = OnChainOnboardingResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    tag = "identity",
    security(
        ("jwt" = [])
    )
)]
#[instrument(skip(auth_service, auth))]
pub async fn onboard_user(
    role: ServiceRole,
    auth: AuthenticatedUser,
    State(auth_service): State<AuthService>,
    Json(request): Json<OnChainOnboardingRequest>,
) -> ApiResult<Json<OnChainOnboardingResponse>> {
    let claims = auth.0;
    role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin])
        .map_err(|(_code, msg)| ApiError::Unauthorized(msg.to_string()))?;

    let user_type = match request.user_type {
        UserType::Prosumer => iam_core::domain::identity::UserType::Prosumer,
        UserType::Consumer => iam_core::domain::identity::UserType::Consumer,
    };

    let response = auth_service.onboard_user_on_chain(
        claims.sub,
        user_type,
        request.location.lat_e7,
        request.location.long_e7,
        request.h3_index,
        request.shard_id,
    ).await?;

    Ok(Json(OnChainOnboardingResponse {
        status: if response.success { "processing".to_string() } else { "failed".to_string() },
        transaction_signature: response.transaction_signature,
        message: response.message,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/users/me/wallets",
    request_body = LinkWalletRequest,
    responses(
        (status = 200, description = "Wallet linked successfully", body = LinkWalletResponse),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Wallet already linked"),
        (status = 500, description = "Internal server error")
    ),
    tag = "identity",
    security(
        ("jwt" = [])
    )
)]
#[instrument(skip(auth_service, auth))]
pub async fn link_wallet(
    role: ServiceRole,
    auth: AuthenticatedUser,
    State(auth_service): State<AuthService>,
    Json(request): Json<LinkWalletRequest>,
) -> ApiResult<Json<LinkWalletResponse>> {
    let claims = auth.0;
    role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin])
        .map_err(|(_code, msg)| ApiError::Unauthorized(msg.to_string()))?;

    let w = auth_service.link_wallet(
        claims.sub,
        request.wallet_address,
        request.label,
        request.is_primary,
    ).await?;

    Ok(Json(map_user_wallet(w)))
}

#[utoipa::path(
    get,
    path = "/api/v1/users/me/wallets",
    responses(
        (status = 200, description = "List of wallets", body = WalletListResponse),
        (status = 401, description = "Unauthorized"),
    ),
    tag = "identity",
    security(("jwt" = []))
)]
#[instrument(skip(auth_service, auth))]
pub async fn list_wallets(
    role: ServiceRole,
    auth: AuthenticatedUser,
    State(auth_service): State<AuthService>,
) -> ApiResult<Json<WalletListResponse>> {
    let claims = auth.0;
    role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin])
        .map_err(|(_code, msg)| ApiError::Unauthorized(msg.to_string()))?;
    
    let wallets = auth_service.list_wallets(claims.sub).await?;
    
    Ok(Json(WalletListResponse {
        wallets: wallets.into_iter().map(map_user_wallet).collect(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/users/me/wallets/{wallet_id}",
    params(("wallet_id" = Uuid, Path, description = "Wallet ID")),
    responses(
        (status = 200, description = "Wallet details", body = UserWallet),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not found"),
    ),
    tag = "identity",
    security(("jwt" = []))
)]
#[instrument(skip(auth_service, auth))]
pub async fn get_wallet(
    role: ServiceRole,
    auth: AuthenticatedUser,
    State(auth_service): State<AuthService>,
    Path(wallet_id): Path<Uuid>,
) -> ApiResult<Json<UserWallet>> {
    let claims = auth.0;
    role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin])
        .map_err(|(_code, msg)| ApiError::Unauthorized(msg.to_string()))?;
    
    let w = auth_service.get_wallet(claims.sub, wallet_id).await?;
    
    Ok(Json(map_user_wallet(w)))
}

#[utoipa::path(
    put,
    path = "/api/v1/users/me/wallets/{wallet_id}/primary",
    params(("wallet_id" = Uuid, Path, description = "Wallet ID")),
    responses(
        (status = 200, description = "Primary wallet updated", body = UserWallet),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not found"),
    ),
    tag = "identity",
    security(("jwt" = []))
)]
#[instrument(skip(auth_service, auth))]
pub async fn set_primary_wallet(
    role: ServiceRole,
    auth: AuthenticatedUser,
    State(auth_service): State<AuthService>,
    Path(wallet_id): Path<Uuid>,
) -> ApiResult<Json<UserWallet>> {
    let claims = auth.0;
    role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin])
        .map_err(|(_code, msg)| ApiError::Unauthorized(msg.to_string()))?;
    
    let w = auth_service.set_primary_wallet(claims.sub, wallet_id).await?;
    
    Ok(Json(map_user_wallet(w)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/users/me/wallets/{wallet_id}",
    params(("wallet_id" = Uuid, Path, description = "Wallet ID")),
    responses(
        (status = 200, description = "Wallet unlinked", body = DeleteWalletResponse),
        (status = 400, description = "Cannot delete primary wallet"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not found"),
    ),
    tag = "identity",
    security(("jwt" = []))
)]
#[instrument(skip(auth_service, auth))]
pub async fn unlink_wallet(
    role: ServiceRole,
    auth: AuthenticatedUser,
    State(auth_service): State<AuthService>,
    Path(wallet_id): Path<Uuid>,
) -> ApiResult<Json<DeleteWalletResponse>> {
    let claims = auth.0;
    role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin])
        .map_err(|(_code, msg)| ApiError::Unauthorized(msg.to_string()))?;
    
    auth_service.unlink_wallet(claims.sub, wallet_id).await?;
    Ok(Json(DeleteWalletResponse { message: "Wallet unlinked successfully".to_string() }))
}
