use axum::{Router, routing::post, middleware};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use connectrpc::Server;
use tracing::{info, error};
use tokio_util::sync::CancellationToken;

use crate::api::handlers::auth::{login, register, verify};
use crate::api::identity_grpc::{
    IdentityGrpcService, identity::IdentityServiceExt,
};
use crate::api::middleware::metrics;
use crate::api::middleware::otel_tracing;
use crate::core::config::Config;
use anyhow::Context as _;
use crate::domain::identity::{JwtService, ApiKeyService};
use crate::services::AuthService;

pub async fn run(config: Config, token: CancellationToken) -> anyhow::Result<()> {
    // 1. Initialize Database
    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await
        .context("Failed to connect to PostgreSQL")?;
    info!("✅ Connected to PostgreSQL");

    // Run database migrations
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .context("Failed to run database migrations")?;
    info!("✅ Database migrations completed");

    // 2. Initialize Services
    let jwt_service = JwtService::new().context("Failed to initialize JWT service")?;
    let api_key_service = ApiKeyService::new().context("Failed to initialize API Key service")?;
    let auth_service = AuthService::new(
        db_pool.clone(),
        Arc::new(config.clone()),
        jwt_service.clone(),
        api_key_service,
    );

    // 3. Build REST Router with metrics and OTel tracing middleware
    let app = Router::new()
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/token", post(login))
        .route("/api/v1/auth/verify", axum::routing::get(verify))
        .route("/metrics", axum::routing::get(get_metrics))
        .layer(middleware::from_fn(otel_tracing::otel_tracing_middleware))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(middleware::from_fn(metrics::metrics_middleware))
        .with_state(auth_service.clone());

    // 4. Initialize gRPC Service
    let grpc_service = IdentityGrpcService::new(auth_service, jwt_service);
    let grpc_router = Arc::new(grpc_service).register(connectrpc::Router::new());
    let grpc_server = Server::new(grpc_router);

    // 5. Start Servers Concurrently
    let rest_addr = format!("0.0.0.0:{}", config.port);
    let grpc_port = config.port + 10; // Simple offset for gRPC
    let grpc_addr: std::net::SocketAddr = format!("0.0.0.0:{}", grpc_port)
        .parse()
        .context("Failed to parse IAM gRPC address")?;

    let rest_listener = tokio::net::TcpListener::bind(&rest_addr)
        .await
        .map_err(|e| {
            crate::core::error::ApiError::Internal(format!(
                "Failed to bind REST to {}: {}",
                rest_addr, e
            ))
        })?;

    info!("🚀 IAM REST Service starting on {}", rest_addr);
    info!("🚀 IAM gRPC Service starting on {}", grpc_addr);

    // Run both servers
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
                    crate::core::error::ApiError::Internal(format!("gRPC failed: {}", e))
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

use axum::{response::IntoResponse, http::StatusCode};
// use tracing::error;

/// Metrics endpoint handler - exposes Prometheus-format metrics
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
