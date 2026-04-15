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
    pub fn with_code(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::WithCode(code, message.into())
    }

    pub fn with_details(code: ErrorCode, message: impl Into<String>, details: impl Into<String>) -> Self {
        Self::WithCodeAndDetails(code, message.into(), details.into())
    }

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
    pub fn status_code(&self) -> StatusCode {
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

    /// Helper for internal server errors
    pub fn internal(message: impl Into<String>) -> Self {
        ApiError::Internal(message.into())
    }

    /// Helper for invalid credentials (common in auth)
    pub fn invalid_credentials() -> Self {
        ApiError::WithCode(ErrorCode::InvalidCredentials, "Invalid username or password".to_string())
    }

    /// Helper for unauthorized errors
    pub fn unauthorized(message: impl Into<String>) -> Self {
        ApiError::Unauthorized(message.into())
    }

    /// Helper for validation errors
    pub fn validation(message: impl Into<String>) -> Self {
        ApiError::Validation(message.into())
    }

    /// Helper for service unavailable
    pub fn service_unavailable(service: &str) -> Self {
        ApiError::WithCodeAndDetails(
            ErrorCode::ServiceUnavailable,
            format!("{} service is currently unavailable", service),
            "Please try again later".to_string(),
        )
    }

    /// Helper for resource already exists
    pub fn already_exists(resource: &str) -> Self {
        ApiError::WithCode(
            ErrorCode::AlreadyExists,
            format!("{} already exists", resource),
        )
    }

    /// Helper for resource not found
    pub fn not_found(resource: &str) -> Self {
        ApiError::WithCode(ErrorCode::NotFound, format!("{} not found", resource))
    }

    /// Helper: Invalid wallet address
    pub fn invalid_wallet() -> Self {
        ApiError::WithCode(ErrorCode::InvalidWalletAddress, "Invalid wallet address".to_string())
    }

    /// Helper: Email not verified
    pub fn email_not_verified() -> Self {
        ApiError::WithCode(ErrorCode::EmailNotVerified, "Email not verified".to_string())
    }

    /// Helper: Token expired
    pub fn token_expired() -> Self {
        ApiError::WithCode(ErrorCode::TokenExpired, "Token expired".to_string())
    }

    /// Create validation error for specific field
    pub fn validation_field(field: impl Into<String>, message: impl Into<String>) -> Self {
        ApiError::ValidationWithField {
            code: ErrorCode::InvalidInput,
            field: field.into(),
            message: message.into(),
        }
    }

    /// Log error with appropriate level
    fn log_error(&self, request_id: &str) {
        use tracing::{error, warn};
        match self.status_code() {
            status if status.is_server_error() => {
                error!(
                    request_id = %request_id,
                    error = %self,
                    "Server error occurred"
                );
            }
            status if status.is_client_error() => {
                warn!(
                    request_id = %request_id,
                    error = %self,
                    "Client error occurred"
                );
            }
            _ => {}
        }
    }
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let request_id = uuid::Uuid::new_v4().to_string();
        let status = self.status_code();
        let code = self.error_code();

        // Log the error
        self.log_error(&request_id);

        // Build error response
        let error_response = ErrorResponse {
            error: ErrorDetail {
                code,
                code_number: code.code(),
                message: match &self {
                    ApiError::WithCode(_, msg) | ApiError::WithCodeAndDetails(_, msg, _) => {
                        msg.clone()
                    }
                    ApiError::BadRequest(msg) => msg.clone(),
                    ApiError::ValidationWithField { message, .. } => message.clone(),
                    ApiError::Validation(msg) => msg.clone(),
                    ApiError::Internal(msg) => msg.clone(),
                    _ => code.message().to_string(),
                },
                details: self.error_details(),
                field: self.error_field(),
            },
            request_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        (status, axum::Json(error_response)).into_response()
    }
}
