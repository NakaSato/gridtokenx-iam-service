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

// ---------------------------------------------------------------------------
// RBAC allowlist drift guard.
//
// These assert the *wired* per-method allowlists (the `*_ROLES` consts the
// handlers gate on) against the documented spec. A change to any handler's
// allowlist — widening or narrowing — fails here until the spec is updated
// deliberately. This is the static counterpart to the runtime matrix below.
// ---------------------------------------------------------------------------

use crate::identity_grpc::{
    AUTHORIZE_ROLES, GET_USER_INFO_ROLES, GET_USER_WALLET_ROLES, INITIALIZE_USER_WALLET_ROLES,
    LINK_WALLET_ROLES, REGISTER_USER_ROLES, VERIFY_API_KEY_ROLES, VERIFY_TOKEN_ROLES,
};
use gridtokenx_blockchain_core::auth::ServiceRole;

/// Compare an allowlist against the spec regardless of declaration order.
fn assert_roles(actual: &[ServiceRole], expected: &[ServiceRole]) {
    let mut a = actual.to_vec();
    let mut e = expected.to_vec();
    a.sort_by_key(|r| format!("{r}"));
    e.sort_by_key(|r| format!("{r}"));
    assert_eq!(a, e, "allowlist drift: {actual:?} != spec {expected:?}");
}

#[test]
fn allowlist_verify_token_matches_spec() {
    assert_roles(
        VERIFY_TOKEN_ROLES,
        &[
            ServiceRole::ApiGateway,
            ServiceRole::TradingApi,
            ServiceRole::AggregatorBridge,
            ServiceRole::MeterService,
            ServiceRole::Admin,
        ],
    );
}

#[test]
fn allowlist_verify_api_key_matches_spec() {
    assert_roles(
        VERIFY_API_KEY_ROLES,
        &[
            ServiceRole::ApiGateway,
            ServiceRole::AggregatorBridge,
            ServiceRole::Admin,
        ],
    );
}

#[test]
fn allowlist_get_user_wallet_matches_spec() {
    assert_roles(
        GET_USER_WALLET_ROLES,
        &[
            ServiceRole::AggregatorBridge,
            ServiceRole::ApiGateway,
            ServiceRole::Admin,
        ],
    );
}

#[test]
fn allowlist_gateway_only_methods_match_spec() {
    // Authorize, GetUserInfo, RegisterUser, LinkWallet, InitializeUserWallet
    // are all gateway-only writes/reads: exactly {ApiGateway, Admin}.
    let gateway_only = &[ServiceRole::ApiGateway, ServiceRole::Admin];
    for list in [
        AUTHORIZE_ROLES,
        GET_USER_INFO_ROLES,
        REGISTER_USER_ROLES,
        LINK_WALLET_ROLES,
        INITIALIZE_USER_WALLET_ROLES,
    ] {
        assert_roles(list, gateway_only);
    }
}

#[test]
fn allowlist_invariants_hold() {
    let all = [
        VERIFY_TOKEN_ROLES,
        AUTHORIZE_ROLES,
        GET_USER_INFO_ROLES,
        VERIFY_API_KEY_ROLES,
        REGISTER_USER_ROLES,
        LINK_WALLET_ROLES,
        GET_USER_WALLET_ROLES,
        INITIALIZE_USER_WALLET_ROLES,
    ];
    for list in all {
        // Admin and ApiGateway are present everywhere.
        assert!(list.contains(&ServiceRole::Admin), "Admin missing from {list:?}");
        assert!(
            list.contains(&ServiceRole::ApiGateway),
            "ApiGateway missing from {list:?}"
        );
        // No allowlist ever admits these — they have no IAM-read business.
        assert!(
            !list.contains(&ServiceRole::Unknown),
            "Unknown must never be allowlisted: {list:?}"
        );
        assert!(
            !list.contains(&ServiceRole::IamService),
            "IamService must not call its own RPCs: {list:?}"
        );
        assert!(
            !list.contains(&ServiceRole::SettlementService),
            "SettlementService never allowlisted: {list:?}"
        );
    }

    // TradingApi and MeterService are *only* trusted for the broad VerifyToken read.
    for list in [
        AUTHORIZE_ROLES,
        GET_USER_INFO_ROLES,
        VERIFY_API_KEY_ROLES,
        REGISTER_USER_ROLES,
        LINK_WALLET_ROLES,
        GET_USER_WALLET_ROLES,
        INITIALIZE_USER_WALLET_ROLES,
    ] {
        assert!(
            !list.contains(&ServiceRole::TradingApi),
            "TradingApi only allowed on VerifyToken: {list:?}"
        );
        assert!(
            !list.contains(&ServiceRole::MeterService),
            "MeterService only allowed on VerifyToken: {list:?}"
        );
    }
    assert!(VERIFY_TOKEN_ROLES.contains(&ServiceRole::TradingApi));
    assert!(VERIFY_TOKEN_ROLES.contains(&ServiceRole::MeterService));

    // AggregatorBridge is trusted for VerifyToken, VerifyApiKey, GetUserWallet only.
    for list in [
        AUTHORIZE_ROLES,
        GET_USER_INFO_ROLES,
        REGISTER_USER_ROLES,
        LINK_WALLET_ROLES,
        INITIALIZE_USER_WALLET_ROLES,
    ] {
        assert!(
            !list.contains(&ServiceRole::AggregatorBridge),
            "AggregatorBridge must not write/authorize: {list:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// ApiError -> ConnectError mapping.
//
// `register_user`, `link_wallet`, `get_user_wallet`, `initialize_user_wallet`
// route every `AuthService` error through `map_api_error`, which reuses
// `ApiError::status_code()` (the same classification the REST layer applies)
// instead of collapsing everything to one code. A client-side fault (bad
// input, duplicate resource, not-found) must stay distinguishable from an
// infra fault (DB/Redis/blockchain down) so gateway retry logic and 5xx
// alarms aren't misled.
// ---------------------------------------------------------------------------

use crate::identity_grpc::map_api_error;
use iam_core::error::ApiError;

#[test]
fn map_api_error_distinguishes_client_from_infra_faults() {
    let cases = [
        (ApiError::Unauthorized("x".into()), ErrorCode::Unauthenticated),
        (ApiError::Forbidden("x".into()), ErrorCode::PermissionDenied),
        (ApiError::BadRequest("x".into()), ErrorCode::InvalidArgument),
        (ApiError::Validation("x".into()), ErrorCode::InvalidArgument),
        (ApiError::NotFound("x".into()), ErrorCode::NotFound),
        (
            ApiError::Conflict("Username or email already exists".into()),
            ErrorCode::AlreadyExists,
        ),
        (ApiError::Blockchain("x".into()), ErrorCode::Unavailable),
        (ApiError::ExternalService("x".into()), ErrorCode::Unavailable),
        (ApiError::RateLimitExceeded("x".into()), ErrorCode::ResourceExhausted),
        // Infra/unclassified faults stay Internal — never silently downgraded
        // to a client-fault code that would suppress alarms or retries.
        (ApiError::Internal("x".into()), ErrorCode::Internal),
        (ApiError::Redis("x".into()), ErrorCode::Internal),
    ];
    for (err, expected) in cases {
        let mapped = map_api_error(&err).code;
        assert_eq!(mapped, expected, "{err:?} mapped to {mapped:?}, expected {expected:?}");
    }
}

// ---------------------------------------------------------------------------
// RBAC matrix — runtime gate behaviour for specific roles.
//
// `from_headers` maps any non-ApiGateway role straight from the header (no
// secret needed), so these use AggregatorBridge / TradingApi / MeterService /
// SettlementService directly. ApiGateway's secret handshake is covered by
// `auth.rs::test_gateway_secret_fail_closed`.
// ---------------------------------------------------------------------------

/// Build a `Context` carrying `role` (kebab-case, via `Display`), no secret.
fn ctx_role(role: ServiceRole) -> Context {
    let mut h = HeaderMap::new();
    h.insert("x-gridtokenx-role", role.to_string().parse().unwrap());
    Context::new(h)
}

/// `SettlementService` is in no allowlist — it must be denied by every RPC.
#[tokio::test]
async fn settlement_service_denied_on_every_rpc() {
    let (svc, _) = make_service();
    let c = || ctx_role(ServiceRole::SettlementService);
    let uid = Uuid::new_v4().to_string();

    assert_eq!(
        svc.verify_token(c(), token_req("x")).await.unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        svc.authorize(c(), authorize_req("x", "read:foo")).await.unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        svc.get_user_info(c(), token_req("x")).await.unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        svc.verify_api_key(c(), api_key_req("x")).await.unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        svc.register_user(c(), register_req()).await.unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        svc.link_wallet(c(), link_wallet_req(&uid)).await.unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        svc.get_user_wallet(c(), get_user_wallet_req(&uid)).await.unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        svc.initialize_user_wallet(c(), init_wallet_req(&uid)).await.unwrap_err().code,
        ErrorCode::PermissionDenied
    );
}

/// `TradingApi` may call `VerifyToken` (gate passes → garbage token → Ok/invalid),
/// but nothing else (`Authorize` here) — that must be `PermissionDenied`.
#[tokio::test]
async fn trading_api_allowed_only_on_verify_token() {
    let (svc, _) = make_service();
    // Allowed: gate passes, handler runs (invalid token → Ok, not a gate error).
    let (resp, _) = svc
        .verify_token(ctx_role(ServiceRole::TradingApi), token_req("garbage"))
        .await
        .unwrap();
    assert!(!resp.valid);
    // Denied elsewhere.
    assert_eq!(
        svc.authorize(ctx_role(ServiceRole::TradingApi), authorize_req("x", "read:foo"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
}

/// `MeterService` may call `VerifyToken` only.
#[tokio::test]
async fn meter_service_allowed_only_on_verify_token() {
    let (svc, _) = make_service();
    let (resp, _) = svc
        .verify_token(ctx_role(ServiceRole::MeterService), token_req("garbage"))
        .await
        .unwrap();
    assert!(!resp.valid);
    assert_eq!(
        svc.verify_api_key(ctx_role(ServiceRole::MeterService), api_key_req("x"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
}

/// `AggregatorBridge` is allowed on `GetUserWallet`: the gate passes, so the
/// request reaches UUID validation and fails there with `InvalidArgument`
/// (NOT `PermissionDenied`) — proving the role cleared RBAC.
#[tokio::test]
async fn aggregator_bridge_allowed_on_get_user_wallet() {
    let (svc, _) = make_service();
    let err = svc
        .get_user_wallet(
            ctx_role(ServiceRole::AggregatorBridge),
            get_user_wallet_req("not-a-uuid"),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidArgument);
    // And denied where it has no business.
    assert_eq!(
        svc.register_user(ctx_role(ServiceRole::AggregatorBridge), register_req())
            .await
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
}

// ---------------------------------------------------------------------------
// DB integration — RBAC gate + real Postgres-backed VerifyApiKey, end to end.
//
// `#[sqlx::test]` provisions a throwaway database, runs the platform
// migrations, and hands us a pool. We seed a real api-key row, then drive
// `verify_api_key` through the gate AND the real `ApiKeyRepository`. Only the
// cache / event-bus / blockchain edges stay mocked so the test isolates
// "allowed role + valid DB key ⇒ valid" from "disallowed role ⇒ denied even
// when the key is valid". Requires a live Postgres (DATABASE_URL); not part of
// the infra-free unit run.
// ---------------------------------------------------------------------------

use iam_persistence::ApiKeyRepository;
use sqlx::PgPool;

#[sqlx::test(migrations = "../../migrations")]
async fn verify_api_key_db_gate_and_lookup(pool: PgPool) {
    let config = mock_config();
    let jwt = JwtService::new(&config.jwt_secret).unwrap();
    let api_key_svc = ApiKeyService::new(config.api_key_secret.clone()).unwrap();

    // Seed a real, active key — hashed exactly as the service will hash the
    // raw value on the request path.
    let raw_key = "gtx_rbac_integration_test_key";
    let key_hash = api_key_svc.hash_key(raw_key).unwrap();
    let key_id = Uuid::new_v4();
    sqlx::query("INSERT INTO api_keys (id, name, key_hash, role, is_active) VALUES ($1, $2, $3, $4, true)")
        .bind(key_id)
        .bind("RBAC Integration Key")
        .bind(&key_hash)
        .bind("aggregator-bridge")
        .execute(&pool)
        .await
        .expect("seed api key");

    // Real repo; cache always misses (forces the DB path) and accepts writes.
    let mut cache = MockCacheTrait::new();
    cache
        .expect_get_value()
        .returning(|_| Box::pin(async { Ok(None) }));
    cache
        .expect_set_value()
        .returning(|_, _, _| Box::pin(async { Ok(()) }));

    // Cache-miss verification publishes an `ApiKeyVerified` event.
    let mut event_bus = MockEventBusTrait::new();
    event_bus
        .expect_publish()
        .returning(|_| Box::pin(async { Ok(()) }));

    let auth = AuthService::new(
        Arc::new(MockUserRepositoryTrait::new()),
        Arc::new(MockWalletRepositoryTrait::new()),
        Arc::new(ApiKeyRepository::new(pool)),
        config.clone(),
        jwt.clone(),
        api_key_svc,
        Arc::new(cache),
        Arc::new(event_bus),
        Arc::new(MockBlockchainTrait::new()),
        mock_wallet_service(),
    );
    let svc = IdentityGrpcService::new(auth, jwt);

    // Allowed role + valid key ⇒ verified, role echoed from the DB row.
    let (resp, _) = svc
        .verify_api_key(ctx_role(ServiceRole::AggregatorBridge), api_key_req(raw_key))
        .await
        .expect("verify_api_key should not transport-error");
    assert!(resp.valid, "valid seeded key must verify");
    assert_eq!(resp.role, "aggregator-bridge");

    // Disallowed role ⇒ denied at the gate BEFORE any DB lookup, even though
    // the same key is valid.
    let err = svc
        .verify_api_key(ctx_role(ServiceRole::MeterService), api_key_req(raw_key))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PermissionDenied);

    // Allowed role + unknown key ⇒ gate passes, DB miss ⇒ Ok(valid=false).
    let (miss, _) = svc
        .verify_api_key(ctx_role(ServiceRole::AggregatorBridge), api_key_req("nope"))
        .await
        .expect("unknown key is a denied key, not a transport error");
    assert!(!miss.valid);
}
