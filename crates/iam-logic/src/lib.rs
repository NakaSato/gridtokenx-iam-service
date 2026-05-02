//! Business logic and domain services for the GridTokenX IAM service.
//!
//! This crate implements the core business rules and workflows, such as user authentication,
//! password management, and blockchain integration, independent of specific I/O implementations.

/// Core authentication and user management service.
pub mod auth_service;
/// Password hashing and verification utilities.
pub mod password;
/// JWT and API key token management services.
pub mod jwt_service;
/// Provider for blockchain-specific operations.
pub mod blockchain_provider;

pub use auth_service::AuthService;
pub use jwt_service::{JwtService, ApiKeyService};
pub use password::PasswordService;

#[cfg(test)]
mod auth_service_tests;
