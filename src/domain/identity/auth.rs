use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// Re-export Permission from the roles module
pub use crate::domain::identity::roles::Permission;

/// User claims for JWT tokens
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Claims {
    pub sub: Uuid,        // Subject (user ID)
    pub username: String, // Username
    pub role: String,     // User role (user, admin, ami)
    pub exp: i64,         // Expiration time
    pub iat: i64,         // Issued at
    pub iss: String,      // Issuer
}

impl Claims {
    pub fn new(user_id: Uuid, username: String, role: String) -> Self {
        let now = Utc::now();
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

    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp() > self.exp
    }

    pub fn has_role(&self, required_role: &str) -> bool {
        self.role == required_role
    }

    pub fn has_any_role(&self, required_roles: &[&str]) -> bool {
        required_roles.contains(&self.role.as_str())
    }
}

/// API Key for AMI systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: Uuid,
    pub key_hash: String,
    pub name: String,
    pub role: String,
    pub permissions: Vec<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Secure authentication response (excludes sensitive user data)
#[derive(Debug, Serialize, ToSchema)]
pub struct SecureAuthResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: SecureUserInfo,
}

/// User information for responses
#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfo {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: String,
    pub wallet_address: Option<String>,
}

/// Secure user information for login responses (excludes sensitive data)
#[derive(Debug, Serialize, ToSchema)]
pub struct SecureUserInfo {
    pub username: String,
    pub email: String,
    pub role: String,
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
