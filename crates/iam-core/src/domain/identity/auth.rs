use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// Re-export Permission from the roles module
pub use crate::domain::identity::roles::Permission;

/// User claims for JWT tokens
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Claims {
    /// Subject — the user ID.
    pub sub: Uuid,
    /// Username.
    pub username: String,
    /// User role (user, admin, ami, ...).
    pub role: String,
    /// Expiration time (Unix timestamp, seconds).
    pub exp: i64,
    /// Issued-at time (Unix timestamp, seconds).
    pub iat: i64,
    /// Issuer identifier.
    pub iss: String,
}

impl Claims {
    /// Builds claims for `user_id`/`username`/`role`, expiring 24h from now.
    pub fn new(user_id: Uuid, username: String, role: String) -> Self {
        let now = gridtokenx_telemetry::time::now();
        let exp = now + chrono::Duration::hours(24); // 24 hour expiration

        Self {
            sub: user_id,
            username,
            role,
            exp: exp.timestamp(),
            iat: now.timestamp(),
            iss: "gridtokenx-iam-service".to_string(),
        }
    }

    /// Whether `exp` is in the past.
    pub fn is_expired(&self) -> bool {
        gridtokenx_telemetry::time::now().timestamp() > self.exp
    }

    /// Whether `role` matches `required_role` exactly.
    pub fn has_role(&self, required_role: &str) -> bool {
        self.role == required_role
    }

    /// Whether `role` matches any of `required_roles`.
    pub fn has_any_role(&self, required_roles: &[&str]) -> bool {
        required_roles.contains(&self.role.as_str())
    }
}

/// API Key for AMI systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// Unique key ID.
    pub id: Uuid,
    /// HMAC-SHA256 hash of the raw key (never the raw key itself).
    pub key_hash: String,
    /// Human-readable label for the key.
    pub name: String,
    /// Role granted to requests authenticated with this key.
    pub role: String,
    /// Permission strings granted to this key.
    pub permissions: Vec<String>,
    /// Whether the key is currently usable.
    pub is_active: bool,
    /// When the key was created.
    pub created_at: DateTime<Utc>,
    /// When the key was last used to authenticate, if ever.
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Secure authentication response (excludes sensitive user data)
#[derive(Debug, Serialize, ToSchema)]
pub struct SecureAuthResponse {
    /// The signed JWT.
    pub access_token: String,
    /// Always `"Bearer"`.
    pub token_type: String,
    /// Seconds until `access_token` expires.
    pub expires_in: i64,
    /// The authenticated user's public-safe info.
    pub user: SecureUserInfo,
}

/// User information for responses
#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfo {
    /// User ID.
    pub id: Uuid,
    /// Username.
    pub username: String,
    /// Email address.
    pub email: String,
    /// Assigned role.
    pub role: String,
    /// Primary on-chain wallet address, if linked.
    pub wallet_address: Option<String>,
}

/// Secure user information for login responses (excludes sensitive data)
#[derive(Debug, Serialize, ToSchema)]
pub struct SecureUserInfo {
    /// Username.
    pub username: String,
    /// Email address.
    pub email: String,
    /// Assigned role.
    pub role: String,
    /// Whether the user has completed on-chain registration.
    pub blockchain_registered: bool,
}

// Use Role from roles module
pub use crate::domain::identity::roles::Role;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_permissions() {
        let admin = Role::Admin;
        assert!(admin.can_access("users:create"));
        assert!(admin.can_access("energy:read"));
        assert!(admin.can_access("admin:settings"));

        let user = Role::User;
        assert!(user.can_access("energy:read"));
        assert!(user.can_access("trading:create"));
        assert!(!user.can_access("users:create"));
        assert!(!user.can_access("admin:settings"));
    }

    #[test]
    fn test_claims_expiration() {
        let claims = Claims::new(Uuid::new_v4(), "test_user".to_string(), "user".to_string());

        assert!(!claims.is_expired());
        assert!(claims.has_role("user"));
        assert!(!claims.has_role("admin"));
    }
}
