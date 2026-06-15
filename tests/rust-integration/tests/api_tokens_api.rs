mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{Value, json};
use support::TestEnvironment;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn api_token_lifecycle_create_use_list_revoke() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("api-tokens-lifecycle").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("api-tokens-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;

    let mint = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "name": "test-token", "projectId": null }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mint.status(), StatusCode::CREATED);
    let body = read_json(mint.into_body()).await;
    let token = body["token"].as_str().unwrap().to_string();
    let token_id = body["id"].as_str().unwrap().to_string();
    assert!(!body["prefix"].as_str().unwrap().is_empty());

    let listing = build_app(state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/projects")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listing.status(), StatusCode::OK);

    let revoke = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/users/me/api-tokens/{token_id}/revoke"))
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoke.status(), StatusCode::OK);

    let after = build_app(state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/projects")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(after.status(), StatusCode::UNAUTHORIZED);

    let list = build_app(state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/users/me/api-tokens")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let listed = read_json(list.into_body()).await;
    let tokens = listed["tokens"].as_array().unwrap();
    assert_eq!(tokens.len(), 1);
    assert!(tokens[0]["revokedAt"].as_str().is_some());

    let _ = project_id;
}

#[tokio::test]
async fn project_scoped_token_rejected_for_other_projects() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("api-tokens-scope").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_a_root = temp.path().join("project-a");
    let project_a = create_project(state.clone(), &cookie, &csrf, project_a_root).await;
    let project_b_root = temp.path().join("project-b");
    let project_b = create_project(state.clone(), &cookie, &csrf, project_b_root).await;

    let mint = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "name": "scoped", "projectId": project_a }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mint.status(), StatusCode::CREATED);
    let token = read_json(mint.into_body()).await["token"].as_str().unwrap().to_string();

    let allowed = build_app(state.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_a}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);

    let denied = build_app(state.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_b}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
}

async fn login_and_csrf(state: knowledge_server::app::state::AppState) -> (String, String) {
    let login = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                      "username": "admin",
                      "password": "secret-password"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let cookie = login
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let body = read_json(login.into_body()).await;
    let csrf = body
        .get("csrfToken")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();
    (cookie, csrf)
}

async fn create_project(
    state: knowledge_server::app::state::AppState,
    cookie: &str,
    csrf: &str,
    project_root: std::path::PathBuf,
) -> String {
    support::create_project_with_alias(state, cookie, csrf, project_root).await
}

async fn read_json(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn project_scoped_token_cannot_mint_unscoped() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("api-tokens-no-escalate-mint").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("scope-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;

    let scoped_token = mint_and_id(state.clone(), &cookie, &csrf, Some(&project_id)).await.1;

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header("authorization", format!("Bearer {scoped_token}"))
                .body(Body::from(
                    json!({ "name": "escalation", "projectId": null }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn project_scoped_token_cannot_mint_for_other_project() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("api-tokens-no-escalate-cross").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_a_root = temp.path().join("project-a");
    let project_a = create_project(state.clone(), &cookie, &csrf, project_a_root).await;
    let project_b_root = temp.path().join("project-b");
    let project_b = create_project(state.clone(), &cookie, &csrf, project_b_root).await;

    let scoped_a = mint_and_id(state.clone(), &cookie, &csrf, Some(&project_a)).await.1;

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header("authorization", format!("Bearer {scoped_a}"))
                .body(Body::from(
                    json!({ "name": "cross", "projectId": project_b }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn project_scoped_token_list_and_revoke_only_see_same_scope() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("api-tokens-scope-isolation").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("scope-iso");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;

    let unscoped_id = mint_and_id(state.clone(), &cookie, &csrf, None).await;
    let scoped_id = mint_and_id(state.clone(), &cookie, &csrf, Some(&project_id)).await;
    let scoped_token = scoped_id.1.clone();

    let listed = build_app(state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/users/me/api-tokens")
                .header("authorization", format!("Bearer {}", scoped_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let payload = read_json(listed.into_body()).await;
    let ids = payload["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|token| token["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(ids.contains(&scoped_id.0), "scoped token should see itself");
    assert!(!ids.contains(&unscoped_id.0), "scoped token must not see unscoped tokens");

    let denied = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/users/me/api-tokens/{}/revoke", unscoped_id.0))
                .header("authorization", format!("Bearer {}", scoped_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);

    let allowed = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/users/me/api-tokens/{}/revoke", scoped_id.0))
                .header("authorization", format!("Bearer {}", scoped_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);
}

async fn mint_and_id(
    state: knowledge_server::app::state::AppState,
    cookie: &str,
    csrf: &str,
    project_id: Option<&str>,
) -> (String, String) {
    let body = match project_id {
        Some(value) => json!({ "name": "test", "projectId": value }),
        None => json!({ "name": "test", "projectId": null }),
    };
    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, cookie)
                .header("x-csrf-token", csrf)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let payload = read_json(response.into_body()).await;
    (
        payload["id"].as_str().unwrap().to_string(),
        payload["token"].as_str().unwrap().to_string(),
    )
}
