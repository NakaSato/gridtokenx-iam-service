use std::sync::Arc;
use uuid::Uuid;
use iam_core::domain::identity::{User, UserWithHash, Role};
use iam_core::traits::{
    MockUserRepositoryTrait, MockWalletRepositoryTrait, MockApiKeyRepositoryTrait,
    MockCacheTrait, MockEmailTrait, MockEventBusTrait, MockBlockchainTrait
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
    let email_service = MockEmailTrait::new();
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
        Arc::new(email_service),
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
    let email_service = MockEmailTrait::new();
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
        Arc::new(email_service),
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
