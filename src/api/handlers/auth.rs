use crate::api::handlers::types::{
    AuthResponse, LoginRequest, RegistrationRequest, RegistrationResponse, VerifyEmailResponse,
};
use crate::api::middleware::metrics;
use crate::core::error::Result;
use crate::services::AuthService;
use axum::{Json, extract::State};
use std::time::Instant;

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    request_body = RegistrationRequest,
    responses(
        (status = 200, description = "Registration successful", body = RegistrationResponse),
        (status = 409, description = "Conflict - User already exists"),
        (status = 500, description = "Internal server error")
    ),
    tag = "auth"
)]
pub async fn register(
    State(auth_service): State<AuthService>,
    Json(request): Json<RegistrationRequest>,
) -> Result<Json<RegistrationResponse>> {
    let start = Instant::now();
    let result = auth_service.register(request).await;
    let _duration_ms = start.elapsed().as_secs_f64() * 1000.0;
    
    let success = result.is_ok();
    metrics::record_user_operation("register", success);
    
    if !success {
        metrics::record_auth_failure("register", "registration_error");
    }
    
    let response = result?;
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/token",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Unauthorized - Invalid credentials"),
        (status = 500, description = "Internal server error")
    ),
    tag = "auth"
)]
pub async fn login(
    State(auth_service): State<AuthService>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<AuthResponse>> {
    let start = Instant::now();
    let result = auth_service.login(request).await;
    let _duration_ms = start.elapsed().as_secs_f64() * 1000.0;
    
    let success = result.is_ok();
    metrics::record_auth_attempt("login", success);
    metrics::record_user_operation("login", success);
    
    if !success {
        metrics::record_auth_failure("login", "invalid_credentials");
    }
    
    let response = result?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/verify",
    params(
        ("token" = String, Query, description = "Verification token")
    ),
    responses(
        (status = 200, description = "Verification successful", body = VerifyEmailResponse),
        (status = 400, description = "Invalid token"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "auth"
)]
pub async fn verify(
    State(auth_service): State<AuthService>,
    axum::extract::Query(params): axum::extract::Query<crate::api::handlers::types::VerifyEmailRequest>,
) -> Result<Json<crate::api::handlers::types::VerifyEmailResponse>> {
    let start = Instant::now();
    let result = auth_service.verify_email(params).await;
    let _duration_ms = start.elapsed().as_secs_f64() * 1000.0;
    
    let success = result.is_ok();
    metrics::record_user_operation("verify", success);
    
    let response = result?;
    Ok(Json(response))
}
