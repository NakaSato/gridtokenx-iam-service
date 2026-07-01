use axum::{Json, extract::State};
use gridtokenx_blockchain_core::auth::ServiceRole;
use iam_core::config::Config;
use iam_core::error::{ApiError, Result as ApiResult};
use crate::handlers::types::SystemConfigResponse;

#[utoipa::path(
    get,
    path = "/api/v1/system/config",
    responses(
        (status = 200, description = "System configuration retrieved successfully", body = SystemConfigResponse),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error")
    ),
    tag = "system"
)]
pub async fn get_config(
    role: ServiceRole,
    State(auth_service): State<iam_logic::AuthService>,
) -> ApiResult<Json<SystemConfigResponse>> {
    // APISIX gates this route to internal CIDRs (see apisix.yaml route 13) and
    // injects x-gridtokenx-role: api-gateway. This check is defense-in-depth,
    // not the primary boundary — every other handler in auth.rs/identity.rs
    // already does the same, this one was the odd one out.
    role.require_any(&[ServiceRole::ApiGateway, ServiceRole::Admin])
        .map_err(|(_code, msg)| ApiError::Forbidden(msg.to_string()))?;

    let config = &auth_service.config;
    Ok(Json(SystemConfigResponse {
        environment: config.environment.clone(),
        solana_rpc_url: config.solana_rpc_url.clone(),
        solana_cluster: config.solana_cluster.clone(),
        registry_program_id: config.registry_program_id.clone(),
        oracle_program_id: config.oracle_program_id.clone(),
        governance_program_id: config.governance_program_id.clone(),
        energy_token_program_id: config.energy_token_program_id.clone(),
        trading_program_id: config.trading_program_id.clone(),
        energy_token_mint: config.energy_token_mint.clone(),
        currency_token_mint: config.currency_token_mint.clone(),
    }))
}
