use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub token_type: String,
    pub iat: i64,
    pub exp: i64,
}

const ACCESS_TOKEN_EXPIRY_SECS: i64 = 900; // 15 minutes
const REFRESH_TOKEN_EXPIRY_SECS: i64 = 604800; // 7 days
const WS_TOKEN_EXPIRY_SECS: i64 = 30; // 30 seconds

pub fn create_access_token(
    user_id: Uuid,
    email: &str,
    name: &str,
    secret: &str,
) -> Result<String, AppError> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: user_id.to_string(),
        email: Some(email.to_string()),
        name: Some(name.to_string()),
        token_type: "access".to_string(),
        iat: now,
        exp: now + ACCESS_TOKEN_EXPIRY_SECS,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Token creation failed: {}", e)))
}

pub fn create_refresh_token(user_id: Uuid, secret: &str) -> Result<String, AppError> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: user_id.to_string(),
        email: None,
        name: None,
        token_type: "refresh".to_string(),
        iat: now,
        exp: now + REFRESH_TOKEN_EXPIRY_SECS,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Token creation failed: {}", e)))
}

pub fn create_ws_token(user_id: Uuid, secret: &str) -> Result<String, AppError> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: user_id.to_string(),
        email: None,
        name: None,
        token_type: "ws".to_string(),
        iat: now,
        exp: now + WS_TOKEN_EXPIRY_SECS,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Token creation failed: {}", e)))
}

pub fn validate_token(
    token: &str,
    expected_type: &str,
    secret: &str,
) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| AppError::Unauthorized("Invalid or expired token".to_string()))?;

    if token_data.claims.token_type != expected_type {
        return Err(AppError::Unauthorized(
            "Invalid or expired token".to_string(),
        ));
    }

    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "test-secret-key";

    #[test]
    fn test_create_and_validate_access_token() {
        let user_id = Uuid::new_v4();
        let token =
            create_access_token(user_id, "alice@example.com", "Alice", TEST_SECRET).unwrap();
        let claims = validate_token(&token, "access", TEST_SECRET).unwrap();
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.email.as_deref(), Some("alice@example.com"));
        assert_eq!(claims.name.as_deref(), Some("Alice"));
        assert_eq!(claims.token_type, "access");
    }

    #[test]
    fn test_create_and_validate_refresh_token() {
        let user_id = Uuid::new_v4();
        let token = create_refresh_token(user_id, TEST_SECRET).unwrap();
        let claims = validate_token(&token, "refresh", TEST_SECRET).unwrap();
        assert_eq!(claims.sub, user_id.to_string());
        assert!(claims.email.is_none());
        assert!(claims.name.is_none());
        assert_eq!(claims.token_type, "refresh");
    }

    #[test]
    fn test_reject_expired_token() {
        // Create a token that's already expired by manually building claims
        let user_id = Uuid::new_v4();
        let now = Utc::now().timestamp();
        let claims = Claims {
            sub: user_id.to_string(),
            email: None,
            name: None,
            token_type: "access".to_string(),
            iat: now - 1000,
            exp: now - 500, // expired 500 seconds ago
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();
        let result = validate_token(&token, "access", TEST_SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn test_reject_wrong_token_type() {
        let user_id = Uuid::new_v4();
        let token = create_refresh_token(user_id, TEST_SECRET).unwrap();
        // Try to validate as access token
        let result = validate_token(&token, "access", TEST_SECRET);
        assert!(result.is_err());
    }
}
