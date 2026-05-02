//! Library for the GridTokenX IAM service.
//! 
//! This crate contains the main entry point and startup logic for the IAM service,
//! including both REST and gRPC servers.

/// Service startup and initialization logic.
pub mod startup;

/// Observability and telemetry (logging, tracing, metrics).
pub mod telemetry;
