//! Tests for the `IdentityGrpcService` ConnectRPC implementation.
//!
//! Coverage is split in two:
//! - **RBAC gate** — every RPC must fail closed with `PermissionDenied` when the
//!   caller carries no recognised `x-gridtokenx-role` header.
//! - **Business paths that need no DB** — the JWT-only RPCs (`VerifyToken`,
//!   `Authorize`, `GetUserInfo`) and the pre-`AuthService` UUID validation in the
//!   wallet RPCs are exercised end to end. Paths that touch `AuthService` repos
//!   are covered by `iam-logic`'s `auth_service_tests`.

use std::sync::Arc;

use buffa::view::OwnedView;
use connectrpc::{Context, ErrorCode};
use http::HeaderMap;
use uuid::Uuid;

use iam_core::config::Config;
use iam_core::domain::identity::Claims;
use iam_core::traits::{
    MockApiKeyRepositoryTrait, MockBlockchainTrait, MockCacheTrait, MockEventBusTrait,
    MockUserRepositoryTrait, MockWalletRepositoryTrait,
};
use iam_logic::jwt_service::ApiKeyService;
use iam_logic::{AuthService, JwtService};

use gridtokenx_blockchain_core::rpc::metrics::NoopMetrics;
use gridtokenx_blockchain_core::rpc::transaction::{MockChainBridgeProvider, TransactionHandler};

use crate::identity_grpc::IdentityGrpcService;
use iam_protocol::identity;
use identity::IdentityService;
use identity::{
    ApiKeyRequest, ApiKeyRequestView, AuthorizeRequest, AuthorizeRequestView,
    GetUserWalletRequest, GetUserWalletRequestView, InitializeWalletRequest,
    InitializeWalletRequestView, LinkWalletRequest, LinkWalletRequestView, RegisterUserRequest,
    RegisterUserRequestView, TokenRequest, TokenRequestView,
};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn mock_wallet_service() -> Arc<gridtokenx_blockchain_core::WalletService> {
    let provider = Arc::new(MockChainBridgeProvider::default());
    let metrics = Arc::new(NoopMetrics);
    let handler = TransactionHandler::new(provider, metrics);
    Arc::new(gridtokenx_blockchain_core::WalletService::new(handler))
}

fn mock_config() -> Arc<Config> {
    Arc::new(Config {
        environment: "test".to_string(),
        port: 4010,
        database_url: "postgres://localhost/test".to_string(),
        redis_url: "redis://localhost/test".to_string(),
        jwt_secret: "test-secret-12345678901234567890".to_string(),
        jwt_expiration: 3600,
        encryption_secret: "test-encryption-secret".to_string(),
        api_key_secret: "test-api-key-secret".to_string(),
        log_level: "info".to_string(),
        test_mode: true,
        solana_rpc_url: "http://localhost:8899".to_string(),
        chain_bridge_url: "http://localhost:5040".to_string(),
        solana_cluster: "localnet".to_string(),
        master_secret: "master-secret".to_string(),
        kafka_brokers: None,
        rabbitmq_url: None,
        smtp_host: "localhost".to_string(),
        smtp_port: 1025,
        smtp_from: "noreply@test.com".to_string(),
        app_base_url: "http://localhost:3000".to_string(),
        registry_program_id: "HZR6b8GhzhDowyL6dX58qBjdSDNtFyJHU5dPF3kXDcTS".to_string(),
        oracle_program_id: "AiWcoPDEk3G4iKrDXj1wCN1ffWxQDEsgtJZKcjauoFJr".to_string(),
        governance_program_id: "6FsfuFEg8LHjSiejc8om8Q6iSaAgfEWHCgz78PT8jocw".to_string(),
        energy_token_program_id: "GjSjmPt8VSHr49ti4BijWZSu7rwb8o32pod7gNBnTY4U".to_string(),
        trading_program_id: "DXxHdUar3pUUKRnt4XAMA8rdYRpAsNY1xk3Zo4crShvY".to_string(),
        auth_cpu_semaphore_limit: 32,
        grpc_port: Some(4020),
        tokio_worker_threads: Some(4),
        database_max_connections: 50,
        database_min_connections: 5,
        request_timeout_secs: 30,
        global_concurrency_limit: 100,
        energy_token_mint: "GpGDVgksF2ivMv3XXR4VZDXRmW9G6agA2AGkKUBQRzk6".to_string(),
        currency_token_mint: "8BGFtQLRaY9Nh5BGUwjJvdeXEsscCgJAi5zTgALk1Vg5".to_string(),
    })
}

/// Build a service backed entirely by no-expectation mocks. Returns the service
/// plus a clone of its `JwtService` so tests can mint matching tokens.
fn make_service() -> (IdentityGrpcService, JwtService) {
    let config = mock_config();
    let jwt = JwtService::new(&config.jwt_secret).unwrap();
    let api_key = ApiKeyService::new(config.api_key_secret.clone()).unwrap();

    let auth = AuthService::new(
        Arc::new(MockUserRepositoryTrait::new()),
        Arc::new(MockWalletRepositoryTrait::new()),
        Arc::new(MockApiKeyRepositoryTrait::new()),
        config.clone(),
        jwt.clone(),
        api_key,
        Arc::new(MockCacheTrait::new()),
        Arc::new(MockEventBusTrait::new()),
        Arc::new(MockBlockchainTrait::new()),
        mock_wallet_service(),
    );

    let svc = IdentityGrpcService::new(auth, jwt.clone());
    (svc, jwt)
}

fn ctx_no_role() -> Context {
    Context::new(HeaderMap::new())
}

/// `admin` passes every per-method allowlist and needs no gateway secret.
fn ctx_admin() -> Context {
    let mut h = HeaderMap::new();
    h.insert("x-gridtokenx-role", "admin".parse().unwrap());
    Context::new(h)
}

fn token_req(tok: &str) -> OwnedView<TokenRequestView<'static>> {
    OwnedView::from_owned(&TokenRequest {
        token: tok.to_string(),
        ..Default::default()
    })
    .unwrap()
}

fn authorize_req(tok: &str, perm: &str) -> OwnedView<AuthorizeRequestView<'static>> {
    OwnedView::from_owned(&AuthorizeRequest {
        token: tok.to_string(),
        required_permission: perm.to_string(),
        ..Default::default()
    })
    .unwrap()
}

fn api_key_req(key: &str) -> OwnedView<ApiKeyRequestView<'static>> {
    OwnedView::from_owned(&ApiKeyRequest {
        key: key.to_string(),
        ..Default::default()
    })
    .unwrap()
}

fn register_req() -> OwnedView<RegisterUserRequestView<'static>> {
    OwnedView::from_owned(&RegisterUserRequest {
        username: "alice".to_string(),
        email: "alice@example.com".to_string(),
        password: "GridTokenX-$2024-@SecureAuth".to_string(),
        ..Default::default()
    })
    .unwrap()
}

fn link_wallet_req(user_id: &str) -> OwnedView<LinkWalletRequestView<'static>> {
    OwnedView::from_owned(&LinkWalletRequest {
        user_id: user_id.to_string(),
        wallet_address: "So11111111111111111111111111111111111111112".to_string(),
        ..Default::default()
    })
    .unwrap()
}

fn init_wallet_req(user_id: &str) -> OwnedView<InitializeWalletRequestView<'static>> {
    OwnedView::from_owned(&InitializeWalletRequest {
        user_id: user_id.to_string(),
        wallet_address: "So11111111111111111111111111111111111111112".to_string(),
        ..Default::default()
    })
    .unwrap()
}

fn get_user_wallet_req(user_id: &str) -> OwnedView<GetUserWalletRequestView<'static>> {
    OwnedView::from_owned(&GetUserWalletRequest {
        user_id: user_id.to_string(),
        ..Default::default()
    })
    .unwrap()
}

fn admin_token(jwt: &JwtService) -> (Uuid, String) {
    let id = Uuid::new_v4();
    let tok = jwt
        .encode_token(&Claims::new(id, "alice".to_string(), "admin".to_string()))
        .unwrap();
    (id, tok)
}

fn user_token(jwt: &JwtService) -> String {
    jwt.encode_token(&Claims::new(
        Uuid::new_v4(),
        "bob".to_string(),
        "user".to_string(),
    ))
    .unwrap()
}

// ---------------------------------------------------------------------------
// RBAC gate — every RPC fails closed without a recognised role.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn verify_token_denied_without_role() {
    let (svc, _) = make_service();
    let err = svc
        .verify_token(ctx_no_role(), token_req("x"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PermissionDenied);
}

#[tokio::test]
async fn authorize_denied_without_role() {
    let (svc, _) = make_service();
    let err = svc
        .authorize(ctx_no_role(), authorize_req("x", "read:foo"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PermissionDenied);
}

#[tokio::test]
async fn get_user_info_denied_without_role() {
    let (svc, _) = make_service();
    let err = svc
        .get_user_info(ctx_no_role(), token_req("x"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PermissionDenied);
}

#[tokio::test]
async fn verify_api_key_denied_without_role() {
    let (svc, _) = make_service();
    let err = svc
        .verify_api_key(ctx_no_role(), api_key_req("x"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PermissionDenied);
}

#[tokio::test]
async fn register_user_denied_without_role() {
    let (svc, _) = make_service();
    let err = svc
        .register_user(ctx_no_role(), register_req())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PermissionDenied);
}

#[tokio::test]
async fn link_wallet_denied_without_role() {
    let (svc, _) = make_service();
    let err = svc
        .link_wallet(ctx_no_role(), link_wallet_req(&Uuid::new_v4().to_string()))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PermissionDenied);
}

#[tokio::test]
async fn initialize_user_wallet_denied_without_role() {
    let (svc, _) = make_service();
    let err = svc
        .initialize_user_wallet(ctx_no_role(), init_wallet_req(&Uuid::new_v4().to_string()))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PermissionDenied);
}

#[tokio::test]
async fn get_user_wallet_denied_without_role() {
    let (svc, _) = make_service();
    let err = svc
        .get_user_wallet(ctx_no_role(), get_user_wallet_req(&Uuid::new_v4().to_string()))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PermissionDenied);
}

// ---------------------------------------------------------------------------
// VerifyToken — JWT only.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn verify_token_valid() {
    let (svc, jwt) = make_service();
    let (id, tok) = admin_token(&jwt);
    let (resp, _) = svc.verify_token(ctx_admin(), token_req(&tok)).await.unwrap();
    assert!(resp.valid);
    assert_eq!(resp.user_id, id.to_string());
    assert_eq!(resp.username, "alice");
    assert_eq!(resp.role, "admin");
    assert!(resp.error_message.is_empty());
}

#[tokio::test]
async fn verify_token_invalid_is_ok_but_not_valid() {
    let (svc, _) = make_service();
    // Garbage token → handler returns Ok with valid=false (not an error).
    let (resp, _) = svc
        .verify_token(ctx_admin(), token_req("not.a.jwt"))
        .await
        .unwrap();
    assert!(!resp.valid);
    assert!(!resp.error_message.is_empty());
}

// ---------------------------------------------------------------------------
// Authorize — role-based decision over decoded claims.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn authorize_admin_allows_admin_permission() {
    let (svc, jwt) = make_service();
    let (_, tok) = admin_token(&jwt);
    let (resp, _) = svc
        .authorize(ctx_admin(), authorize_req(&tok, "admin:delete"))
        .await
        .unwrap();
    assert!(resp.authorized);
}

#[tokio::test]
async fn authorize_user_denied_admin_permission() {
    let (svc, jwt) = make_service();
    let tok = user_token(&jwt);
    let (resp, _) = svc
        .authorize(ctx_admin(), authorize_req(&tok, "admin:delete"))
        .await
        .unwrap();
    assert!(!resp.authorized);
    assert!(!resp.error_message.is_empty());
}

#[tokio::test]
async fn authorize_user_allowed_non_admin_permission() {
    let (svc, jwt) = make_service();
    let tok = user_token(&jwt);
    let (resp, _) = svc
        .authorize(ctx_admin(), authorize_req(&tok, "read:profile"))
        .await
        .unwrap();
    assert!(resp.authorized);
}

#[tokio::test]
async fn authorize_invalid_token_not_authorized() {
    let (svc, _) = make_service();
    let (resp, _) = svc
        .authorize(ctx_admin(), authorize_req("bad", "read:profile"))
        .await
        .unwrap();
    assert!(!resp.authorized);
    assert!(!resp.error_message.is_empty());
}

// ---------------------------------------------------------------------------
// GetUserInfo — JWT only.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_user_info_valid() {
    let (svc, jwt) = make_service();
    let (id, tok) = admin_token(&jwt);
    let (resp, _) = svc
        .get_user_info(ctx_admin(), token_req(&tok))
        .await
        .unwrap();
    assert_eq!(resp.id, id.to_string());
    assert_eq!(resp.username, "alice");
    assert_eq!(resp.role, "admin");
}

#[tokio::test]
async fn get_user_info_invalid_token_unauthenticated() {
    let (svc, _) = make_service();
    let err = svc
        .get_user_info(ctx_admin(), token_req("bad"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Unauthenticated);
}

// ---------------------------------------------------------------------------
// Wallet RPCs — UUID validation happens before any AuthService call.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn link_wallet_invalid_uuid_rejected() {
    let (svc, _) = make_service();
    let err = svc
        .link_wallet(ctx_admin(), link_wallet_req("not-a-uuid"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidArgument);
}

#[tokio::test]
async fn initialize_user_wallet_invalid_uuid_rejected() {
    let (svc, _) = make_service();
    let err = svc
        .initialize_user_wallet(ctx_admin(), init_wallet_req("not-a-uuid"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidArgument);
}

#[tokio::test]
async fn get_user_wallet_invalid_uuid_rejected() {
    let (svc, _) = make_service();
    let err = svc
        .get_user_wallet(ctx_admin(), get_user_wallet_req("not-a-uuid"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidArgument);
}
