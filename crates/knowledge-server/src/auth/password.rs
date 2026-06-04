use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rand_core::OsRng;

use crate::http::error::ApiError;

pub fn hash_password(password: &str) -> Result<String, ApiError> {
  let salt = SaltString::generate(&mut OsRng);
  Argon2::default()
    .hash_password(password.as_bytes(), &salt)
    .map(|hash| hash.to_string())
    .map_err(|_| ApiError::internal("failed to hash password"))
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, ApiError> {
  let parsed =
    PasswordHash::new(hash).map_err(|_| ApiError::internal("invalid password hash"))?;
  Ok(
    Argon2::default()
      .verify_password(password.as_bytes(), &parsed)
      .is_ok(),
  )
}
