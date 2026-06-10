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
async fn admin_can_create_project_with_generated_root() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("project-create").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let project_root_dir = temp.path().join("project-roots");
    std::fs::create_dir_all(&project_root_dir).unwrap();
    let project_root_dir = project_root_dir.to_string_lossy().to_string();
    let mut config = config;
    config.project_root = project_root_dir.clone();
    let state = bootstrap_state(&config).await.unwrap();

    let login_app = build_app(state.clone());
    let login = login(login_app).await;
    let cookie = login
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let body = read_json(login.into_body()).await;
    let csrf = body.get("csrfToken").and_then(Value::as_str).unwrap();

    let app = build_app(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/projects")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, cookie)
                .header("x-csrf-token", csrf)
                .body(Body::from(json!({ "name": "knowledge-demo" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let project = read_json(response.into_body()).await;
    let root_path = project.get("rootPath").and_then(Value::as_str).unwrap();
    let root_path = normalized_path(root_path);
    assert_eq!(
        root_path.parent().unwrap(),
        std::path::Path::new(&project_root_dir)
    );
    assert!(root_path.join("purpose.md").is_file());
    assert!(root_path.join("schema.md").is_file());
    assert!(root_path.join("wiki/index.md").is_file());
    assert!(root_path.join("wiki/log.md").is_file());
    assert!(root_path.join("wiki/overview.md").is_file());
    assert!(root_path.join("raw/sources").is_dir());
    assert!(root_path.join("raw/assets").is_dir());
    assert!(root_path.join(".knowledge/ingest").is_dir());
    assert!(root_path.join(".obsidian/app.json").is_file());
    assert!(root_path.join(".obsidian/appearance.json").is_file());
    assert!(root_path.join(".obsidian/core-plugins.json").is_file());

    let schema = std::fs::read_to_string(root_path.join("schema.md")).unwrap();
    assert!(schema.contains("| entity | wiki/entities/ |"));
    assert!(schema.contains("## Cross-referencing Rules"));

    let purpose = std::fs::read_to_string(root_path.join("purpose.md")).unwrap();
    assert!(purpose.contains("## Scope"));
    assert!(purpose.contains("## Thesis"));

    let log = std::fs::read_to_string(root_path.join("wiki/log.md")).unwrap();
    assert!(log.contains("Project created"));
}

#[tokio::test]
async fn non_member_cannot_list_project_members() {
    let _env = TestEnvironment::start("project-members").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let app = build_app(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/projects/project-1/members")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_projects_and_users_return_registered_data() {
    let _env = TestEnvironment::start("project-listing").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();

    let login_app = build_app(state.clone());
    let login = login(login_app).await;
    let cookie = login
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let body = read_json(login.into_body()).await;
    let csrf = body.get("csrfToken").and_then(Value::as_str).unwrap();

    let _ = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/projects")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", csrf)
                .body(Body::from(json!({ "name": "listed-project" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let projects_response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/projects")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(projects_response.status(), StatusCode::OK);
    let projects_payload = read_json(projects_response.into_body()).await;
    let projects = projects_payload
        .get("projects")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(
        projects[0].get("name").and_then(Value::as_str),
        Some("listed-project")
    );

    let users_response = build_app(state)
        .oneshot(
            Request::builder()
                .uri("/api/users")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(users_response.status(), StatusCode::OK);
    let users_payload = read_json(users_response.into_body()).await;
    let users = users_payload
        .get("users")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(
        users[0].get("username").and_then(Value::as_str),
        Some("admin")
    );
}

async fn login(app: axum::Router) -> axum::response::Response {
    app.oneshot(
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
    .unwrap()
}

async fn read_json(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn normalized_path(path: &str) -> std::path::PathBuf {
    let value = if let Some(stripped) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", stripped)
    } else {
        path.strip_prefix(r"\\?\").unwrap_or(path).to_string()
    };
    std::path::PathBuf::from(value)
}
