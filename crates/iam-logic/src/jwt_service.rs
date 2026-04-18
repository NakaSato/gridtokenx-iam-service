use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use std::env;
use uuid::Uuid;

use iam_core::error::{ApiError, Result};
use iam_core::domain::identity::Claims;

#[derive(Clone)]
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
}

impl JwtService {
    pub fn new(secret: &str) -> Result<Self> {
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
                        ApiError::with_code(iam_core::error::ErrorCode::TokenExpired, "Token has expired")
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidToken => {
                        ApiError::with_code(iam_core::error::ErrorCode::TokenInvalid, "Invalid token")
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidSignature => {
                        ApiError::with_code(iam_core::error::ErrorCode::TokenInvalid, "Invalid token signature")
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
    pub fn new(secret: String) -> Result<Self> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use iam_core::domain::identity::Role;
    use chrono::Utc;

    // Environmental set_var is unsafe in modern Rust and discouraged in tests.
    // Instead, we pass secrets directly to the constructors.

    #[test]
    fn test_jwt_lifecycle() {
        let secret = "test-secret-12345678901234567890";
        let service = JwtService::new(secret).unwrap();
        let user_id = Uuid::new_v4();
        let claims = Claims::new(user_id, "testuser".to_string(), Role::User.to_string());

        // Encode
        let token = service.encode_token(&claims).expect("Encoding failed");
        assert!(!token.is_empty());

        // Decode
        let decoded = service.decode_token(&token).expect("Decoding failed");
        assert_eq!(decoded.sub, user_id);
        assert_eq!(decoded.username, "testuser");
        assert_eq!(decoded.role, Role::User.to_string());

        // Validate
        assert!(service.validate_token(&token).unwrap());
    }

    #[test]
    fn test_jwt_expiration() {
        let secret = "test-secret-12345678901234567890";
        let service = JwtService::new(secret).unwrap();
        let user_id = Uuid::new_v4();
        
        // Create expired claims
        let mut claims = Claims::new(user_id, "testuser".to_string(), Role::User.to_string());
        claims.exp = Utc::now().timestamp() - 600; // Force expiry 10 mins ago

        let token = service.encode_token(&claims).unwrap();
        
        // Validation should fail
        assert!(!service.validate_token(&token).unwrap());
        
        // Decoding should return Unauthorized error
        let result = service.decode_token(&token);
        assert!(result.is_err(), "Expected error for expired token, but got success");
    }

    #[test]
    fn test_jwt_refresh() {
        let secret = "test-secret-12345678901234567890";
        let service = JwtService::new(secret).unwrap();
        let user_id = Uuid::new_v4();
        let claims = Claims::new(user_id, "testuser".to_string(), Role::User.to_string());
        let token = service.encode_token(&claims).unwrap();

        // Refresh
        let refreshed_token = service.refresh_token(&token).expect("Refresh failed");
        
        let decoded = service.decode_token(&refreshed_token).expect("Failed to decode refreshed token");
        assert_eq!(decoded.sub, user_id);
        assert!(service.validate_token(&refreshed_token).unwrap());
    }

    #[test]
    fn test_api_key_lifecycle() {
        let secret = "test-api-key-secret".to_string();
        let service = ApiKeyService::new(secret).unwrap();
        
        let (key, hash) = service.generate_key("test-key", vec!["read".to_string()]).unwrap();
        assert!(key.starts_with("ak_"));
        
        // Verify success
        assert!(service.verify_key(&key, &hash).unwrap());
        
        // Verify failure
        assert!(!service.verify_key("wrong-key", &hash).unwrap());
    }
}
