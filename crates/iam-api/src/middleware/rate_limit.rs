use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    extract::{State, ConnectInfo},
};
use std::net::SocketAddr;
use iam_logic::AuthService;
use iam_core::error::ApiError;

/// Middleware to enforce IP-based rate limiting on specific endpoints.
pub async fn rate_limit_middleware(
    State(auth_service): State<AuthService>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = addr.ip().to_string();
    let path = req.uri().path().to_string();
    
    // Default limits
    let (limit, window) = match path.as_str() {
        p if p.contains("/auth/login") => (10, 60), // 10 attempts per minute
        p if p.contains("/auth/register") => (5, 3600), // 5 registrations per hour
        _ => (100, 60), // General limit
    };

    match auth_service.check_rate_limit(&ip, &path, limit, window).await {
        Ok(_) => Ok(next.run(req).await),
        Err(e) => {
            tracing::warn!("Rate limit hit for IP {}: {}", ip, e);
            // We return the error as a response
            // Convert ApiError to response
            // Since this is a middleware, we might need to return a Response directly or a compatible Result
            
            // For simplicity in this environment, we'll return a 429 status code
            Err(StatusCode::TOO_MANY_REQUESTS)
        }
    }
}
