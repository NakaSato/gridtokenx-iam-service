use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub environment: String,
    pub port: u16,
    pub database_url: String,
    pub redis_url: String,
    pub jwt_secret: String,
    pub jwt_expiration: i64,
    pub encryption_secret: String,
    pub api_key_secret: String,
    pub log_level: String,
    pub test_mode: bool,
    pub solana_rpc_url: String,
    pub chain_bridge_url: String,
    pub solana_cluster: String,
    pub master_secret: String,
    pub kafka_brokers: Option<String>,
    pub rabbitmq_url: Option<String>,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_from: String,
    pub app_base_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        Ok(Config {
            environment: env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            port: env::var("IAM_PORT")
                .unwrap_or_else(|_| "4010".to_string())
                .parse()?,
            database_url: env::var("IAM_DATABASE_URL")
                .or_else(|_| env::var("DATABASE_URL"))
                .map_err(|_| anyhow::anyhow!("IAM_DATABASE_URL or DATABASE_URL is required"))?,
            redis_url: env::var("REDIS_URL")
                .map_err(|_| anyhow::anyhow!("REDIS_URL is required"))?,
            jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| "supersecretjwtkey".to_string()),
            jwt_expiration: env::var("JWT_EXPIRATION")
                .unwrap_or_else(|_| "86400".to_string())
                .parse()?,
            encryption_secret: env::var("ENCRYPTION_SECRET")
                .unwrap_or_else(|_| "supersecretencryptionkey".to_string()),
            api_key_secret: env::var("API_KEY_SECRET")
                .unwrap_or_else(|_| "supersecretapikey".to_string()),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            test_mode: env::var("TEST_MODE")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            solana_rpc_url: env::var("SOLANA_RPC_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8899".to_string()),
            chain_bridge_url: env::var("CHAIN_BRIDGE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:5040".to_string()),
            solana_cluster: env::var("SOLANA_CLUSTER")
                .unwrap_or_else(|_| "localnet".to_string()),
            master_secret: env::var("MASTER_SECRET")
                .unwrap_or_else(|_| "super-secret-master-key-change-me".to_string()),
            kafka_brokers: env::var("KAFKA_CMD_BROKERS").ok(),
            rabbitmq_url: env::var("RABBITMQ_URL").ok(),
            smtp_host: env::var("SMTP_HOST").unwrap_or_else(|_| "localhost".to_string()),
            smtp_port: env::var("SMTP_PORT").unwrap_or_else(|_| "1025".to_string()).parse()?,
            smtp_from: env::var("SMTP_FROM").unwrap_or_else(|_| "noreply@gridtokenx.local".to_string()),
            app_base_url: env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string()),
        })
    }
}
