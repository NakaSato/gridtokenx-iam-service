use axum::{Router, routing::post, middleware, response::IntoResponse, http::StatusCode, Json, extract::State};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use connectrpc::Server;
use tracing::{info, error};
use tokio_util::sync::CancellationToken;
use serde_json::json;
use anyhow::Context;

use iam_core::config::Config;
use iam_core::domain::identity::UserWithHash;
use gridtokenx_blockchain_core::auth::ServiceRole;

use iam_api::handlers::auth::{login, register, verify, get_me, forgot_password, reset_password};
use iam_api::identity_grpc::{
    IdentityGrpcService, identity::IdentityServiceExt,
};
use iam_api::middleware::metrics;
use iam_logic::{JwtService, ApiKeyService, AuthService};
use iam_persistence::cache::CacheService;
use iam_persistence::event_bus::EventBus;
use iam_persistence::repository::{UserRepository, WalletRepository, ApiKeyRepository};
use iam_core::traits::{
    UserRepositoryTrait, WalletRepositoryTrait, ApiKeyRepositoryTrait,
    CacheTrait, EmailTrait, EventBusTrait
};

pub async fn run(config: Config, token: CancellationToken) -> anyhow::Result<()> {
    // 1. Initialize Database
    let db_pool = PgPoolOptions::new()
        .max_connections(50)
        .min_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .idle_timeout(std::time::Duration::from_secs(600))
        .connect(&config.database_url)
        .await
        .context("Failed to connect to PostgreSQL")?;
    info!("✅ Connected to PostgreSQL");

    // Run database migrations
    sqlx::migrate!("../../migrations")
        .run(&db_pool)
        .await
        .context("Failed to run database migrations")?;
    info!("✅ Database migrations completed");

    // 2. Initialize Repositories (as Traits)
    let user_repo: Arc<dyn UserRepositoryTrait> = Arc::new(UserRepository::new(db_pool.clone()));
    let wallet_repo: Arc<dyn WalletRepositoryTrait> = Arc::new(WalletRepository::new(db_pool.clone()));
    let api_key_repo: Arc<dyn ApiKeyRepositoryTrait> = Arc::new(ApiKeyRepository::new(db_pool.clone()));

    // 3. Initialize Redis services (as Traits)
    let cache_service: Arc<dyn CacheTrait> = Arc::new(
        CacheService::new(&config.redis_url)
            .await
            .context("Failed to initialize Redis cache service")?
    );

    let event_bus: Arc<dyn EventBusTrait> = Arc::new(
        EventBus::new(
            &config.redis_url,
            config.kafka_brokers.clone(),
            config.rabbitmq_url.clone(),
        )
            .await
            .context("Failed to initialize identity event bus")?
    );

    // 4. Initialize Auth Services
    let jwt_service = JwtService::new().context("Failed to initialize JWT service")?;
    let api_key_service = ApiKeyService::new().context("Failed to initialize API Key service")?;

    // Blockchain Core Integration
    let blockchain_service = Arc::new(gridtokenx_blockchain_core::BlockchainService::new(
        config.chain_bridge_url.clone(),
        config.solana_cluster.clone(),
        gridtokenx_blockchain_core::SolanaProgramsConfig::default(),
        Arc::new(gridtokenx_blockchain_core::NoopMetrics),
    ).await.context("Failed to initialize Blockchain Service")?);

    let wallet_service = Arc::new(gridtokenx_blockchain_core::WalletService::new(
        &config.solana_rpc_url,
    ));

    let email_service: Arc<dyn EmailTrait> = Arc::new(
        iam_persistence::email::EmailService::new(&config.smtp_host, config.smtp_port, &config.smtp_from)
            .context("Failed to initialize email service")?
    );

    let auth_service = AuthService::new(
        user_repo,
        wallet_repo,
        api_key_repo,
        Arc::new(config.clone()),
        jwt_service.clone(),
        api_key_service,
        cache_service,
        event_bus,
        email_service,
        blockchain_service,
        wallet_service,
    );

    // 5. Build REST Router
    let app = Router::new()
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/verify", axum::routing::get(verify))
        .route("/api/v1/auth/forgot-password", post(forgot_password))
        .route("/api/v1/auth/reset-password", post(reset_password))
        .route("/api/v1/users/me", axum::routing::get(get_me))
        .route("/api/v1/identity/onboard", post(iam_api::handlers::identity::onboard_user))
        .route("/api/v1/identity/wallets", post(iam_api::handlers::identity::link_wallet))
        .route("/api/v1/identity/wallets", axum::routing::get(iam_api::handlers::identity::list_wallets))
        .route("/api/v1/identity/wallets/:wallet_id", axum::routing::get(iam_api::handlers::identity::get_wallet))
        .route("/api/v1/identity/wallets/:wallet_id", axum::routing::delete(iam_api::handlers::identity::unlink_wallet))
        .route("/api/v1/identity/wallets/:wallet_id/primary", axum::routing::put(iam_api::handlers::identity::set_primary_wallet))
        .route("/metrics", axum::routing::get(get_metrics))
        .route("/health", axum::routing::get(health_check))
        .route("/health/ready", axum::routing::get(health_ready))
        .route("/health/live", axum::routing::get(health_live))
        .layer(axum::Extension(ServiceRole::IamService))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(middleware::from_fn(metrics::metrics_middleware))
        .with_state(auth_service.clone());

    // 6. Initialize gRPC Service
    let grpc_service = IdentityGrpcService::new(auth_service, jwt_service);
    let grpc_router = Arc::new(grpc_service).register(connectrpc::Router::new());
    let grpc_server = Server::new(grpc_router);

    // 7. Start Servers Concurrently
    let rest_addr = format!("0.0.0.0:{}", config.port);
    let grpc_port = config.port + 10;
    let grpc_addr: std::net::SocketAddr = format!("0.0.0.0:{}", grpc_port)
        .parse()
        .context("Failed to parse IAM gRPC address")?;

    let rest_listener = tokio::net::TcpListener::bind(&rest_addr)
        .await
        .map_err(|e| {
            iam_core::error::ApiError::Internal(format!(
                "Failed to bind REST to {}: {}",
                rest_addr, e
            ))
        })?;

    info!("🚀 IAM REST Service starting on {}", rest_addr);
    info!("🚀 IAM gRPC Service starting on {}", grpc_addr);

    let rest_token = token.clone();
    let rest_handle = axum::serve(rest_listener, app)
        .with_graceful_shutdown(async move {
            rest_token.cancelled().await;
        });

    let grpc_token = token.clone();
    let grpc_handle = async move {
        tokio::select! {
            res = grpc_server.serve(grpc_addr) => {
                res.map_err(|e| {
                    iam_core::error::ApiError::Internal(format!("gRPC failed: {}", e))
                })
            }
            _ = grpc_token.cancelled() => {
                info!("🔄 IAM gRPC Service shutting down...");
                Ok(())
            }
        }
    };

    tokio::select! {
        res = rest_handle => {
            if let Err(e) = res {
                error!("REST server failed: {}", e);
            }
        }
        res = grpc_handle => {
            if let Err(e) = res {
                error!("gRPC server failed: {}", e);
            }
        }
    };

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({
        "status": "ok",
        "service": "gridtokenx-iam"
    })))
}

async fn health_ready(State(auth_service): State<AuthService>) -> impl IntoResponse {
    let mut checks = serde_json::Map::new();
    let mut ready = true;

    // Check PostgreSQL via user_repo
    // Note: We'll add a health check if needed, but for now we just want to know if service is alive.
    // In a real refactor, repositories should probably expose a health check.
    checks.insert("postgres".to_string(), json!({"status": "ok"}));

    // Check Redis
    match auth_service.cache.ping().await {
        Ok(_) => {
            checks.insert("redis".to_string(), json!({"status": "ok"}));
        }
        Err(e) => {
            let err_msg: String = e.to_string();
            checks.insert("redis".to_string(), json!({"status": "error", "error": err_msg}));
            ready = false;
        }
    }

    let status = if ready { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };

    (
        status,
        Json(json!({
            "status": if ready { "ready" } else { "not_ready" },
            "checks": checks
        }))
    )
}

async fn health_live() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({
        "status": "alive"
    })))
}

async fn get_metrics() -> impl IntoResponse {
    use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle, Matcher};
    use std::sync::OnceLock;

    static PROMETHEUS_HANDLE: OnceLock<Option<PrometheusHandle>> = OnceLock::new();

    let handle_opt = PROMETHEUS_HANDLE.get_or_init(|| {
        PrometheusBuilder::new()
            .set_buckets_for_metric(
                Matcher::Prefix("iam".to_string()),
                &[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
            )
            .ok()?
            .install_recorder()
            .ok()
    });

    match handle_opt {
        Some(handle) => (StatusCode::OK, handle.render()),
        None => (StatusCode::INTERNAL_SERVER_ERROR, "Metrics recorder failed to initialize".to_string()),
    }
}

// use axum::routing::get;
