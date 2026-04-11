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
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        Ok(Config {
            environment: env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            port: env::var("IAM_PORT")
                .unwrap_or_else(|_| "8081".to_string())
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
        })
    }
}
