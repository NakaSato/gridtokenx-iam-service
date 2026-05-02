//! API layer for the IAM service.
//! 
//! This crate contains the REST handlers, gRPC service implementations,
//! and custom middleware for the GridTokenX Identity and Access Management system.

/// HTTP request handlers for authentication and user management.
pub mod handlers;
/// Custom Axum middleware (metrics, rate limiting, etc.).
pub mod middleware;
/// ConnectRPC gRPC service implementation.
pub mod identity_grpc;
/// Helpers for mapping errors and rejections to HTTP responses.
pub mod error_helpers;

pub use identity_grpc::IdentityGrpcService;
pub use error_helpers::handle_rejection;
