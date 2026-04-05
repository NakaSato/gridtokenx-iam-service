use gridtokenx_iam_service::core::config::Config;
use gridtokenx_iam_service::startup;
use gridtokenx_iam_service::telemetry;
use tracing::{info, error};
use tokio_util::sync::CancellationToken;
use tokio::signal;

#[tokio::main]
async fn main() {
    // Initialize OpenTelemetry tracing (sets up global subscriber)
    let telemetry_guard = telemetry::init_telemetry("gridtokenx-iam");

    // Load configuration
    let config = match Config::from_env() {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Lifecycle coordination
    let shutdown_token = CancellationToken::new();
    let service_token = shutdown_token.clone();

    // Spawn signal handler
    tokio::spawn(async move {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                info!("🛑 SIGINT received, triggering shutdown...");
            },
            _ = terminate => {
                info!("🛑 SIGTERM received, triggering shutdown...");
            },
        }

        shutdown_token.cancel();
    });

    // Run the service
    if let Err(e) = startup::run(config, service_token).await {
        error!("❌ IAM Service failed: {:#}", e);
        telemetry::shutdown_telemetry(&telemetry_guard);
        std::process::exit(1);
    }

    info!("👋 Shutdown complete. Cleaning up telemetry...");
    telemetry::shutdown_telemetry(&telemetry_guard);
}
