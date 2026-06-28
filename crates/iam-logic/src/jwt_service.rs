use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
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
                    _ => ApiError::with_code(iam_core::error::ErrorCode::TokenInvalid, format!("Invalid token: {}", e)),
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

    /// Verifies a presented key against a stored hash in constant time.
    ///
    /// Recomputes the HMAC of `key` and compares against `stored_hash` using
    /// `Mac::verify_slice`, which is constant-time — no early-exit byte compare
    /// that could leak the hash via timing. A malformed (non-hex) `stored_hash`
    /// is treated as a non-match, not an error.
    pub fn verify_key(&self, key: &str, stored_hash: &str) -> Result<bool> {
        use hmac::Mac as _;
        let Ok(expected) = hex::decode(stored_hash) else {
            return Ok(false);
        };
        Ok(self.mac(key)?.verify_slice(&expected).is_ok())
    }

    /// Derives the stored fingerprint of an API key as lowercase-hex
    /// HMAC-SHA256(secret, key).
    ///
    /// HMAC (keyed) rather than a plain `SHA-256(key || secret)` digest: the
    /// latter is a non-standard keyed construction vulnerable to length
    /// extension, whereas HMAC is the correct primitive for keyed hashing.
    pub fn hash_key(&self, key: &str) -> Result<String> {
        use hmac::Mac as _;
        Ok(hex::encode(self.mac(key)?.finalize().into_bytes()))
    }

    /// Builds an HMAC-SHA256 instance keyed by the service secret, pre-loaded
    /// with `key` as the message. Shared by `hash_key` and `verify_key` so both
    /// derive identically.
    fn mac(&self, key: &str) -> Result<hmac::Hmac<sha2::Sha256>> {
        use hmac::Mac as _;
        let mut mac = <hmac::Hmac<sha2::Sha256>>::new_from_slice(self.secret.as_bytes())
            .map_err(|e| ApiError::internal(format!("HMAC key init failed: {e}")))?;
        mac.update(key.as_bytes());
        Ok(mac)
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

    /// Pins `hash_key` to HMAC-SHA256(secret, key) and proves the old
    /// SHA-256(key || secret) construction no longer verifies — guards the
    /// keyed-hash swap. Vector matches the re-seed migration
    /// (20260629000000_reseed_default_api_key_hmac.sql).
    #[test]
    fn hash_key_is_hmac_sha256_and_rejects_legacy_digest() {
        use sha2::{Digest, Sha256};

        let secret = "test-api-key-secret-for-development-and-testing".to_string();
        let key = "engineering-department-api-key-2025";
        let service = ApiKeyService::new(secret.clone()).unwrap();

        // Exact HMAC vector shipped in the re-seed migration.
        let hmac_hex = service.hash_key(key).unwrap();
        assert_eq!(
            hmac_hex,
            "0c9b5d31c7e6ec3963f5c7de72c4a6d3346a2991b7021e2f70e7392b5662ac21"
        );
        assert!(service.verify_key(key, &hmac_hex).unwrap());

        // The legacy SHA-256(key || secret) digest must NOT verify under HMAC.
        let mut h = Sha256::new();
        h.update(key.as_bytes());
        h.update(secret.as_bytes());
        let legacy_hex = format!("{:x}", h.finalize());
        assert!(
            !service.verify_key(key, &legacy_hex).unwrap(),
            "legacy SHA-256(key||secret) digest must no longer authenticate"
        );
    }

    /// A malformed (non-hex) stored hash is a non-match, never an error — so a
    /// corrupt DB row can't 500 the auth path.
    #[test]
    fn verify_key_malformed_hash_is_false_not_error() {
        let service = ApiKeyService::new("s".to_string()).unwrap();
        assert!(!service.verify_key("anykey", "not-hex-zzzz").unwrap());
    }
}
