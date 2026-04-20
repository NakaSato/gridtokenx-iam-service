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
    pub grpc_port: Option<u16>,
    pub registry_program_id: String,
    pub oracle_program_id: String,
    pub governance_program_id: String,
    pub energy_token_program_id: String,
    pub trading_program_id: String,
    pub auth_cpu_semaphore_limit: usize,
    pub tokio_worker_threads: Option<usize>,
    pub database_max_connections: u32,
    pub database_min_connections: u32,
    pub request_timeout_secs: u64,
    pub global_concurrency_limit: usize,
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
            grpc_port: env::var("IAM_GRPC_PORT").ok().and_then(|p| p.parse().ok()),
            registry_program_id: env::var("SOLANA_REGISTRY_PROGRAM_ID")
                .unwrap_or_else(|_| "C8RT8L5pZCVDrf9v94CNNk3XPBKZU5p4o4aPnAVQGiTu".to_string()),
            oracle_program_id: env::var("SOLANA_ORACLE_PROGRAM_ID")
                .unwrap_or_else(|_| "DdeZQdfv7qtnhHktPt8CevKrW6BvjbgKknkD7c63C9hP".to_string()),
            governance_program_id: env::var("SOLANA_GOVERNANCE_PROGRAM_ID")
                .unwrap_or_else(|_| "AMowMcC3gVkEvZ3vaskGC4L9uTsBvTxcD4ewEA1TyrK4".to_string()),
            energy_token_program_id: env::var("SOLANA_ENERGY_TOKEN_PROGRAM_ID")
                .unwrap_or_else(|_| "6ZoMJypt2vufxeUarFJRZxAvRfUsf7gRHZ1pRQTYerNp".to_string()),
            trading_program_id: env::var("SOLANA_TRADING_PROGRAM_ID")
                .unwrap_or_else(|_| "ctBDmdW3VHqqQF7HyEKwoMWszyNcKBNNFsofem3JEup".to_string()),
            auth_cpu_semaphore_limit: env::var("AUTH_CPU_SEMAPHORE_LIMIT")
                .unwrap_or_else(|_| "32".to_string())
                .parse()?,
            tokio_worker_threads: env::var("TOKIO_WORKER_THREADS")
                .ok()
                .and_then(|v| v.parse().ok()),
            database_max_connections: env::var("DATABASE_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "50".to_string())
                .parse()?,
            database_min_connections: env::var("DATABASE_MIN_CONNECTIONS")
                .unwrap_or_else(|_| "5".to_string())
                .parse()?,
            request_timeout_secs: env::var("REQUEST_TIMEOUT_SECS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()?,
            global_concurrency_limit: env::var("GLOBAL_CONCURRENCY_LIMIT")
                .unwrap_or_else(|_| "100".to_string())
                .parse()?,
        })
    }
}
