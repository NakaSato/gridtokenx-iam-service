// GridTokenX IAM Service Error Codes

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Error codes for categorizing errors
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, ToSchema)]
pub enum ErrorCode {
    // Authentication errors (1xxx)
    #[serde(rename = "AUTH_1001")]
    InvalidCredentials,
    #[serde(rename = "AUTH_1002")]
    TokenExpired,
    #[serde(rename = "AUTH_1003")]
    TokenInvalid,
    #[serde(rename = "AUTH_1004")]
    TokenMissing,
    #[serde(rename = "AUTH_1005")]
    EmailNotVerified,
    #[serde(rename = "AUTH_1006")]
    AccountLocked,
    #[serde(rename = "AUTH_1007")]
    AccountDisabled,

    // Authorization errors (2xxx)
    #[serde(rename = "AUTHZ_2001")]
    InsufficientPermissions,
    #[serde(rename = "AUTHZ_2002")]
    ResourceAccessDenied,
    #[serde(rename = "AUTHZ_2003")]
    RoleNotAuthorized,

    // Validation errors (3xxx)
    #[serde(rename = "VAL_3001")]
    InvalidInput,
    #[serde(rename = "VAL_3002")]
    MissingRequiredField,
    #[serde(rename = "VAL_3003")]
    InvalidFormat,
    #[serde(rename = "VAL_3004")]
    InvalidWalletAddress,
    #[serde(rename = "VAL_3005")]
    InvalidAmount,
    #[serde(rename = "VAL_3006")]
    InvalidEmail,
    #[serde(rename = "VAL_3007")]
    InvalidPassword,
    #[serde(rename = "VAL_3008")]
    PasswordTooWeak,

    // Resource errors (4xxx)
    #[serde(rename = "RES_4001")]
    NotFound,
    #[serde(rename = "RES_4002")]
    AlreadyExists,
    #[serde(rename = "RES_4003")]
    Conflict,
    #[serde(rename = "RES_4004")]
    Gone,

    // Database errors (7xxx)
    #[serde(rename = "DB_7001")]
    DatabaseConnectionFailed,
    #[serde(rename = "DB_7002")]
    QueryFailed,
    #[serde(rename = "DB_7003")]
    DatabaseTransactionFailed,
    #[serde(rename = "DB_7004")]
    ConstraintViolation,

    // External service errors (8xxx)
    #[serde(rename = "EXT_8001")]
    ExternalServiceUnavailable,
    #[serde(rename = "EXT_8002")]
    ExternalServiceTimeout,
    #[serde(rename = "EXT_8003")]
    ExternalServiceError,
    #[serde(rename = "EXT_8004")]
    EmailServiceFailed,
    #[serde(rename = "EXT_8005")]
    ServiceUnavailable,

    // Rate Limiting (9xxx)
    #[serde(rename = "RATE_9001")]
    RateLimitExceeded,
    #[serde(rename = "RATE_9002")]
    TooManyRequests,

    // Internal errors (9xxx)
    #[serde(rename = "INT_9999")]
    InternalServerError,
    #[serde(rename = "INT_9998")]
    ConfigurationError,
    #[serde(rename = "INT_9997")]
    UnexpectedError,
}

impl ErrorCode {
    /// Get numeric code
    pub fn code(&self) -> u16 {
        match self {
            // Authentication
            ErrorCode::InvalidCredentials => 1001,
            ErrorCode::TokenExpired => 1002,
            ErrorCode::TokenInvalid => 1003,
            ErrorCode::TokenMissing => 1004,
            ErrorCode::EmailNotVerified => 1005,
            ErrorCode::AccountLocked => 1006,
            ErrorCode::AccountDisabled => 1007,

            // Authorization
            ErrorCode::InsufficientPermissions => 2001,
            ErrorCode::ResourceAccessDenied => 2002,
            ErrorCode::RoleNotAuthorized => 2003,

            // Validation
            ErrorCode::InvalidInput => 3001,
            ErrorCode::MissingRequiredField => 3002,
            ErrorCode::InvalidFormat => 3003,
            ErrorCode::InvalidWalletAddress => 3004,
            ErrorCode::InvalidAmount => 3005,
            ErrorCode::InvalidEmail => 3006,
            ErrorCode::InvalidPassword => 3007,
            ErrorCode::PasswordTooWeak => 3008,

            // Resource
            ErrorCode::NotFound => 4001,
            ErrorCode::AlreadyExists => 4002,
            ErrorCode::Conflict => 4003,
            ErrorCode::Gone => 4004,

            // Database
            ErrorCode::DatabaseConnectionFailed => 7001,
            ErrorCode::QueryFailed => 7002,
            ErrorCode::DatabaseTransactionFailed => 7003,
            ErrorCode::ConstraintViolation => 7004,

            // External Service
            ErrorCode::ExternalServiceUnavailable => 8001,
            ErrorCode::ExternalServiceTimeout => 8002,
            ErrorCode::ExternalServiceError => 8003,
            ErrorCode::EmailServiceFailed => 8004,
            ErrorCode::ServiceUnavailable => 8005,

            // Rate Limiting
            ErrorCode::RateLimitExceeded => 9001,
            ErrorCode::TooManyRequests => 9002,

            // Internal
            ErrorCode::InternalServerError => 9999,
            ErrorCode::ConfigurationError => 9998,
            ErrorCode::UnexpectedError => 9997,
        }
    }

    /// Get user-friendly message
    pub fn message(&self) -> &'static str {
        match self {
            // Authentication
            ErrorCode::InvalidCredentials => "Invalid email or password",
            ErrorCode::TokenExpired => "Your session has expired. Please log in again",
            ErrorCode::TokenInvalid => "Invalid authentication token",
            ErrorCode::TokenMissing => "Authentication required. Please log in",
            ErrorCode::EmailNotVerified => "Please verify your email address before proceeding",
            ErrorCode::AccountLocked => "Your account has been locked. Please contact support",
            ErrorCode::AccountDisabled => "Your account has been disabled. Please contact support",

            // Authorization
            ErrorCode::InsufficientPermissions => {
                "You don't have permission to perform this action"
            }
            ErrorCode::ResourceAccessDenied => "Access to this resource is denied",
            ErrorCode::RoleNotAuthorized => "Your role is not authorized for this action",

            // Validation
            ErrorCode::InvalidInput => "Invalid input provided",
            ErrorCode::MissingRequiredField => "Required field is missing",
            ErrorCode::InvalidFormat => "Invalid format provided",
            ErrorCode::InvalidWalletAddress => "Invalid wallet address format",
            ErrorCode::InvalidAmount => "Invalid amount provided",
            ErrorCode::InvalidEmail => "Invalid email address format",
            ErrorCode::InvalidPassword => "Invalid password",
            ErrorCode::PasswordTooWeak => {
                "Password is too weak. Use at least 8 characters with letters and numbers"
            }

            // Resource
            ErrorCode::NotFound => "The requested resource was not found",
            ErrorCode::AlreadyExists => "This resource already exists",
            ErrorCode::Conflict => "A conflict occurred with an existing resource",
            ErrorCode::Gone => "This resource is no longer available",

            // Database
            ErrorCode::DatabaseConnectionFailed => "Database connection failed",
            ErrorCode::QueryFailed => "Database query failed",
            ErrorCode::DatabaseTransactionFailed => "Database transaction failed",
            ErrorCode::ConstraintViolation => "Database constraint violation",

            // External Service
            ErrorCode::ExternalServiceUnavailable => "External service is currently unavailable",
            ErrorCode::ExternalServiceTimeout => "External service request timed out",
            ErrorCode::ExternalServiceError => "External service error occurred",
            ErrorCode::EmailServiceFailed => "Failed to send email",
            ErrorCode::ServiceUnavailable => "Service is currently unavailable",

            // Rate Limiting
            ErrorCode::RateLimitExceeded => "Rate limit exceeded. Please try again later",
            ErrorCode::TooManyRequests => "Too many requests. Please slow down",

            // Internal
            ErrorCode::InternalServerError => "An internal server error occurred",
            ErrorCode::ConfigurationError => "Server configuration error",
            ErrorCode::UnexpectedError => "An unexpected error occurred",
        }
    }
}
