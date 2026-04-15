/// Cache key patterns for IAM service.
pub mod cache {
    /// Login attempt counter for rate limiting.
    pub fn login_attempts(identifier: &str) -> String {
        format!("iam:login_attempts:{}", identifier)
    }

    /// Account lock status after too many failed logins.
    pub fn account_lock(identifier: &str) -> String {
        format!("iam:account_lock:{}", identifier)
    }

    /// Cached user profile (by user ID).
    pub fn user_profile(user_id: &str) -> String {
        format!("iam:user:profile:{}", user_id)
    }

    /// Cached API key lookup (by hash).
    pub fn api_key(key_hash: &str) -> String {
        format!("iam:api_key:{}", key_hash)
    }

    /// Email verification token TTL.
    pub fn email_verification_token(token: &str) -> String {
        format!("iam:email_verify:{}", token)
    }

    /// Password reset token TTL.
    pub fn password_reset_token(token: &str) -> String {
        format!("iam:password_reset:{}", token)
    }
}
