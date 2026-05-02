//! Simplified telemetry for GridTokenX IAM service (standard logging only).

use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// holds the telemetry provider (kept as unit for backward compatibility)
/// Guard that manages the lifecycle of the telemetry system.
/// Currently a placeholder for future complex teardown logic.
#[derive(Debug)]
pub struct TelemetryGuard;

impl TelemetryGuard {
    /// Shuts down the telemetry system and flushes any pending logs.
    pub fn shutdown(&self) {}
}

/// Initialize tracing and set up the global subscriber for JSON logging.
///
/// `_service_name_default`: Kept for backward compatibility with calls.
pub fn init_telemetry(_service_name_default: &'static str) -> TelemetryGuard {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    tracing::info!("Tracing initialized (JSON logging enabled)");
    
    TelemetryGuard
}

/// Graceful shutdown — placeholder for backward compatibility.
pub fn shutdown_telemetry(_guard: &TelemetryGuard) {
    tracing::info!("Telemetry shutdown sequence (JSON logger requires no explicit teardown)");
}
