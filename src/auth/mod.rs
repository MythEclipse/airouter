pub mod middleware;
pub mod jwt;

use axum::http::HeaderMap;

pub fn sha2_hex(input: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

pub fn validate_key(key: &str, valid_keys: &[String]) -> bool {
    valid_keys.iter().any(|k| k == key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn test_extract_bearer_token_some() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer sk-test-123".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers), Some("sk-test-123".to_string()));
    }

    #[test]
    fn test_extract_bearer_token_none() {
        let headers = HeaderMap::new();
        assert_eq!(extract_bearer_token(&headers), None);
    }

    #[test]
    fn test_extract_bearer_token_wrong_prefix() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Basic abc".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers), None);
    }

    #[test]
    fn test_validate_key_match() {
        let keys = vec!["sk-1".into(), "sk-2".into()];
        assert!(validate_key("sk-1", &keys));
        assert!(validate_key("sk-2", &keys));
    }

    #[test]
    fn test_validate_key_no_match() {
        let keys = vec!["sk-1".into()];
        assert!(!validate_key("sk-wrong", &keys));
        assert!(!validate_key("", &keys));
    }

    #[test]
    fn test_validate_key_empty_list() {
        let keys: Vec<String> = vec![];
        assert!(!validate_key("anything", &keys));
    }
}
