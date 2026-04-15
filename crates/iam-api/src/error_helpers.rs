//! rejected content-type and JSON handling for Axum.

use axum::{
    response::{IntoResponse, Response},
    extract::rejection::JsonRejection,
};
use iam_core::error::codes::ErrorCode;
use iam_core::error::types::ApiError;

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
