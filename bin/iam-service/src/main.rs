use iam_core::config::Config;
use gridtokenx_iam_service::telemetry;
use gridtokenx_iam_service::startup;
use tokio_util::sync::CancellationToken;

fn main() -> anyhow::Result<()> {
    // Install default crypto provider for rustls
    rustls::crypto::ring::default_provider().install_default().expect("Failed to install default crypto provider");

    // Load environment variables
    dotenvy::dotenv().ok();

    // 1. Initialize Config (early to setup runtime)
    let config = Config::from_env()?;

    // 2. Initialize Telemetry (Tracing, Metrics)
    let _telemetry = telemetry::init_telemetry("gridtokenx-iam");

    // 3. Build optimized Tokio Runtime
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    builder.thread_name("iam-worker");

    if let Some(threads) = config.tokio_worker_threads {
        tracing::info!("Tokio worker threads explicitly set to: {}", threads);
        builder.worker_threads(threads);
    }

    let runtime = builder.build()?;

    // 4. Run Application
    runtime.block_on(async {
        // Graceful Shutdown Token
        let token = CancellationToken::new();
        let ctrl_c_token = token.clone();

        tokio::spawn(async move {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to listen for Ctrl+C");
            println!("\n🔄 Shutdown signal received...");
            ctrl_c_token.cancel();
        });

        // Run Server
        startup::run(config, token.clone()).await
    })?;

    Ok(())
}
