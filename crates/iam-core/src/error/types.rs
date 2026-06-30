//! Error type definitions — ApiError enum and response structures.

use axum::http::StatusCode;
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;

use super::codes::ErrorCode;

/// Structured error response
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    /// The error detail payload.
    pub error: ErrorDetail,
    /// Request ID for correlating with server logs.
    pub request_id: String,
    /// RFC3339 timestamp the error was generated.
    pub timestamp: String,
}

/// The body of an `ErrorResponse` — code, message, and optional extras.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorDetail {
    /// Structured error code.
    pub code: ErrorCode,
    /// Numeric form of `code`.
    pub code_number: u16,
    /// Human-readable, client-safe message.
    pub message: String,
    /// Extra context, when available.
    pub details: Option<String>,
    /// Offending field name, for validation errors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

/// Application-level error type. Maps to an HTTP status (`status_code`) and
/// a structured `ErrorCode` for clients; converted to an `axum` response via
/// `IntoResponse`.
#[derive(Debug, Error)]
pub enum ApiError {
    /// Credential check failed.
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// Caller lacks permission for the requested action.
    #[error("Authorization failed: {0}")]
    Authorization(String),

    /// Malformed or invalid request.
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// No valid credentials supplied.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Caller is explicitly denied access.
    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// Input failed validation rules.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Underlying SQL/database failure.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Redis/cache failure.
    #[error("Redis error: {0}")]
    Redis(String),

    /// Solana/Chain Bridge interaction failure.
    #[error("Blockchain error: {0}")]
    Blockchain(String),

    /// A downstream service call failed.
    #[error("External service error: {0}")]
    ExternalService(String),

    /// Server misconfiguration.
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Requested resource doesn't exist.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Operation conflicts with existing state.
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Unclassified server-side fault.
    #[error("Internal server error: {0}")]
    Internal(String),

    /// Caller exceeded a rate limit.
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    /// Carries an explicit `ErrorCode` plus message, for cases not covered
    /// by the named variants above.
    #[error("{1}")]
    WithCode(ErrorCode, String),

    /// Like `WithCode`, plus extra detail text for the client.
    #[error("{1}")]
    WithCodeAndDetails(ErrorCode, String, String),

    /// Validation failure scoped to a specific input field.
    #[error("Validation failed: {field}")]
    ValidationWithField {
        /// Structured error code for the failure.
        code: ErrorCode,
        /// Name of the field that failed validation.
        field: String,
        /// Human-readable validation message.
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
    /// Builds an error with an explicit `ErrorCode` and message.
    pub fn with_code(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::WithCode(code, message.into())
    }

    /// Builds an error with an explicit `ErrorCode`, message, and extra detail.
    pub fn with_details(code: ErrorCode, message: impl Into<String>, details: impl Into<String>) -> Self {
        Self::WithCodeAndDetails(code, message.into(), details.into())
    }

    /// Get error code
    // `WithCode`/`WithCodeAndDetails` bind different field counts, so the
    // arms can't be merged despite both bodies being `*code`.
    #[allow(clippy::match_same_arms)]
    pub(crate) fn error_code(&self) -> ErrorCode {
        match self {
            ApiError::Authentication(_) => ErrorCode::InvalidCredentials,
            ApiError::Authorization(_) => ErrorCode::InsufficientPermissions,
            ApiError::BadRequest(_) | ApiError::Validation(_) => ErrorCode::InvalidInput,
            ApiError::Unauthorized(_) => ErrorCode::TokenMissing,
            ApiError::Forbidden(_) => ErrorCode::ResourceAccessDenied,
            ApiError::NotFound(_) => ErrorCode::NotFound,
            ApiError::Conflict(_) => ErrorCode::Conflict,
            ApiError::Database(_) => ErrorCode::QueryFailed,
            ApiError::Redis(_) | ApiError::ExternalService(_) => ErrorCode::ExternalServiceError,
            ApiError::Blockchain(_) | ApiError::Internal(_) => ErrorCode::InternalServerError,
            ApiError::Configuration(_) => ErrorCode::ConfigurationError,
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
    #[must_use]
    pub fn status_code(&self) -> StatusCode {
        match self {
            ApiError::Authentication(_)
            | ApiError::Unauthorized(_)
            | ApiError::WithCode(
                ErrorCode::InvalidCredentials
                | ErrorCode::TokenExpired
                | ErrorCode::TokenInvalid
                | ErrorCode::TokenMissing
                | ErrorCode::EmailNotVerified,
                _,
            ) => StatusCode::UNAUTHORIZED,

            ApiError::Authorization(_)
            | ApiError::Forbidden(_)
            | ApiError::WithCode(
                ErrorCode::InsufficientPermissions | ErrorCode::ResourceAccessDenied,
                _,
            ) => StatusCode::FORBIDDEN,

            ApiError::BadRequest(_)
            | ApiError::Validation(_)
            | ApiError::ValidationWithField { .. }
            // All validation codes (VAL_3xxx) are client input faults -> 400, never 500.
            // Without these arms InvalidEmail/InvalidFormat/InvalidPassword/etc. fell
            // through the WithCode(_) catch-all to 500, polluting 5xx alarms and
            // mislabeling a bad email as a server error to clients.
            | ApiError::WithCode(
                ErrorCode::InvalidInput
                | ErrorCode::MissingRequiredField
                | ErrorCode::InvalidFormat
                | ErrorCode::InvalidWalletAddress
                | ErrorCode::InvalidAmount
                | ErrorCode::InvalidEmail
                | ErrorCode::InvalidPassword
                | ErrorCode::PasswordTooWeak,
                _,
            ) => StatusCode::BAD_REQUEST,

            ApiError::NotFound(_) | ApiError::WithCode(ErrorCode::NotFound, _) => {
                StatusCode::NOT_FOUND
            }

            ApiError::Conflict(_)
            | ApiError::WithCode(ErrorCode::Conflict | ErrorCode::AlreadyExists, _) => {
                StatusCode::CONFLICT
            }

            ApiError::Blockchain(_)
            | ApiError::ExternalService(_)
            | ApiError::WithCode(
                ErrorCode::ExternalServiceUnavailable | ErrorCode::ServiceUnavailable,
                _,
            ) => StatusCode::BAD_GATEWAY,

            ApiError::RateLimitExceeded(_)
            | ApiError::WithCode(ErrorCode::RateLimitExceeded, _) => StatusCode::TOO_MANY_REQUESTS,

            // A locked account is a client-side condition, not a server fault.
            // Without this arm it fell through the WithCode(_) catch-all to 500,
            // which polluted 5xx alarms and hid the real cause from clients.
            ApiError::WithCode(ErrorCode::AccountLocked, _) => StatusCode::LOCKED,

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
    #[must_use]
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
    #[must_use]
    pub fn service_unavailable(service: &str) -> Self {
        ApiError::WithCodeAndDetails(
            ErrorCode::ServiceUnavailable,
            format!("{service} service is currently unavailable"),
            "Please try again later".to_string(),
        )
    }

    /// Helper for resource already exists
    #[must_use]
    pub fn already_exists(resource: &str) -> Self {
        ApiError::WithCode(
            ErrorCode::AlreadyExists,
            format!("{resource} already exists"),
        )
    }

    /// Helper for resource not found
    #[must_use]
    pub fn not_found(resource: &str) -> Self {
        ApiError::WithCode(ErrorCode::NotFound, format!("{resource} not found"))
    }

    /// Helper: Invalid wallet address
    #[must_use]
    pub fn invalid_wallet() -> Self {
        ApiError::WithCode(ErrorCode::InvalidWalletAddress, "Invalid wallet address".to_string())
    }

    /// Helper: Email not verified
    #[must_use]
    pub fn email_not_verified() -> Self {
        ApiError::WithCode(ErrorCode::EmailNotVerified, "Email not verified".to_string())
    }

    /// Helper: Token expired
    #[must_use]
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
                    ApiError::WithCode(_, msg)
                    | ApiError::WithCodeAndDetails(_, msg, _)
                    | ApiError::BadRequest(msg)
                    | ApiError::Validation(msg)
                    | ApiError::Internal(msg) => msg.clone(),
                    ApiError::ValidationWithField { message, .. } => message.clone(),
                    _ => code.message().to_string(),
                },
                details: self.error_details(),
                field: self.error_field(),
            },
            request_id,
            timestamp: gridtokenx_telemetry::time::now().to_rfc3339(),
        };

        (status, axum::Json(error_response)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_locked_maps_to_423_not_500() {
        // Regression: AccountLocked used to fall through the WithCode(_) catch-all
        // to 500, masking a client-side lockout as a server fault.
        let err = ApiError::with_code(ErrorCode::AccountLocked, "locked");
        assert_eq!(err.status_code(), StatusCode::LOCKED);
        assert!(!err.status_code().is_server_error(), "lockout must not be a 5xx");
    }

    #[test]
    fn validation_codes_map_to_400_not_500() {
        // Regression: VAL_3xxx codes other than InvalidInput/InvalidWalletAddress/
        // InvalidAmount used to fall through the WithCode(_) catch-all to 500, so a
        // bad email/password surfaced as a server fault and polluted 5xx alarms.
        for code in [
            ErrorCode::InvalidInput,
            ErrorCode::MissingRequiredField,
            ErrorCode::InvalidFormat,
            ErrorCode::InvalidWalletAddress,
            ErrorCode::InvalidAmount,
            ErrorCode::InvalidEmail,
            ErrorCode::InvalidPassword,
            ErrorCode::PasswordTooWeak,
        ] {
            let err = ApiError::with_code(code, "bad input");
            assert_eq!(
                err.status_code(),
                StatusCode::BAD_REQUEST,
                "{code:?} must map to 400",
            );
            assert!(!err.status_code().is_server_error(), "{code:?} must not be 5xx");
        }
    }

    #[test]
    fn auth_and_rate_limit_statuses_unchanged() {
        assert_eq!(ApiError::invalid_credentials().status_code(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            ApiError::WithCode(ErrorCode::RateLimitExceeded, "slow down".to_string()).status_code(),
            StatusCode::TOO_MANY_REQUESTS,
        );
        // A genuinely internal WithCode still maps to 500.
        assert_eq!(
            ApiError::Internal("boom".to_string()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR,
        );
    }
}
