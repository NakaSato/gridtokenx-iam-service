//! Business logic and domain services for the GridTokenX IAM service.
//!
//! This crate implements the core business rules and workflows, such as user authentication,
//! password management, and blockchain integration, independent of specific I/O implementations.

pub mod auth_service;
pub mod password;
pub mod jwt_service;
pub mod blockchain_provider;

pub use auth_service::AuthService;
pub use jwt_service::{JwtService, ApiKeyService};
pub use password::PasswordService;

#[cfg(test)]
mod auth_service_tests;
