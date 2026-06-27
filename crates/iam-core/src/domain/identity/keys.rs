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

    /// Resend-verification cooldown gate (keyed by user ID, not email, to keep
    /// PII out of Redis). Presence of the key == still within the cooldown.
    pub fn resend_verification_cooldown(user_id: &str) -> String {
        format!("iam:resend_cooldown:{}", user_id)
    }

    /// Password reset token TTL.
    pub fn password_reset_token(token: &str) -> String {
        format!("iam:password_reset:{}", token)
    }

    /// IP-based rate limit counter.
    pub fn rate_limit(ip: &str, endpoint: &str) -> String {
        format!("iam:rate_limit:{}:{}", endpoint, ip)
    }
}

#[cfg(test)]
mod tests {
    use super::cache;

    #[test]
    fn keys_have_exact_redis_format() {
        // These strings are a contract with Redis — a silent format change
        // orphans every previously-written key (cache misses, lost locks/TTLs).
        assert_eq!(cache::login_attempts("u@x.io"), "iam:login_attempts:u@x.io");
        assert_eq!(cache::account_lock("u@x.io"), "iam:account_lock:u@x.io");
        assert_eq!(cache::user_profile("uid-1"), "iam:user:profile:uid-1");
        assert_eq!(cache::api_key("h@sh"), "iam:api_key:h@sh");
        assert_eq!(
            cache::email_verification_token("tok"),
            "iam:email_verify:tok"
        );
        assert_eq!(
            cache::resend_verification_cooldown("uid-1"),
            "iam:resend_cooldown:uid-1"
        );
        assert_eq!(
            cache::password_reset_token("tok"),
            "iam:password_reset:tok"
        );
    }

    #[test]
    fn rate_limit_orders_endpoint_before_ip() {
        // Args are (ip, endpoint) but the key is endpoint-first — guards against
        // a silent arg swap that would collide unrelated counters.
        assert_eq!(
            cache::rate_limit("1.2.3.4", "/login"),
            "iam:rate_limit:/login:1.2.3.4"
        );
    }

    #[test]
    fn all_keys_share_iam_namespace() {
        for k in [
            cache::login_attempts("x"),
            cache::account_lock("x"),
            cache::user_profile("x"),
            cache::api_key("x"),
            cache::email_verification_token("x"),
            cache::resend_verification_cooldown("x"),
            cache::password_reset_token("x"),
            cache::rate_limit("x", "y"),
        ] {
            assert!(k.starts_with("iam:"), "key not namespaced: {k}");
        }
    }
}
