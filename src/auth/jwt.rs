use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Serialize, Deserialize};
use chrono::{Utc, Duration};

use crate::auth::jwt_secret_store::JwtSecrets;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    Login,
    ChangePwd,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// "dashboard" or "ai"
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub typ: TokenType,
}

/// Create a JWT with the given subject, token type, and TTL.
pub fn create_token_with_type(
    secret: &str,
    sub: &str,
    typ: TokenType,
    ttl_secs: i64,
) -> anyhow::Result<String> {
    let now = Utc::now().timestamp() as usize;
    let exp = (Utc::now().timestamp() + ttl_secs) as usize;
    let claims = Claims {
        sub: sub.to_string(),
        iat: now,
        exp,
        typ,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok(token)
}

/// Create a JWT for dashboard access (24h expiry)
pub fn create_dashboard_token(secret: &str) -> anyhow::Result<String> {
    create_token_with_type(secret, "dashboard", TokenType::Login, 86400)
}

/// Create a JWT for AI API access (30 day expiry)
pub fn create_ai_token(secret: &str) -> anyhow::Result<String> {
    create_token_with_type(secret, "ai", TokenType::Login, 30 * 86400)
}

/// Validate a JWT using current secret first, then previous (if in grace).
pub fn validate_token(token: &str, secrets: &JwtSecrets) -> Result<Claims, jsonwebtoken::errors::Error> {
    // Try current
    if let Ok(claims) = decode_one(token, &secrets.current_secret) {
        return Ok(claims);
    }
    // Try previous if grace period active
    if let (Some(prev), Some(expires_at)) = (&secrets.previous_secret, secrets.previous_expires_at) {
        if expires_at > Utc::now() {
            if let Ok(claims) = decode_one(token, prev) {
                return Ok(claims);
            }
        }
    }
    // Fall through to fail with current decode error
    decode_one(token, &secrets.current_secret)
}

fn decode_one(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let validation = Validation::default();
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_token(secret: &str, sub: &str, typ: TokenType) -> String {
        create_token_with_type(secret, sub, typ, 3600).unwrap()
    }

    // ── Dual-secret tests ──────────────────────────────────────────────

    #[test]
    fn validate_succeeds_with_current_secret() {
        let secrets = JwtSecrets {
            current_secret: "current123".to_string(),
            previous_secret: None,
            previous_expires_at: None,
        };
        let token = make_token("current123", "dashboard", TokenType::Login);
        let claims = validate_token(&token, &secrets).unwrap();
        assert_eq!(claims.sub, "dashboard");
        assert_eq!(claims.typ, TokenType::Login);
    }

    #[test]
    fn validate_succeeds_with_previous_in_grace_period() {
        let secrets = JwtSecrets {
            current_secret: "current".to_string(),
            previous_secret: Some("previous".to_string()),
            previous_expires_at: Some(Utc::now() + Duration::hours(1)),
        };
        let token = make_token("previous", "dashboard", TokenType::Login);
        let claims = validate_token(&token, &secrets).unwrap();
        assert_eq!(claims.sub, "dashboard");
    }

    #[test]
    fn validate_fails_with_previous_after_grace_period() {
        let secrets = JwtSecrets {
            current_secret: "current".to_string(),
            previous_secret: Some("previous".to_string()),
            previous_expires_at: Some(Utc::now() - Duration::hours(1)),
        };
        let token = make_token("previous", "dashboard", TokenType::Login);
        assert!(validate_token(&token, &secrets).is_err());
    }

    #[test]
    fn validate_fails_with_completely_wrong_secret() {
        let secrets = JwtSecrets {
            current_secret: "current".to_string(),
            previous_secret: None,
            previous_expires_at: None,
        };
        let token = make_token("wrong", "dashboard", TokenType::Login);
        assert!(validate_token(&token, &secrets).is_err());
    }

    // ── Existing tests (updated for new signatures) ────────────────────

    #[test]
    fn test_create_and_validate_dashboard_token() {
        let secret = "test-secret-key-123";
        let secrets = JwtSecrets {
            current_secret: secret.to_string(),
            previous_secret: None,
            previous_expires_at: None,
        };
        let token = create_dashboard_token(secret).unwrap();
        let claims = validate_token(&token, &secrets).unwrap();
        assert_eq!(claims.sub, "dashboard");
        assert_eq!(claims.typ, TokenType::Login);
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_create_and_validate_ai_token() {
        let secret = "test-secret-key-123";
        let secrets = JwtSecrets {
            current_secret: secret.to_string(),
            previous_secret: None,
            previous_expires_at: None,
        };
        let token = create_ai_token(secret).unwrap();
        let claims = validate_token(&token, &secrets).unwrap();
        assert_eq!(claims.sub, "ai");
        assert_eq!(claims.typ, TokenType::Login);
    }

    #[test]
    fn test_wrong_secret_fails() {
        let secret = "test-secret-key-123";
        let token = create_dashboard_token(secret).unwrap();
        let wrong_secrets = JwtSecrets {
            current_secret: "wrong-secret".to_string(),
            previous_secret: None,
            previous_expires_at: None,
        };
        let result = validate_token(&token, &wrong_secrets);
        assert!(result.is_err());
    }

    #[test]
    fn test_dashboard_and_ai_are_different() {
        let secret = "test-secret-key-123";
        let dash = create_dashboard_token(secret).unwrap();
        let ai = create_ai_token(secret).unwrap();
        assert_ne!(dash, ai);
    }
}
