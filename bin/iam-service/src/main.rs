use iam_core::config::Config;
use gridtokenx_iam_service::telemetry;
use gridtokenx_iam_service::startup;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Install default crypto provider for rustls
    rustls::crypto::ring::default_provider().install_default().expect("Failed to install default crypto provider");

    // Load environment variables
    dotenvy::dotenv().ok();

    // 1. Initialize Telemetry (Tracing, Metrics)
    telemetry::init_telemetry("gridtokenx-iam");

    // 2. Initialize Config
    let config = Config::from_env()?;

    // 3. Graceful Shutdown Token
    let token = CancellationToken::new();
    let ctrl_c_token = token.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        println!("\n🔄 Shutdown signal received...");
        ctrl_c_token.cancel();
    });

    // 4. Run Server
    startup::run(config, token.clone()).await?;

    Ok(())
}
