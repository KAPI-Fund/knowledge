use axum::http::{HeaderMap, header};

use crate::app::state::AppState;
use crate::auth::api_token::{find_by_token, touch_last_used};
use crate::auth::session::find_session;
use crate::http::error::ApiError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthScope {
  Session,
  ApiToken,
}

#[derive(Debug, Clone)]
pub struct Principal {
  pub user_id: String,
  /// Present only for session principals.
  pub csrf_token: Option<String>,
  pub scope: AuthScope,
  /// Present only for api-token principals — used to update last_used_at.
  pub token_id: Option<String>,
  /// Present only for project-scoped api tokens. None = unscoped.
  pub project_id: Option<String>,
}

impl Principal {
  /// Sessions need CSRF validation on mutating routes; bearer tokens don't
  /// (no ambient credential to attack).
  pub fn requires_csrf(&self) -> bool {
    matches!(self.scope, AuthScope::Session)
  }

  /// True when this principal is allowed to act on the named project.
  pub fn permits_project(&self, project_id: &str) -> bool {
    match (&self.scope, &self.project_id) {
      (AuthScope::Session, _) => true,
      (AuthScope::ApiToken, None) => true,
      (AuthScope::ApiToken, Some(scope)) => scope == project_id,
    }
  }
}

pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
  let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
  let mut parts = value.splitn(2, char::is_whitespace);
  let scheme = parts.next()?;
  let token = parts.next()?;
  if !scheme.eq_ignore_ascii_case("bearer") {
    return None;
  }
  let trimmed = token.trim();
  if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

pub fn extract_session_cookie(headers: &HeaderMap) -> Option<String> {
  headers
    .get(header::COOKIE)
    .and_then(|value| value.to_str().ok())
    .and_then(|cookie| {
      cookie
        .split(';')
        .map(str::trim)
        .find(|item| item.starts_with("knowledge_session="))
        .map(|item| item.trim_start_matches("knowledge_session=").to_string())
    })
}

/// Resolve a Principal from cookie OR Bearer. Tries Bearer first because
/// API clients explicitly opt in by sending Authorization; if both are
/// present, the Bearer wins (the more explicit credential).
pub async fn resolve_principal(
  state: &AppState,
  headers: &HeaderMap,
) -> Result<Principal, ApiError> {
  if let Some(plaintext) = extract_bearer_token(headers)
    && let Some(token) = find_by_token(state, &plaintext).await?
  {
    if let Err(error) = touch_last_used(state, &token.id).await {
      tracing::warn!("failed to update api_tokens.last_used_at: {error}");
    }
    return Ok(Principal {
      user_id: token.user_id,
      csrf_token: None,
      scope: AuthScope::ApiToken,
      token_id: Some(token.id),
      project_id: token.project_id,
    });
  }

  let session_id =
    extract_session_cookie(headers).ok_or_else(|| ApiError::unauthorized("missing session"))?;
  let session = find_session(state, &session_id)
    .await?
    .ok_or_else(|| ApiError::unauthorized("missing session"))?;
  Ok(Principal {
    user_id: session.user_id,
    csrf_token: Some(session.csrf_token),
    scope: AuthScope::Session,
    token_id: None,
    project_id: None,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use axum::http::HeaderMap;

  #[test]
  fn extract_bearer_token_reads_authorization_header() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer abc123".parse().unwrap());
    assert_eq!(extract_bearer_token(&headers), Some("abc123".to_string()));
  }

  #[test]
  fn extract_bearer_token_is_case_insensitive_on_scheme() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "bearer abc123".parse().unwrap());
    assert_eq!(extract_bearer_token(&headers), Some("abc123".to_string()));
  }

  #[test]
  fn extract_bearer_token_returns_none_when_missing() {
    let headers = HeaderMap::new();
    assert!(extract_bearer_token(&headers).is_none());
  }

  #[test]
  fn extract_bearer_token_returns_none_for_other_schemes() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Basic abc123".parse().unwrap());
    assert!(extract_bearer_token(&headers).is_none());
  }

  #[test]
  fn principal_requires_csrf_only_for_sessions() {
    let session = Principal {
      user_id: "user-1".to_string(),
      csrf_token: Some("csrf-1".to_string()),
      scope: AuthScope::Session,
      token_id: None,
      project_id: None,
    };
    let bearer = Principal {
      user_id: "user-1".to_string(),
      csrf_token: None,
      scope: AuthScope::ApiToken,
      token_id: Some("token-1".to_string()),
      project_id: None,
    };
    assert!(session.requires_csrf());
    assert!(!bearer.requires_csrf());
  }

  #[test]
  fn principal_project_scope_check_passes_when_token_is_unscoped() {
    let principal = Principal {
      user_id: "user-1".to_string(),
      csrf_token: None,
      scope: AuthScope::ApiToken,
      token_id: Some("token-1".to_string()),
      project_id: None,
    };
    assert!(principal.permits_project("any-project"));
  }

  #[test]
  fn principal_project_scope_check_passes_when_token_matches_project() {
    let principal = Principal {
      user_id: "user-1".to_string(),
      csrf_token: None,
      scope: AuthScope::ApiToken,
      token_id: Some("token-1".to_string()),
      project_id: Some("project-1".to_string()),
    };
    assert!(principal.permits_project("project-1"));
    assert!(!principal.permits_project("project-2"));
  }

  #[test]
  fn principal_project_scope_check_session_principals_always_permit() {
    let principal = Principal {
      user_id: "user-1".to_string(),
      csrf_token: Some("csrf-1".to_string()),
      scope: AuthScope::Session,
      token_id: None,
      project_id: None,
    };
    assert!(principal.permits_project("any-project"));
  }
}
