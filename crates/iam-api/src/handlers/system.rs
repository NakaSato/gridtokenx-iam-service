use axum::{Json, extract::State};
use iam_core::config::Config;
use crate::handlers::types::SystemConfigResponse;

#[utoipa::path(
    get,
    path = "/api/v1/system/config",
    responses(
        (status = 200, description = "System configuration retrieved successfully", body = SystemConfigResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "system"
)]
pub async fn get_config(
    State(auth_service): State<iam_logic::AuthService>,
) -> Json<SystemConfigResponse> {
    let config = &auth_service.config;
    Json(SystemConfigResponse {
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
    })
}
