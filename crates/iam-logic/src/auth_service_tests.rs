use std::sync::Arc;
use uuid::Uuid;
use iam_core::domain::identity::{User, UserWithHash, Role, EmailVerificationState, ApiKey};
use iam_core::error::ApiError;
use iam_core::traits::{
    MockUserRepositoryTrait, MockWalletRepositoryTrait, MockApiKeyRepositoryTrait,
    MockCacheTrait, MockEventBusTrait, MockBlockchainTrait
};
use iam_core::config::Config;
use crate::auth_service::AuthService;
use crate::jwt_service::{JwtService, ApiKeyService};
use gridtokenx_blockchain_core::rpc::transaction::{TransactionHandler, MockChainBridgeProvider};
use gridtokenx_blockchain_core::rpc::metrics::NoopMetrics;

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

#[tokio::test]
async fn test_login_success() {
    let mut user_repo = MockUserRepositoryTrait::new();
    let wallet_repo = MockWalletRepositoryTrait::new();
    let api_key_repo = MockApiKeyRepositoryTrait::new();
    let mut cache = MockCacheTrait::new();
    let mut event_bus = MockEventBusTrait::new();
    let blockchain_service = MockBlockchainTrait::new();
    
    let config = mock_config();
    let jwt_service = JwtService::new(&config.jwt_secret).unwrap();
    let api_key_service = ApiKeyService::new(config.api_key_secret.clone()).unwrap();
 
    let username = "testuser";
    let password = "GridTokenX-$2024-@SecureAuth";
    let password_hash = crate::password::PasswordService::hash_password(password).unwrap();
    
    let user_id = Uuid::new_v4();
    let user = User {
        id: user_id,
        username: username.to_string(),
        email: "test@example.com".to_string(),
        role: Role::User.to_string(),
        first_name: None,
        last_name: None,
        wallet_address: None,
        is_active: true,
        blockchain_registered: false,
        user_type: None,
        latitude: None,
        longitude: None,
    };
    let user_with_hash = UserWithHash {
        user: user.clone(),
        password_hash,
    };

    // 1. Rate limit check
    let lock_key = iam_core::domain::identity::keys::cache::account_lock(username);
    cache.expect_exists()
        .with(mockall::predicate::eq(lock_key))
        .returning(|_| Box::pin(async move { Ok(false) }));

    // 2. Profile cache check
    let profile_key = iam_core::domain::identity::keys::cache::user_profile(username);
    cache.expect_get_value()
        .with(mockall::predicate::eq(profile_key.clone()))
        .returning(|_| Box::pin(async move { Ok(None) }));

    // 3. DB query
    user_repo.expect_find_by_username_or_email()
        .with(mockall::predicate::eq(username))
        .returning(move |_| {
            let user_with_hash = user_with_hash.clone();
            Box::pin(async move { Ok(Some(user_with_hash)) })
        });

    // 4. Cache profile set
    cache.expect_set_value()
        .with(mockall::predicate::eq(profile_key), mockall::predicate::always(), mockall::predicate::always())
        .returning(|_, _, _| Box::pin(async move { Ok(()) }));

    // 5. Failed attempts reset
    let attempts_key = iam_core::domain::identity::keys::cache::login_attempts(username);
    cache.expect_delete()
        .with(mockall::predicate::eq(attempts_key))
        .returning(|_| Box::pin(async move { Ok(()) }));

    // 6. Event publish
    event_bus.expect_publish_batch()
        .returning(|_| Box::pin(async move { Ok(()) }));

    let wallet_service = mock_wallet_service();

    let auth_service = AuthService::new(
        Arc::new(user_repo),
        Arc::new(wallet_repo),
        Arc::new(api_key_repo),
        config,
        jwt_service,
        api_key_service,
        Arc::new(cache),
        Arc::new(event_bus),
        Arc::new(blockchain_service),
        wallet_service,
    );

    let result = auth_service.login(username.to_string(), password.to_string()).await.expect("Login failed");
    assert_eq!(result.user.id, user_id);
}

#[tokio::test]
async fn test_register_success() {
    let mut user_repo = MockUserRepositoryTrait::new();
    let wallet_repo = MockWalletRepositoryTrait::new();
    let api_key_repo = MockApiKeyRepositoryTrait::new();
    let cache = MockCacheTrait::new();
    let mut event_bus = MockEventBusTrait::new();
    let blockchain_service = MockBlockchainTrait::new();

    let config = mock_config();
    let jwt_service = JwtService::new(&config.jwt_secret).unwrap();
    let api_key_service = ApiKeyService::new(config.api_key_secret.clone()).unwrap();

    let username = "newuser";
    let email = "new@example.com";
    let password = "GridTokenX-$2024-@NewRegistration";
    
    // 1. User creation
    user_repo.expect_create()
        .returning(|_, _, _, _, _, _, _, _| Box::pin(async move { Ok(()) }));

    // 2. Event publish
    event_bus.expect_publish()
        .returning(|_| Box::pin(async move { Ok(()) }));

    let wallet_service = mock_wallet_service();

    let auth_service = AuthService::new(
        Arc::new(user_repo),
        Arc::new(wallet_repo),
        Arc::new(api_key_repo),
        config,
        jwt_service,
        api_key_service,
        Arc::new(cache),
        Arc::new(event_bus),
        Arc::new(blockchain_service),
        wallet_service,
    );

    let result = auth_service.register(
        username.to_string(),
        email.to_string(),
        password.to_string(),
        None,
        None,
    ).await.expect("Registration failed");

    assert_eq!(result.username, username);
}

/// Builds an `AuthService` from the supplied mocks with the standard test config.
fn build_auth_service(
    user_repo: MockUserRepositoryTrait,
    cache: MockCacheTrait,
    event_bus: MockEventBusTrait,
) -> AuthService {
    let config = mock_config();
    let jwt_service = JwtService::new(&config.jwt_secret).unwrap();
    let api_key_service = ApiKeyService::new(config.api_key_secret.clone()).unwrap();
    AuthService::new(
        Arc::new(user_repo),
        Arc::new(MockWalletRepositoryTrait::new()),
        Arc::new(MockApiKeyRepositoryTrait::new()),
        config,
        jwt_service,
        api_key_service,
        Arc::new(cache),
        Arc::new(event_bus),
        Arc::new(MockBlockchainTrait::new()),
        mock_wallet_service(),
    )
}

/// Builds an `AuthService` whose API-key repo is caller-supplied (the standard
/// `build_auth_service` stubs an empty one). Used by the `verify_api_key` tests.
fn build_auth_service_with_keys(
    api_key_repo: MockApiKeyRepositoryTrait,
    cache: MockCacheTrait,
    event_bus: MockEventBusTrait,
) -> AuthService {
    let config = mock_config();
    let jwt_service = JwtService::new(&config.jwt_secret).unwrap();
    let api_key_service = ApiKeyService::new(config.api_key_secret.clone()).unwrap();
    AuthService::new(
        Arc::new(MockUserRepositoryTrait::new()),
        Arc::new(MockWalletRepositoryTrait::new()),
        Arc::new(api_key_repo),
        config,
        jwt_service,
        api_key_service,
        Arc::new(cache),
        Arc::new(event_bus),
        Arc::new(MockBlockchainTrait::new()),
        mock_wallet_service(),
    )
}

/// Test fixture: an active API key whose `key_hash` matches `raw` under the test
/// secret, so the cache key derived inside `verify_api_key` lines up.
fn fixture_api_key(raw: &str) -> ApiKey {
    let api_key_service = ApiKeyService::new(mock_config().api_key_secret.clone()).unwrap();
    ApiKey {
        id: Uuid::new_v4(),
        key_hash: api_key_service.hash_key(raw).unwrap(),
        name: "meter-1".to_string(),
        role: "aggregator-bridge".to_string(),
        permissions: vec![],
        is_active: true,
        created_at: chrono::Utc::now(),
        last_used_at: None,
    }
}

/// Cache hit ⇒ hot path. The key was verified within the TTL window, so the
/// per-request `update_last_used` DB write, the `cache_set`, and the event
/// publish are all skipped — only the cache GET runs. Guards the #2/#3 fix
/// (write/event amplification) against regression.
#[tokio::test]
async fn verify_api_key_cache_hit_skips_db_and_event() {
    let mut api_key_repo = MockApiKeyRepositoryTrait::new();
    let mut cache = MockCacheTrait::new();
    let mut event_bus = MockEventBusTrait::new();

    let raw_key = "gridtokenx-cache-hit-key";
    let api_key = fixture_api_key(raw_key);
    let cached_value = serde_json::to_value(&api_key).unwrap();

    cache.expect_get_value()
        .returning(move |_| {
            let v = cached_value.clone();
            Box::pin(async move { Ok(Some(v)) })
        });
    // Hot path must NOT touch DB, re-cache, or emit events.
    api_key_repo.expect_update_last_used().times(0);
    cache.expect_set_value().times(0);
    event_bus.expect_publish().times(0);

    let auth_service = build_auth_service_with_keys(api_key_repo, cache, event_bus);
    let result = auth_service.verify_api_key(raw_key).await.expect("verify failed");
    assert_eq!(result.role, "aggregator-bridge");
}

/// Cache miss + DB hit ⇒ cold path. Resolve from Postgres, then refresh
/// `last_used_at`, populate the cache, and publish exactly one event.
#[tokio::test]
async fn verify_api_key_cache_miss_db_hit_caches_and_publishes() {
    let mut api_key_repo = MockApiKeyRepositoryTrait::new();
    let mut cache = MockCacheTrait::new();
    let mut event_bus = MockEventBusTrait::new();

    let raw_key = "gridtokenx-cold-key";
    let api_key = fixture_api_key(raw_key);

    cache.expect_get_value()
        .returning(|_| Box::pin(async move { Ok(None) }));
    api_key_repo.expect_find_by_hash()
        .returning(move |_| {
            let k = api_key.clone();
            Box::pin(async move { Ok(Some(k)) })
        });
    api_key_repo.expect_update_last_used()
        .times(1)
        .returning(|_| Box::pin(async move { Ok(()) }));
    cache.expect_set_value()
        .times(1)
        .returning(|_, _, _| Box::pin(async move { Ok(()) }));
    event_bus.expect_publish()
        .times(1)
        .returning(|_| Box::pin(async move { Ok(()) }));

    let auth_service = build_auth_service_with_keys(api_key_repo, cache, event_bus);
    let result = auth_service.verify_api_key(raw_key).await.expect("verify failed");
    assert_eq!(result.role, "aggregator-bridge");
}

/// Cache miss + no DB row (missing/inactive) ⇒ genuine auth failure. Surfaces as
/// `ApiError::Unauthorized` (which the gRPC handler maps to `valid:false`), and
/// nothing is cached or published.
#[tokio::test]
async fn verify_api_key_unknown_key_is_unauthorized() {
    let mut api_key_repo = MockApiKeyRepositoryTrait::new();
    let mut cache = MockCacheTrait::new();
    let mut event_bus = MockEventBusTrait::new();

    cache.expect_get_value()
        .returning(|_| Box::pin(async move { Ok(None) }));
    api_key_repo.expect_find_by_hash()
        .returning(|_| Box::pin(async move { Ok(None) }));
    cache.expect_set_value().times(0);
    event_bus.expect_publish().times(0);

    let auth_service = build_auth_service_with_keys(api_key_repo, cache, event_bus);
    let err = auth_service.verify_api_key("nope").await.unwrap_err();
    assert!(matches!(err, ApiError::Unauthorized(_)), "expected Unauthorized, got {err:?}");
}

/// Cache miss + DB infra error ⇒ must propagate as a NON-Unauthorized error so
/// the gRPC handler maps it to a retryable `Unavailable` rather than masking an
/// outage as an invalid key. Guards the #4 fix.
#[tokio::test]
async fn verify_api_key_db_error_is_not_unauthorized() {
    let mut api_key_repo = MockApiKeyRepositoryTrait::new();
    let mut cache = MockCacheTrait::new();
    let mut event_bus = MockEventBusTrait::new();

    cache.expect_get_value()
        .returning(|_| Box::pin(async move { Ok(None) }));
    api_key_repo.expect_find_by_hash()
        .returning(|_| Box::pin(async move { Err(ApiError::Internal("db down".to_string())) }));
    cache.expect_set_value().times(0);
    event_bus.expect_publish().times(0);

    let auth_service = build_auth_service_with_keys(api_key_repo, cache, event_bus);
    let err = auth_service.verify_api_key("any").await.unwrap_err();
    assert!(!matches!(err, ApiError::Unauthorized(_)), "infra error must not look like a bad key");
    assert!(matches!(err, ApiError::Internal(_)), "expected Internal, got {err:?}");
}

/// First resend for an unverified account: publishes the email event AND arms the
/// per-account cooldown key so the next request can be throttled.
#[tokio::test]
async fn resend_verification_sends_and_arms_cooldown() {
    let mut user_repo = MockUserRepositoryTrait::new();
    let mut cache = MockCacheTrait::new();
    let mut event_bus = MockEventBusTrait::new();

    let user_id = Uuid::new_v4();
    user_repo.expect_find_verification_state_by_email()
        .returning(move |_| {
            let state = EmailVerificationState {
                user_id,
                username: "pending".to_string(),
                email_verified: false,
                // Existing token → no set_verification_token call.
                verification_token: Some("tok-123".to_string()),
            };
            Box::pin(async move { Ok(Some(state)) })
        });

    // Not in cooldown → send proceeds.
    cache.expect_exists()
        .returning(|_| Box::pin(async move { Ok(false) }));
    // Email event published exactly once.
    event_bus.expect_publish()
        .times(1)
        .returning(|_| Box::pin(async move { Ok(()) }));
    // Cooldown armed exactly once after send.
    cache.expect_set_value()
        .times(1)
        .returning(|_, _, _| Box::pin(async move { Ok(()) }));

    let auth_service = build_auth_service(user_repo, cache, event_bus);
    let result = auth_service.resend_verification("pending@example.com").await.expect("resend failed");
    assert_eq!(result.status, "sent");
}

/// Resend while still inside the cooldown window: returns the SAME generic
/// response but publishes nothing and re-arms nothing — throttle holds and no
/// enumeration signal leaks.
#[tokio::test]
async fn resend_verification_suppressed_during_cooldown() {
    let mut user_repo = MockUserRepositoryTrait::new();
    let mut cache = MockCacheTrait::new();
    let mut event_bus = MockEventBusTrait::new();

    let user_id = Uuid::new_v4();
    user_repo.expect_find_verification_state_by_email()
        .returning(move |_| {
            let state = EmailVerificationState {
                user_id,
                username: "pending".to_string(),
                email_verified: false,
                verification_token: Some("tok-123".to_string()),
            };
            Box::pin(async move { Ok(Some(state)) })
        });

    // Cooldown key present → throttled.
    cache.expect_exists()
        .returning(|_| Box::pin(async move { Ok(true) }));
    // No email published while throttled.
    event_bus.expect_publish().times(0);
    // No cooldown re-arm while throttled.
    cache.expect_set_value().times(0);

    let auth_service = build_auth_service(user_repo, cache, event_bus);
    let result = auth_service.resend_verification("pending@example.com").await.expect("resend failed");
    // Indistinguishable from a real send.
    assert_eq!(result.status, "sent");
}

/// Guards the registry UserAccount PDA seed against regression. The on-chain
/// registry program derives the account with seeds `[b"user", authority]`; IAM
/// once used `b"user_account"`, which produced a PDA that never existed on-chain.
#[test]
fn user_account_pda_uses_registry_seed() {
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    let program = Pubkey::from_str("FcSd5x4X1nzJMKLZC4tMZXnQ1ipLrGsEfeoH8N4mvJX7").unwrap();
    let wallet = Pubkey::from_str("2HznxvQihameZMgj4C2RoRAP9PJHnrSp3RzB7JtEgEky").unwrap();

    let (pda, _) = Pubkey::find_program_address(&[b"user", wallet.as_ref()], &program);

    // Value produced on-chain by the registry RegisterUser instruction.
    assert_eq!(pda.to_string(), "2We19dA5Z8RNg9DR2QKwY1i7ac9dkojiWVDBrGG48SAM");
}

/// Cross-component contract for the custodial-wallet auto-provision path that
/// `verify_email` drives across logic → persistence → blockchain traits.
///
/// Hard-proves two regressions stay fixed without any running infrastructure:
/// - **lat/long preservation**: `mark_user_onboarded` must receive the user's
///   real coordinates, never `0.0, 0.0` (which `mark_user_onboarded`'s
///   unconditional `UPDATE` would persist, clobbering location).
/// - **on-chain bookkeeping**: a successful `register_user_on_chain` is followed
///   by `mark_registered` + `mark_user_onboarded`, and the provisioned address
///   propagates to the verification result.
#[tokio::test]
async fn verify_email_provisions_custodial_wallet_preserves_location() {
    use iam_core::domain::identity::{UserType, UserWallet};

    const LAT: f64 = 13.7563;
    const LONG: f64 = 100.5018;

    let mut user_repo = MockUserRepositoryTrait::new();
    let mut wallet_repo = MockWalletRepositoryTrait::new();
    let api_key_repo = MockApiKeyRepositoryTrait::new();
    let mut cache = MockCacheTrait::new();
    let mut event_bus = MockEventBusTrait::new();
    let mut blockchain_service = MockBlockchainTrait::new();

    let config = mock_config();
    let jwt_service = JwtService::new(&config.jwt_secret).unwrap();
    let api_key_service = ApiKeyService::new(config.api_key_secret.clone()).unwrap();

    let user_id = Uuid::new_v4();
    let email = "wallet@example.com";
    let user = User {
        id: user_id,
        username: "walletuser".to_string(),
        email: email.to_string(),
        role: Role::User.to_string(),
        first_name: None,
        last_name: None,
        wallet_address: None,
        is_active: true,
        blockchain_registered: false,
        user_type: Some(UserType::Consumer),
        latitude: Some(LAT),
        longitude: Some(LONG),
    };

    // `verify_<email>` dev shortcut (config.environment == "test") → no token lookup.
    user_repo.expect_verify_email()
        .returning(move |_| {
            let user = user.clone();
            Box::pin(async move { Ok(Some(user)) })
        });

    // No existing wallet → provisioning proceeds.
    wallet_repo.expect_has_any_wallet()
        .returning(|_| Box::pin(async move { Ok(false) }));

    // Atomic persist returns the freshly minted primary custodial wallet.
    wallet_repo.expect_persist_custodial_wallet()
        .returning(move |uid, addr, _label, _enc, _salt, _iv, _kdf| {
            let wallet = UserWallet {
                id: Uuid::new_v4(),
                user_id: uid,
                wallet_address: addr.to_string(),
                label: Some("Custodial".to_string()),
                is_primary: true,
                verified: false,
                blockchain_registered: false,
                user_account_pda: None,
                shard_id: None,
                blockchain_tx_signature: None,
                created_at: chrono::Utc::now(),
            };
            Box::pin(async move { Ok(wallet) })
        });

    // On-chain registration succeeds → bookkeeping must follow.
    blockchain_service.expect_register_user_on_chain()
        .returning(|_, _, _, _, _, _| {
            Box::pin(async move { Ok(solana_sdk::signature::Signature::default()) })
        });
    // Registration is only recorded once the PDA is confirmed on-chain.
    blockchain_service.expect_account_exists()
        .returning(|_| Box::pin(async move { Ok(true) }));
    wallet_repo.expect_mark_registered()
        .returning(|_, _, _| Box::pin(async move { Ok(()) }));

    // On-chain registration now runs in a DETACHED task (verify_email no longer
    // blocks on it). mark_user_onboarded firing is the tail of that task, so the
    // test signals on it and waits below — otherwise the mocks' (default times(1))
    // expectations would be checked at drop before the task ran, flakily failing.
    let (done_tx, mut done_rx) = tokio::sync::mpsc::unbounded_channel::<()>();

    // THE Fix-1 assertion: coordinates flow through unchanged, never 0.0/0.0.
    user_repo.expect_mark_user_onboarded()
        .withf(move |_uid, _utype, lat, long, _pda, _sig| {
            (*lat - LAT).abs() < f64::EPSILON && (*long - LONG).abs() < f64::EPSILON
        })
        .returning(move |_, _, _, _, _, _| {
            let done_tx = done_tx.clone();
            Box::pin(async move {
                let _ = done_tx.send(());
                Ok(())
            })
        });

    event_bus.expect_publish()
        .returning(|_| Box::pin(async move { Ok(()) }));
    cache.expect_delete()
        .returning(|_| Box::pin(async move { Ok(()) }));

    let wallet_service = mock_wallet_service();
    let auth_service = AuthService::new(
        Arc::new(user_repo),
        Arc::new(wallet_repo),
        Arc::new(api_key_repo),
        config,
        jwt_service,
        api_key_service,
        Arc::new(cache),
        Arc::new(event_bus),
        Arc::new(blockchain_service),
        wallet_service,
    );

    let result = auth_service
        .verify_email(format!("verify_{email}"))
        .await
        .expect("verify_email failed");

    assert!(result.success);
    assert!(result.wallet_address.is_some(), "custodial wallet address must propagate to result");

    // Drive + await the detached on-chain task: mark_user_onboarded sends here only
    // after register_user_on_chain → confirm → mark_registered all ran and the
    // lat/long `withf` matched. Awaiting also lets the current-thread runtime poll
    // the spawned task to completion so every mock expectation is satisfied.
    tokio::time::timeout(std::time::Duration::from_secs(5), done_rx.recv())
        .await
        .expect("background on-chain registration did not complete in time")
        .expect("on-chain bookkeeping channel closed before signaling");
}
