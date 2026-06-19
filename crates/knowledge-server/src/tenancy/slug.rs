use crate::http::error::ApiError;

/// Validate an org/team slug: lowercase ASCII letters/digits and single dashes,
/// 1-64 chars, no leading or trailing dash.
pub fn validate_slug(slug: &str) -> Result<(), ApiError> {
  let len = slug.len();
  if len == 0 || len > 64 {
    return Err(ApiError::bad_request("slug must be 1-64 characters"));
  }
  if slug.starts_with('-') || slug.ends_with('-') {
    return Err(ApiError::bad_request("slug must not start or end with '-'"));
  }
  if !slug
    .chars()
    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
  {
    return Err(ApiError::bad_request(
      "slug may only contain lowercase letters, digits, and '-'",
    ));
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn accepts_valid_slug() {
    assert!(validate_slug("acme-corp-2").is_ok());
  }

  #[test]
  fn rejects_empty() {
    assert!(validate_slug("").is_err());
  }

  #[test]
  fn rejects_uppercase_and_spaces() {
    assert!(validate_slug("Acme Corp").is_err());
  }

  #[test]
  fn rejects_leading_and_trailing_dash() {
    assert!(validate_slug("-acme").is_err());
    assert!(validate_slug("acme-").is_err());
  }
}
