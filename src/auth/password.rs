//! Password generation, hashing, and validation.
//!
//! Hashing uses SHA-256 (current). Argon2id upgrade is deferred to a
//! follow-up spec — acceptable for single-admin internal tool with
//! 32-char random initial password.

use rand::Rng;
use sha2::{Digest, Sha256};

const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                          abcdefghijklmnopqrstuvwxyz\
                          0123456789";

/// Generate a random alphanumeric password of given length.
pub fn generate_password(len: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

/// Hash password with SHA-256 and return hex string.
pub fn hash_password(pwd: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pwd.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Validate password meets minimum requirements.
pub fn validate_password_strength(pwd: &str) -> Result<(), &'static str> {
    if pwd.len() < 12 {
        return Err("Password must be at least 12 characters");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_password_produces_correct_length() {
        let p = generate_password(32);
        assert_eq!(p.len(), 32);
    }

    #[test]
    fn generate_password_uses_only_alphanumeric() {
        let p = generate_password(100);
        assert!(p.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn generate_password_is_random() {
        let p1 = generate_password(32);
        let p2 = generate_password(32);
        assert_ne!(p1, p2);
    }

    #[test]
    fn hash_password_is_deterministic() {
        let h1 = hash_password("hello");
        let h2 = hash_password("hello");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_password_is_64_hex_chars() {
        let h = hash_password("hello");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn validate_rejects_short_password() {
        assert!(validate_password_strength("short").is_err());
    }

    #[test]
    fn validate_accepts_12_char_password() {
        assert!(validate_password_strength("abcdefghijkl").is_ok());
    }

    #[test]
    fn validate_accepts_long_password() {
        assert!(validate_password_strength("a-very-long-secure-password-123!").is_ok());
    }
}
