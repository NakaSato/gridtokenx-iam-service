//! Error handling primitives and standard API error types.
//!
//! This module provides a unified error handling system for the entire workspace,
//! including structured error codes and conversion from third-party errors.

pub mod codes;
pub mod types;

// Re-export everything for backward compatibility
pub use codes::ErrorCode;
pub use types::{ApiError, ErrorDetail, ErrorResponse};

/// A specialized Result type for IAM operations.
pub type Result<T> = std::result::Result<T, ApiError>;
