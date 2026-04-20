#[cfg(test)]
mod tests {
    use axum::{Router, routing::post, http::{Request, StatusCode, header}, body::Body};
    use tower::ServiceExt;
    use http_body_util::BodyExt;
    use iam_core::traits::{
        MockUserRepositoryTrait, MockWalletRepositoryTrait, MockApiKeyRepositoryTrait,
        MockCacheTrait, MockEmailTrait, MockEventBusTrait, MockBlockchainTrait
    };
    use iam_core::config::Config;
    use iam_logic::AuthService;
    use iam_logic::{JwtService, ApiKeyService};
    use crate::handlers::auth::login;
    use crate::handlers::types::LoginRequest;
    use std::sync::Arc;
    use uuid::Uuid;
    use iam_core::domain::identity::{User, UserWithHash};
    use gridtokenx_blockchain_core::auth::INTERNAL_ROLE_HEADER;
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
            grpc_port: None,
            registry_program_id: "HZR6b8G3pUUKRnt4XAMA8rdYRpAsNY1xk3Zo4crShvY".to_string(),
            oracle_program_id: "DdeZQdfv7qtnhHktPt8CevKrW6BvjbgKknkD7c63C9hP".to_string(),
            governance_program_id: "6FsfuFEg8LHjSiejc8om8Q6iSaAgfEWHCgz78PT8jocw".to_string(),
            energy_token_program_id: "GjSjmPt8VSHr49ti4BijWZSu7rwb8o32pod7gNBnTY4U".to_string(),
            trading_program_id: "DXxHdUar3pUUKRnt4XAMA8rdYRpAsNY1xk3Zo4crShvY".to_string(),
            auth_cpu_semaphore_limit: 32,
            tokio_worker_threads: Some(4),
            database_max_connections: 50,
            database_min_connections: 5,
            request_timeout_secs: 30,
            global_concurrency_limit: 100,
        })
    }

    #[tokio::test]
    async fn test_api_login_unauthorized() {
        // This fails with UNAUTHORIZED (401) because user is not found, 
        // not because of RBAC (since we allow Unknown).
        
        let mut user_repo = MockUserRepositoryTrait::new();
        user_repo.expect_find_by_username_or_email()
            .returning(|_| Box::pin(async { Ok(None) }));

        let mut cache = MockCacheTrait::new();
        cache.expect_exists().returning(|_| Box::pin(async { Ok(false) }));
        cache.expect_get_value().returning(|_| Box::pin(async { Ok(None) }));
        
        let mut event_bus = MockEventBusTrait::new();
        // Login attempt event on failure
        event_bus.expect_publish().returning(|_| Box::pin(async { Ok(()) }));

        let config = mock_config();
        let jwt_service = JwtService::new(&config.jwt_secret).unwrap();
        let api_key_service = ApiKeyService::new(config.api_key_secret.clone()).unwrap();
        
        let auth_service = AuthService::new(
            Arc::new(user_repo),
            Arc::new(MockWalletRepositoryTrait::new()),
            Arc::new(MockApiKeyRepositoryTrait::new()),
            config,
            jwt_service,
            api_key_service,
            Arc::new(cache),
            Arc::new(event_bus),
            Arc::new(MockEmailTrait::new()),
            Arc::new(MockBlockchainTrait::new()),
            mock_wallet_service(),
        );

        let app = Router::new()
            .route("/api/v1/auth/login", post(login))
            .with_state(auth_service);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_string(&LoginRequest {
                        username: "test".to_string(),
                        password: "password".to_string(),
                    }).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_api_login_success() {
        let user_id = Uuid::new_v4();
        let username = "testuser";
        let email = "test@example.com";
        let hash = bcrypt::hash("StrongPass123!", bcrypt::DEFAULT_COST).unwrap();

        let mut user_repo = MockUserRepositoryTrait::new();
        user_repo.expect_find_by_username_or_email()
            .returning(move |_| {
                let user = User {
                    id: user_id,
                    username: username.to_string(),
                    email: email.to_string(),
                    role: "user".to_string(),
                    first_name: None,
                    last_name: None,
                    wallet_address: None,
                    is_active: true,
                    blockchain_registered: false,
                    user_type: None,
                    latitude: None,
                    longitude: None,
                };
                let hash_clone = hash.clone();
                Box::pin(async move {
                    Ok(Some(UserWithHash {
                        user,
                        password_hash: hash_clone,
                    }))
                })
            });

        let mut cache = MockCacheTrait::new();
        cache.expect_exists().returning(|_| Box::pin(async { Ok(false) }));
        cache.expect_get_value().returning(|_| Box::pin(async { Ok(None) }));
        cache.expect_set_value().returning(|_, _, _| Box::pin(async { Ok(()) }));
        cache.expect_delete().returning(|_| Box::pin(async { Ok(()) }));

        let mut event_bus = MockEventBusTrait::new();
        event_bus.expect_publish_batch().returning(|_| Box::pin(async { Ok(()) }));

        let config = mock_config();
        let jwt_service = JwtService::new(&config.jwt_secret).unwrap();
        let api_key_service = ApiKeyService::new(config.api_key_secret.clone()).unwrap();
        
        let auth_service = AuthService::new(
            Arc::new(user_repo),
            Arc::new(MockWalletRepositoryTrait::new()),
            Arc::new(MockApiKeyRepositoryTrait::new()),
            config,
            jwt_service,
            api_key_service,
            Arc::new(cache),
            Arc::new(event_bus),
            Arc::new(MockEmailTrait::new()),
            Arc::new(MockBlockchainTrait::new()),
            mock_wallet_service(),
        );

        let app = Router::new()
            .route("/api/v1/auth/login", post(login))
            .with_state(auth_service);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(INTERNAL_ROLE_HEADER, "api-gateway")
                    .header("x-gridtokenx-gateway-secret", "gridtokenx-gateway-secret-2025")
                    .body(Body::from(serde_json::to_string(&LoginRequest {
                        username: username.to_string(),
                        password: "StrongPass123!".to_string(),
                    }).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(body["access_token"].is_string());
        assert_eq!(body["user"]["username"], username);
    }
}
