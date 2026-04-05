//! ApiError convenience helpers, IntoResponse impl, and rejection handling.

use axum::{
    Json,
    extract::rejection::JsonRejection,
    response::{IntoResponse, Response},
};
use tracing::{error, warn};
use uuid::Uuid;

use super::codes::ErrorCode;
use super::types::{ApiError, ErrorDetail, ErrorResponse};

// ============================================================================
// Convenience constructors
// ============================================================================

impl ApiError {
    /// Create error with specific error code
    pub fn with_code(code: ErrorCode, message: impl Into<String>) -> Self {
        ApiError::WithCode(code, message.into())
    }

    /// Create error with code and additional details
    pub fn with_details(
        code: ErrorCode,
        message: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        ApiError::WithCodeAndDetails(code, message.into(), details.into())
    }

    /// Create validation error for specific field
    pub fn validation_field(field: impl Into<String>, message: impl Into<String>) -> Self {
        ApiError::ValidationWithField {
            code: ErrorCode::InvalidInput,
            field: field.into(),
            message: message.into(),
        }
    }

    /// Create general validation error
    pub fn validation_error(message: impl Into<String>, field: Option<&str>) -> Self {
        if let Some(field_name) = field {
            ApiError::ValidationWithField {
                code: ErrorCode::InvalidInput,
                field: field_name.to_string(),
                message: message.into(),
            }
        } else {
            ApiError::with_code(ErrorCode::InvalidInput, message)
        }
    }

    /// Helper: Invalid credentials
    pub fn invalid_credentials() -> Self {
        ApiError::with_code(ErrorCode::InvalidCredentials, "Invalid credentials")
    }

    /// Helper: Token expired
    pub fn token_expired() -> Self {
        ApiError::with_code(ErrorCode::TokenExpired, "Token expired")
    }

    /// Helper: Email not verified
    pub fn email_not_verified() -> Self {
        ApiError::with_code(ErrorCode::EmailNotVerified, "Email not verified")
    }

    /// Helper: Resource not found
    pub fn not_found(resource: &str) -> Self {
        ApiError::with_code(ErrorCode::NotFound, format!("{} not found", resource))
    }

    /// Helper: Resource already exists
    pub fn already_exists(resource: &str) -> Self {
        ApiError::with_code(
            ErrorCode::AlreadyExists,
            format!("{} already exists", resource),
        )
    }

    /// Helper: Invalid wallet address
    pub fn invalid_wallet() -> Self {
        ApiError::with_code(ErrorCode::InvalidWalletAddress, "Invalid wallet address")
    }

    /// Helper: Service unavailable
    pub fn service_unavailable(service: &str) -> Self {
        ApiError::with_details(
            ErrorCode::ServiceUnavailable,
            format!("{} service is currently unavailable", service),
            "Please try again later",
        )
    }

    /// Log error with appropriate level
    fn log_error(&self, request_id: &str) {
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

// ============================================================================
// IntoResponse — converts ApiError to HTTP response
// ============================================================================

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let request_id = Uuid::new_v4().to_string();
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
                    _ => code.message().to_string(),
                },
                details: self.error_details(),
                field: self.error_field(),
            },
            request_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        (status, Json(error_response)).into_response()
    }
}

// ============================================================================
// JSON rejection handling
// ============================================================================

/// Handle Axum JSON rejections and convert to structured API errors
pub fn handle_rejection(err: JsonRejection) -> Response {
    match err {
        JsonRejection::JsonDataError(e) => ApiError::with_details(
            ErrorCode::InvalidInput,
            "Invalid input provided",
            e.to_string(),
        )
        .into_response(),
        JsonRejection::JsonSyntaxError(_) => {
            ApiError::with_code(ErrorCode::InvalidFormat, "Invalid JSON format").into_response()
        }
        JsonRejection::MissingJsonContentType(_) => {
            ApiError::with_code(ErrorCode::InvalidFormat, "JSON content type required")
                .into_response()
        }
        JsonRejection::BytesRejection(_) => {
            ApiError::with_code(ErrorCode::InvalidInput, "Invalid request body format")
                .into_response()
        }
        _ => ApiError::with_details(
            ErrorCode::InvalidInput,
            "Invalid input provided",
            format!("{:?}", err),
        )
        .into_response(),
    }
}
