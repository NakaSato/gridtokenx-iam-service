use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use std::env;
use uuid::Uuid;

use crate::core::error::{ApiError, Result};
use crate::domain::identity::Claims;

#[derive(Clone)]
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
}

impl JwtService {
    pub fn new() -> Result<Self> {
        use anyhow::Context;
        
        let secret = env::var("JWT_SECRET")
            .context("JWT_SECRET environment variable not set")
            .map_err(ApiError::from)?;

        let encoding_key = EncodingKey::from_secret(secret.as_ref());
        let decoding_key = DecodingKey::from_secret(secret.as_ref());

        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&["gridtokenx-iam-service"]);
        validation.validate_exp = true;

        Ok(Self {
            encoding_key,
            decoding_key,
            validation,
        })
    }

    pub fn encode_token(&self, claims: &Claims) -> Result<String> {
        use anyhow::Context;

        encode(&Header::new(Algorithm::HS256), claims, &self.encoding_key)
            .context("Failed to encode JWT token")
            .map_err(ApiError::from)
    }

    pub fn decode_token(&self, token: &str) -> Result<Claims> {
        decode::<Claims>(token, &self.decoding_key, &self.validation)
            .map_err(|e| {
                match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                        ApiError::Unauthorized("Token has expired".to_string())
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidToken => {
                        ApiError::Unauthorized("Invalid token".to_string())
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidSignature => {
                        ApiError::Unauthorized("Invalid token signature".to_string())
                    }
                    _ => ApiError::Internal(format!("JWT decode error: {}", e)),
                }
            })
            .map(|data| data.claims)
    }

    pub fn validate_token(&self, token: &str) -> Result<bool> {
        match self.decode_token(token) {
            Ok(claims) => Ok(!claims.is_expired()),
            Err(_) => Ok(false),
        }
    }

    pub fn refresh_token(&self, old_token: &str) -> Result<String> {
        let claims = self.decode_token(old_token)?;

        // Create new claims with extended expiration
        let new_claims = Claims::new(claims.sub, claims.username, claims.role);

        self.encode_token(&new_claims)
    }
}

/// API Key service for AMI systems
#[derive(Clone)]
pub struct ApiKeyService {
    secret: String,
}

impl ApiKeyService {
    pub fn new() -> Result<Self> {
        let secret = env::var("API_KEY_SECRET").map_err(|_| {
            ApiError::Internal("API_KEY_SECRET environment variable not set".to_string())
        })?;

        Ok(Self { secret })
    }

    pub fn generate_key(&self, _name: &str, _permissions: Vec<String>) -> Result<(String, String)> {
        let key = format!("ak_{}", Uuid::new_v4().to_string().replace('-', ""));
        let key_hash = self.hash_key(&key)?;

        Ok((key, key_hash))
    }

    pub fn verify_key(&self, key: &str, stored_hash: &str) -> Result<bool> {
        let computed_hash = self.hash_key(key)?;
        Ok(computed_hash == stored_hash)
    }

    pub fn hash_key(&self, key: &str) -> Result<String> {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hasher.update(self.secret.as_bytes());

        Ok(format!("{:x}", hasher.finalize()))
    }
}
