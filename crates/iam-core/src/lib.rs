//! Core primitives, domain models, and traits for the `GridTokenX` IAM service.
//!
//! This crate contains the shared logic and definitions that are used by all other crates
//! in the IAM workspace, following a "Sync Core" design pattern.

/// Error types and codes shared across the IAM workspace.
pub mod error;
/// Domain models for the IAM service (users, wallets, roles, events, API keys).
pub mod domain;
/// Trait definitions (the DI contracts) for repositories, services, and infrastructure.
pub mod traits;
/// `Config::from_env` — the service's environment-variable configuration source.
pub mod config;

pub use error::ApiError;
pub use traits::*;
