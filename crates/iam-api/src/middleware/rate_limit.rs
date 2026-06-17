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

/// Maps a request path to its `(limit, window_secs)` rate-limit budget.
///
/// NB: this middleware is layered on the auth sub-router *before* it is nested
/// under `/api/v1/auth`, so the path seen at request time is prefix-stripped
/// (`/login`, `/register`) — NOT the full `/api/v1/auth/login`. Match on the
/// suffix; matching the full path silently fell through to the 100/60 default,
/// leaving the auth endpoints effectively unthrottled.
fn limits_for_path(path: &str) -> (u64, u64) {
    match path {
        p if p.ends_with("/login") => (10, 60), // 10 attempts per minute
        p if p.ends_with("/register") => (5, 3600), // 5 registrations per hour
        _ => (100, 60),                          // General limit
    }
}

/// Middleware to enforce IP-based rate limiting on specific endpoints.
pub async fn rate_limit_middleware(
    State(auth_service): State<AuthService>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = addr.ip().to_string();
    let path = req.uri().path().to_string();
    
    let (limit, window) = limits_for_path(&path);

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

#[cfg(test)]
mod tests {
    use super::limits_for_path;

    #[test]
    fn nested_stripped_paths_map_to_intended_budgets() {
        // The router nests under /api/v1/auth, so the middleware sees /login, /register.
        assert_eq!(limits_for_path("/login"), (10, 60));
        assert_eq!(limits_for_path("/register"), (5, 3600));
    }

    #[test]
    fn full_paths_still_match_by_suffix() {
        // Defense in depth: a full (un-stripped) path must resolve identically.
        assert_eq!(limits_for_path("/api/v1/auth/login"), (10, 60));
        assert_eq!(limits_for_path("/api/v1/auth/register"), (5, 3600));
    }

    #[test]
    fn other_paths_use_general_default() {
        assert_eq!(limits_for_path("/verify"), (100, 60));
        assert_eq!(limits_for_path("/forgot-password"), (100, 60));
    }
}
