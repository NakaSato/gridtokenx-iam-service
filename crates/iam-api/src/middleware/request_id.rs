use axum::{
    body::Body,
    http::Request,
    response::Response,
    middleware::Next,
};
use tracing::Span;
use uuid::Uuid;

/// Header name for Request ID
pub const X_REQUEST_ID: &str = "x-request-id";

/// Middleware to add a unique Request ID to every request.
/// It extracts the ID from the incoming `x-request-id` header or generates a new one.
pub async fn request_id_middleware(
    req: Request<Body>,
    next: Next,
) -> Response {
    let request_id = req
        .headers()
        .get(X_REQUEST_ID)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Record the request ID in the current tracing span
    Span::current().record("request_id", &request_id);

    let mut response = next.run(req).await;
    
    // Add the request ID to the response headers
    if let Ok(value) = request_id.parse() {
        response.headers_mut().insert(X_REQUEST_ID, value);
    }
    
    response
}
