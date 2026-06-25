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

/// Parse a `"limit,window_secs"` env override into `(limit, window)`.
///
/// Returns the `default` when the var is unset, empty, or malformed — so a
/// fat-fingered value fails safe to the built-in budget rather than disabling
/// the limiter. Lets dev/large-fleet onboarding (e.g. an 80-meter simulator that
/// would otherwise exhaust the 5/hour register budget on one boot) raise the cap
/// via env without a code change, while prod keeps the tight default by omitting it.
fn env_limit(var: &str, default: (u64, u64)) -> (u64, u64) {
    match std::env::var(var) {
        Ok(raw) => parse_limit(&raw, default),
        Err(_) => default,
    }
}

/// Parse `"limit,window_secs"` into `(limit, window)`, falling back to `default`
/// when empty/malformed/non-positive (fail-safe to the built-in budget).
fn parse_limit(raw: &str, default: (u64, u64)) -> (u64, u64) {
    let mut parts = raw.trim().splitn(2, ',');
    match (
        parts.next().map(str::trim).and_then(|s| s.parse::<u64>().ok()),
        parts.next().map(str::trim).and_then(|s| s.parse::<u64>().ok()),
    ) {
        (Some(limit), Some(window)) if limit > 0 && window > 0 => (limit, window),
        _ => default,
    }
}

/// Maps a request path to its `(limit, window_secs)` rate-limit budget.
///
/// NB: this middleware is layered on the auth sub-router *before* it is nested
/// under `/api/v1/auth`, so the path seen at request time is prefix-stripped
/// (`/login`, `/register`) — NOT the full `/api/v1/auth/login`. Match on the
/// suffix; matching the full path silently fell through to the 100/60 default,
/// leaving the auth endpoints effectively unthrottled.
///
/// The `/login` and `/register` budgets are env-overridable
/// (`IAM_LOGIN_LIMIT` / `IAM_REGISTER_LIMIT`, each `"limit,window_secs"`) so a
/// dev fleet onboard can lift the tight prod defaults without a rebuild.
fn limits_for_path(path: &str) -> (u64, u64) {
    match path {
        p if p.ends_with("/login") => env_limit("IAM_LOGIN_LIMIT", (10, 60)),
        p if p.ends_with("/register") => env_limit("IAM_REGISTER_LIMIT", (5, 3600)),
        _ => (100, 60), // General limit
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

    #[test]
    fn parse_limit_overrides_and_falls_back() {
        use super::parse_limit;
        let def = (5, 3600);
        // Valid override.
        assert_eq!(parse_limit("10000,3600", def), (10000, 3600));
        assert_eq!(parse_limit("  20 , 60 ", def), (20, 60));
        // Malformed / empty / non-positive → default (fail-safe).
        assert_eq!(parse_limit("", def), def);
        assert_eq!(parse_limit("nope", def), def);
        assert_eq!(parse_limit("10", def), def); // missing window
        assert_eq!(parse_limit("0,60", def), def); // zero limit
        assert_eq!(parse_limit("10,0", def), def); // zero window
    }
}
