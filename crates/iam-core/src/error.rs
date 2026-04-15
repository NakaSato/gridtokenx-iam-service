// GridTokenX IAM Service Error Handling

pub mod codes;
pub mod types;

// Re-export everything for backward compatibility
pub use codes::ErrorCode;
pub use types::{ApiError, ErrorDetail, ErrorResponse};

pub type Result<T> = std::result::Result<T, ApiError>;
