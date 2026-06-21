use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Serialize, Deserialize};
use chrono::{Utc, Duration};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// "dashboard" or "ai"
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

/// Create a JWT for dashboard access (24h expiry)
pub fn create_dashboard_token(secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let claims = Claims {
        sub: "dashboard".to_string(),
        iat: now.timestamp() as usize,
        exp: (now + Duration::hours(24)).timestamp() as usize,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
}

/// Create a JWT for AI API access (30 day expiry)
pub fn create_ai_token(secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let claims = Claims {
        sub: "ai".to_string(),
        iat: now.timestamp() as usize,
        exp: (now + Duration::days(30)).timestamp() as usize,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
}

/// Validate and decode a JWT, returning its claims
pub fn validate_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::default();
    validation.validate_exp = true;
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;
    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_validate_dashboard_token() {
        let secret = "test-secret-key-123";
        let token = create_dashboard_token(secret).unwrap();
        let claims = validate_token(&token, secret).unwrap();
        assert_eq!(claims.sub, "dashboard");
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_create_and_validate_ai_token() {
        let secret = "test-secret-key-123";
        let token = create_ai_token(secret).unwrap();
        let claims = validate_token(&token, secret).unwrap();
        assert_eq!(claims.sub, "ai");
    }

    #[test]
    fn test_wrong_secret_fails() {
        let secret = "test-secret-key-123";
        let token = create_dashboard_token(secret).unwrap();
        let result = validate_token(&token, "wrong-secret");
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
