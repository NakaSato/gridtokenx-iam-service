use gridtokenx_blockchain_core::auth::ServiceRole;
use crate::handlers::types::{
    AuthResponse, LoginRequest, RegistrationRequest, RegistrationResponse, VerifyEmailResponse,
    UserResponse, ForgotPasswordRequest, ForgotPasswordResponse, ResetPasswordRequest, ResetPasswordResponse,
    AuthenticatedUser,
};
use crate::middleware::metrics;
use iam_core::error::Result as ApiResult;
use iam_core::domain::identity::Claims;
use iam_logic::AuthService;
use axum::{Json, extract::State};
use tracing::instrument;
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
    role: ServiceRole,
    State(auth_service): State<AuthService>,
    Json(request): Json<RegistrationRequest>,
) -> ApiResult<Json<RegistrationResponse>> {
    // Allow public registration from Gateway or Unknown
    role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin, ServiceRole::Unknown])
        .map_err(|(_code, msg)| iam_core::error::ApiError::Unauthorized(msg.to_string()))?;

    let start = Instant::now();
    let result = auth_service.register(
        request.username,
        request.email,
        request.password,
        request.first_name,
        request.last_name,
    ).await;
    let _duration_ms = start.elapsed().as_secs_f64() * 1000.0;
    
    let success = result.is_ok();
    metrics::record_user_operation("register", success);
    
    if !success {
        metrics::record_auth_failure("register", "registration_error");
    }
    
    let response = result?;
    Ok(Json(RegistrationResponse {
        id: response.id,
        username: response.username,
        email: response.email,
        first_name: response.first_name,
        last_name: response.last_name,
        message: response.message,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Unauthorized - Invalid credentials"),
        (status = 500, description = "Internal server error")
    ),
    tag = "auth"
)]
pub async fn login(
    role: ServiceRole,
    State(auth_service): State<AuthService>,
    Json(request): Json<LoginRequest>,
) -> ApiResult<Json<AuthResponse>> {
    // Allow public login from Gateway or Unknown
    role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin, ServiceRole::Unknown])
        .map_err(|(_code, msg)| iam_core::error::ApiError::Unauthorized(msg.to_string()))?;

    let start = Instant::now();
    let result = auth_service.login(request.username, request.password).await;
    let _duration_ms = start.elapsed().as_secs_f64() * 1000.0;
    
    let success = result.is_ok();
    metrics::record_auth_attempt("login", success);
    metrics::record_user_operation("login", success);
    
    if !success {
        metrics::record_auth_failure("login", "invalid_credentials");
    }
    
    let response = result?;
    Ok(Json(AuthResponse {
        access_token: response.access_token,
        expires_in: response.expires_in,
        user: UserResponse {
            id: response.user.id,
            username: response.user.username,
            email: response.user.email,
            role: response.user.role,
            first_name: response.user.first_name,
            last_name: response.user.last_name,
            wallet_address: response.user.wallet_address,
        },
    }))
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
    role: ServiceRole,
    State(auth_service): State<AuthService>,
    axum::extract::Query(params): axum::extract::Query<crate::handlers::types::VerifyEmailRequest>,
) -> ApiResult<Json<VerifyEmailResponse>> {
    // Verification is public
    role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin, ServiceRole::Unknown])
        .map_err(|(_code, msg)| iam_core::error::ApiError::Unauthorized(msg.to_string()))?;

    let start = Instant::now();
    let result = auth_service.verify_email(params.token).await;
    let _duration_ms = start.elapsed().as_secs_f64() * 1000.0;
    
    let success = result.is_ok();
    metrics::record_user_operation("verify", success);
    
    let response = result?;
    Ok(Json(VerifyEmailResponse {
        success: response.success,
        message: response.message,
        wallet_address: response.wallet_address,
        auth: response.auth.map(|a| AuthResponse {
            access_token: a.access_token,
            expires_in: a.expires_in,
            user: UserResponse {
                id: a.user.id,
                username: a.user.username,
                email: a.user.email,
                role: a.user.role,
                first_name: a.user.first_name,
                last_name: a.user.last_name,
                wallet_address: a.user.wallet_address,
            },
        }),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/users/me",
    responses(
        (status = 200, description = "Profile retrieved successfully", body = UserResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "users",
    security(
        ("jwt" = [])
    )
)]
#[instrument(skip(auth_service, auth))]
pub async fn get_me(
    role: ServiceRole,
    auth: AuthenticatedUser,
    State(auth_service): State<AuthService>,
) -> ApiResult<Json<UserResponse>> {
    let claims = auth.0;
    // /me is for users via Web/App, so it should come via ApiGateway
    role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin])
        .map_err(|(_code, msg)| iam_core::error::ApiError::Unauthorized(msg.to_string()))?;

    tracing::info!("👤 Handling /me request for user: {}", claims.sub);
    let user = auth_service.get_user_profile(claims.sub).await?;
    Ok(Json(UserResponse {
        id: user.id,
        username: user.username,
        email: user.email,
        role: user.role,
        first_name: user.first_name,
        last_name: user.last_name,
        wallet_address: user.wallet_address,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/forgot-password",
    request_body = ForgotPasswordRequest,
    responses(
        (status = 200, description = "Reset email sent (or silently ignored if email not found)", body = ForgotPasswordResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "auth"
)]
pub async fn forgot_password(
    role: ServiceRole,
    State(auth_service): State<AuthService>,
    Json(request): Json<ForgotPasswordRequest>,
) -> ApiResult<Json<ForgotPasswordResponse>> {
    role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin, ServiceRole::Unknown])
        .map_err(|(_code, msg)| iam_core::error::ApiError::Unauthorized(msg.to_string()))?;

    auth_service.forgot_password(&request.email).await?;
    Ok(Json(ForgotPasswordResponse {
        message: "If that email is registered, a reset link has been sent".to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/reset-password",
    request_body = ResetPasswordRequest,
    responses(
        (status = 200, description = "Password reset successful", body = ResetPasswordResponse),
        (status = 400, description = "Invalid or expired token"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "auth"
)]
pub async fn reset_password(
    role: ServiceRole,
    State(auth_service): State<AuthService>,
    Json(request): Json<ResetPasswordRequest>,
) -> ApiResult<Json<ResetPasswordResponse>> {
    role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin, ServiceRole::Unknown])
        .map_err(|(_code, msg)| iam_core::error::ApiError::Unauthorized(msg.to_string()))?;

    auth_service.reset_password(&request.token, &request.new_password).await?;
    Ok(Json(ResetPasswordResponse {
        message: "Password reset successfully".to_string(),
    }))
}
