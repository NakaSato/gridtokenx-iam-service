use iam_core::error::{ApiError, Result};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use bcrypt::verify as bcrypt_verify;

pub struct PasswordService;

impl PasswordService {
    pub fn hash_password(password: &str) -> Result<String> {
        // Validate password strength first
        Self::validate_password_strength(password)?;

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        match argon2.hash_password(password.as_bytes(), &salt) {
            Ok(hash) => Ok(hash.to_string()),
            Err(e) => Err(ApiError::Internal(format!("Argon2 hashing failed: {}", e))),
        }
    }

    pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
        // Detect if it's an Argon2 hash or legacy Bcrypt
        if hash.starts_with("$argon2") {
            let parsed_hash = PasswordHash::new(hash)
                .map_err(|e| ApiError::Internal(format!("Invalid Argon2 hash format: {}", e)))?;

            Ok(Argon2::default()
                .verify_password(password.as_bytes(), &parsed_hash)
                .is_ok())
        } else if hash.starts_with("$2") {
            // Legacy Bcrypt
            bcrypt_verify(password, hash)
                .map_err(|e| ApiError::Internal(format!("Bcrypt verification failed: {}", e)))
        } else {
            Err(ApiError::Internal(
                "Unknown password hash format".to_string(),
            ))
        }
    }

    pub fn validate_password_strength(password: &str) -> Result<()> {
        let min_length = 8;
        let max_length = 128;

        if password.len() < min_length {
            return Err(ApiError::BadRequest(format!(
                "Password must be at least {} characters long",
                min_length
            )));
        }

        if password.len() > max_length {
            return Err(ApiError::BadRequest(format!(
                "Password must be no more than {} characters long",
                max_length
            )));
        }

        let has_lowercase = password.chars().any(|c| c.is_ascii_lowercase());
        let has_uppercase = password.chars().any(|c| c.is_ascii_uppercase());
        let has_digit = password.chars().any(|c| c.is_ascii_digit());
        let has_special = password
            .chars()
            .any(|c| "!@#$%^&*()_+-=[]{}|;:,.<>?".contains(c));

        let strength_checks = [
            (has_lowercase, "at least one lowercase letter"),
            (has_uppercase, "at least one uppercase letter"),
            (has_digit, "at least one digit"),
            (
                has_special,
                "at least one special character (!@#$%^&*()_+-=[]{}|;:,.<>?)",
            ),
        ];

        let mut missing_requirements = Vec::new();
        for (check, requirement) in strength_checks {
            if !check {
                missing_requirements.push(requirement);
            }
        }

        if !missing_requirements.is_empty() {
            return Err(ApiError::BadRequest(format!(
                "Password must contain: {}",
                missing_requirements.join(", ")
            )));
        }

        // Check for common weak patterns
        let password_lower = password.to_lowercase();
        let weak_patterns = [
            "password", "123456", "qwerty", "admin", "letmein", "welcome", "monkey", "dragon",
        ];

        for pattern in &weak_patterns {
            if password_lower.contains(pattern) {
                return Err(ApiError::BadRequest(
                    "Password contains common weak patterns".to_string(),
                ));
            }
        }

        Ok(())
    }

    pub fn generate_temporary_password() -> String {
        use rand::Rng;

        let mut rng = rand::thread_rng();
        const SPECIAL_CHARS: &[u8] = b"!@#$%^&*";
        const ALPHANUMERIC: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

        // Generate a password with mixed case, digits, and special characters
        let mut password = String::new();

        // Add at least one of each required character type
        password.push(rng.gen_range('A'..='Z'));
        password.push(rng.gen_range('a'..='z'));
        password.push(rng.gen_range('0'..='9'));
        // Use byte slice indexing which is always safe for ASCII
        password.push(SPECIAL_CHARS[rng.gen_range(0..SPECIAL_CHARS.len())] as char);

        // Fill the rest with random alphanumeric characters
        for _ in 0..8 {
            let idx = rng.gen_range(0..ALPHANUMERIC.len());
            password.push(ALPHANUMERIC[idx] as char);
        }

        // Shuffle the password
        let mut chars: Vec<char> = password.chars().collect();
        for i in 0..chars.len() {
            let j = rng.gen_range(0..chars.len());
            chars.swap(i, j);
        }

        chars.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_strength_valid() {
        // Strong valid password
        assert!(PasswordService::validate_password_strength("GridTokenX123!").is_ok());
    }

    #[test]
    fn test_password_strength_invalid_length() {
        assert!(PasswordService::validate_password_strength("Short1!").is_err());
        assert!(PasswordService::validate_password_strength(&"a".repeat(129)).is_err());
    }

    #[test]
    fn test_password_strength_missing_requirements() {
        assert!(PasswordService::validate_password_strength("lowercaseonly123!").is_err()); // No uppercase
        assert!(PasswordService::validate_password_strength("UPPERCASEONLY123!").is_err()); // No lowercase
        assert!(PasswordService::validate_password_strength("NoDigitSpecial!").is_err());    // No digit (Wait, it has special. But strength checks check for lowercase, uppercase, digit, special)
        // Let's check requirements in code: lowercase, uppercase, digit, special
        assert!(PasswordService::validate_password_strength("Ab1!").is_err()); // Too short
        assert!(PasswordService::validate_password_strength("Abcdefghigklmnop").is_err()); // No digit, no special
    }

    #[test]
    fn test_password_weak_patterns() {
        assert!(PasswordService::validate_password_strength("Password123!").is_err());
        assert!(PasswordService::validate_password_strength("Admin1234!").is_err());
    }

    #[test]
    fn test_hash_and_verify_argon2() {
        let password = "GridTokenX-IAM-Strong-1!";
        let hash = PasswordService::hash_password(password).expect("Hashing failed");
        
        assert!(hash.starts_with("$argon2"));
        assert!(PasswordService::verify_password(password, &hash).unwrap());
        assert!(!PasswordService::verify_password("WrongPassword123!", &hash).unwrap());
    }

    #[test]
    fn test_verify_legacy_bcrypt() {
        // Example bcrypt hash for "password123" (not strong enough for our current rules, but we test legacy verification)
        let password = "password123";
        let _legacy_hash = "$2y$12$DUMMYHASHFORTESTINGPURPOSESONLYNOTAREALONE_"; 
        // We can't easily dummy it without a real bcrypt lib, so we'll just check if it routes correctly
        // Instead, let's generate a real bcrypt hash if possible
        let bcrypt_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST).unwrap();
        assert!(PasswordService::verify_password(password, &bcrypt_hash).unwrap());
    }

    #[test]
    fn test_generate_temporary_password() {
        for _ in 0..10 {
            let temp = PasswordService::generate_temporary_password();
            // Should be valid according to our strength rules
            assert!(PasswordService::validate_password_strength(&temp).is_ok());
        }
    }
}
