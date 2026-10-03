use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, SaltString};
use argon2::{Argon2, PasswordVerifier};
use rand::Rng;

/// Generate a random 6-digit verification code.
pub fn generate_code() -> String {
    format!("{:06}", rand::thread_rng().gen_range(0..1_000_000u32))
}

/// Hash a value (password or 2FA code) with Argon2id. Codes are never stored
/// in plain text in `login_challenges`.
pub fn hash_secret(value: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(value.as_bytes(), &salt)?
        .to_string())
}

pub fn verify_secret(value: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(value.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_is_six_digits() {
        for _ in 0..50 {
            let code = generate_code();
            assert_eq!(code.len(), 6);
            assert!(code.chars().all(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn hash_and_verify_round_trip() {
        let hash = hash_secret("123456").unwrap();
        assert_ne!(hash, "123456", "must not store plaintext");
        assert!(verify_secret("123456", &hash));
        assert!(!verify_secret("654321", &hash));
    }

    #[test]
    fn garbage_hash_never_verifies() {
        assert!(!verify_secret("123456", "not-a-hash"));
    }
}
