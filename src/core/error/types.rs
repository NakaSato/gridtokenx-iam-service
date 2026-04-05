//! Error type definitions — ApiError enum and response structures.

use axum::http::StatusCode;
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;

use super::codes::ErrorCode;

/// Structured error response
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
    pub request_id: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorDetail {
    pub code: ErrorCode,
    pub code_number: u16,
    pub message: String,
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Authorization failed: {0}")]
    Authorization(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    Redis(String),

    #[error("Blockchain error: {0}")]
    Blockchain(String),

    #[error("External service error: {0}")]
    ExternalService(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    // Enhanced error types with codes
    #[error("{1}")]
    WithCode(ErrorCode, String),

    #[error("{1}")]
    WithCodeAndDetails(ErrorCode, String, String),

    #[error("Validation failed: {field}")]
    ValidationWithField {
        code: ErrorCode,
        field: String,
        message: String,
    },
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        match err.downcast::<ApiError>() {
            Ok(api_err) => api_err,
            Err(e) => ApiError::Internal(e.to_string()),
        }
    }
}

impl ApiError {
    /// Get error code
    pub(crate) fn error_code(&self) -> ErrorCode {
        match self {
            ApiError::Authentication(_) => ErrorCode::InvalidCredentials,
            ApiError::Authorization(_) => ErrorCode::InsufficientPermissions,
            ApiError::BadRequest(_) => ErrorCode::InvalidInput,
            ApiError::Unauthorized(_) => ErrorCode::TokenMissing,
            ApiError::Forbidden(_) => ErrorCode::ResourceAccessDenied,
            ApiError::Validation(_) => ErrorCode::InvalidInput,
            ApiError::NotFound(_) => ErrorCode::NotFound,
            ApiError::Conflict(_) => ErrorCode::Conflict,
            ApiError::Database(_) => ErrorCode::QueryFailed,
            ApiError::Redis(_) => ErrorCode::ExternalServiceError,
            ApiError::Blockchain(_) => ErrorCode::InternalServerError,
            ApiError::ExternalService(_) => ErrorCode::ExternalServiceError,
            ApiError::Configuration(_) => ErrorCode::ConfigurationError,
            ApiError::Internal(_) => ErrorCode::InternalServerError,
            ApiError::RateLimitExceeded(_) => ErrorCode::RateLimitExceeded,
            ApiError::WithCode(code, _) => *code,
            ApiError::WithCodeAndDetails(code, _, _) => *code,
            ApiError::ValidationWithField { code, .. } => *code,
        }
    }

    /// Get error details
    pub(crate) fn error_details(&self) -> Option<String> {
        match self {
            ApiError::WithCodeAndDetails(_, _, details) => Some(details.clone()),
            _ => None,
        }
    }

    /// Get field name for validation errors
    pub(crate) fn error_field(&self) -> Option<String> {
        match self {
            ApiError::ValidationWithField { field, .. } => Some(field.clone()),
            _ => None,
        }
    }

    /// Get status code
    pub(crate) fn status_code(&self) -> StatusCode {
        match self {
            ApiError::Authentication(_)
            | ApiError::Unauthorized(_)
            | ApiError::WithCode(ErrorCode::InvalidCredentials, _)
            | ApiError::WithCode(ErrorCode::TokenExpired, _)
            | ApiError::WithCode(ErrorCode::TokenInvalid, _)
            | ApiError::WithCode(ErrorCode::TokenMissing, _)
            | ApiError::WithCode(ErrorCode::EmailNotVerified, _) => StatusCode::UNAUTHORIZED,

            ApiError::Authorization(_)
            | ApiError::Forbidden(_)
            | ApiError::WithCode(ErrorCode::InsufficientPermissions, _)
            | ApiError::WithCode(ErrorCode::ResourceAccessDenied, _) => StatusCode::FORBIDDEN,

            ApiError::BadRequest(_)
            | ApiError::Validation(_)
            | ApiError::ValidationWithField { .. }
            | ApiError::WithCode(ErrorCode::InvalidInput, _)
            | ApiError::WithCode(ErrorCode::InvalidWalletAddress, _)
            | ApiError::WithCode(ErrorCode::InvalidAmount, _) => StatusCode::BAD_REQUEST,

            ApiError::NotFound(_) | ApiError::WithCode(ErrorCode::NotFound, _) => {
                StatusCode::NOT_FOUND
            }

            ApiError::Conflict(_)
            | ApiError::WithCode(ErrorCode::Conflict, _)
            | ApiError::WithCode(ErrorCode::AlreadyExists, _) => StatusCode::CONFLICT,

            ApiError::Blockchain(_)
            | ApiError::ExternalService(_)
            | ApiError::WithCode(ErrorCode::ExternalServiceUnavailable, _)
            | ApiError::WithCode(ErrorCode::ServiceUnavailable, _) => StatusCode::BAD_GATEWAY,

            ApiError::RateLimitExceeded(_)
            | ApiError::WithCode(ErrorCode::RateLimitExceeded, _) => StatusCode::TOO_MANY_REQUESTS,

            ApiError::Database(_)
            | ApiError::Redis(_)
            | ApiError::Configuration(_)
            | ApiError::Internal(_)
            | ApiError::WithCode(_, _)
            | ApiError::WithCodeAndDetails(_, _, _) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
