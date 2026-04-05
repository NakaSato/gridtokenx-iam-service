// GridTokenX IAM Service Error Handling

pub mod codes;
pub mod helpers;
pub mod types;

// Re-export everything for backward compatibility
pub use codes::ErrorCode;
pub use helpers::handle_rejection;
pub use types::{ApiError, ErrorDetail, ErrorResponse};

pub type Result<T> = std::result::Result<T, ApiError>;
