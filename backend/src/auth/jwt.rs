use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub email: String,
    pub role: String,
    pub exp: i64,
}

pub fn issue_token(
    user_id: Uuid,
    email: &str,
    role: &str,
    secret: &str,
    ttl_hours: i64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = Claims {
        sub: user_id,
        email: email.to_string(),
        role: role.to_string(),
        exp: (Utc::now() + Duration::hours(ttl_hours)).timestamp(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

pub fn verify_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_claims() {
        let id = Uuid::new_v4();
        let token = issue_token(id, "a@b.com", "admin", "secret", 1).unwrap();
        let claims = verify_token(&token, "secret").unwrap();
        assert_eq!(claims.sub, id);
        assert_eq!(claims.email, "a@b.com");
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn wrong_secret_is_rejected() {
        let token = issue_token(Uuid::new_v4(), "a@b.com", "staff", "secret", 1).unwrap();
        assert!(verify_token(&token, "other-secret").is_err());
    }

    #[test]
    fn expired_token_is_rejected() {
        let token = issue_token(Uuid::new_v4(), "a@b.com", "staff", "secret", -1).unwrap();
        assert!(verify_token(&token, "secret").is_err());
    }
}
