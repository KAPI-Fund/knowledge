mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::config::AppConfig;
use knowledge_server::tasks::{scheduler, store};
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{Value, json};
use support::TestEnvironment;
use support::mock_openai::{MockOpenAiServer, MockScenario};
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tower::util::ServiceExt;

#[tokio::test]
async fn deep_research_succeeds_with_mock_provider_and_searxng() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("deep-research-flow").await.unwrap();
    let mock_openai = MockOpenAiServer::start(MockScenario::deep_research_success())
        .await
        .unwrap();
    let (searxng_handle, searxng_base) = spawn_mock_searxng().await;
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("deep-research-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    configure_settings(&state, &mock_openai, &searxng_base).await;

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/deep-research"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "topic": "Knowledge Graphs",
                      "searchQueries": ["knowledge graphs", "rag knowledge graph"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let payload = read_json(response.into_body()).await;
    let task_id = payload["taskId"].as_str().unwrap().to_string();

    wait_for_task_terminal(&state, &task_id).await;
    let task = store::get_task_by_id(&state, &task_id).await.unwrap();
    assert_eq!(task.status, "succeeded");

    let saved = project_root.join("wiki/queries/research-knowledge-graphs.md");
    assert!(saved.exists(), "research page should be saved at {saved:?}");
    let content = std::fs::read_to_string(&saved).unwrap();
    assert!(content.contains("origin: deep-research"));
    assert!(content.contains("# Research: Knowledge Graphs"));
    assert!(content.contains("Graphs of typed entities and relations"));
    assert!(content.contains("## References"));
    assert!(content.contains("[Knowledge graphs explained]"));

    let index = std::fs::read_to_string(project_root.join("wiki/index.md")).unwrap();
    assert!(index.contains("- [[queries/research-knowledge-graphs]] - Research: Knowledge Graphs"));

    searxng_handle.abort();
}

async fn configure_settings(
    state: &knowledge_server::app::state::AppState,
    mock: &MockOpenAiServer,
    searxng_base: &str,
) {
    sqlx::query(
        "UPDATE system_settings
     SET provider_mode = $1,
         provider_base_url = $2,
         provider_api_key = $3,
         provider_model = $4,
         provider_timeout_seconds = $5,
         search_provider = $6,
         searxng_url = $7,
         searxng_categories = $8",
    )
    .bind("openai-compatible")
    .bind(mock.base_url())
    .bind("test-key")
    .bind("mock-model")
    .bind(30_i64)
    .bind("searxng")
    .bind(searxng_base)
    .bind(serde_json::json!(["general"]))
    .execute(&state.pool)
    .await
    .unwrap();
}

async fn spawn_mock_searxng() -> (tokio::task::JoinHandle<()>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let _ = socket.read(&mut buf).await;
                let body = json!({
                  "results": [
                    {
                      "title": "Knowledge graphs explained",
                      "url": "https://example.com/knowledge-graphs",
                      "content": "An overview of knowledge graphs and their applications.",
                      "engine": "duckduckgo"
                    }
                  ]
                })
                .to_string();
                let payload = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(payload.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    (handle, format!("http://127.0.0.1:{port}"))
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
    let csrf = body.get("csrfToken").and_then(Value::as_str).unwrap().to_string();
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
async fn deep_research_via_bearer_token() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("deep-research-bearer").await.unwrap();
    let mock_openai = MockOpenAiServer::start(MockScenario::deep_research_success())
        .await
        .unwrap();
    let (searxng_handle, searxng_base) = spawn_mock_searxng().await;
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("dr-bearer-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    configure_settings(&state, &mock_openai, &searxng_base).await;

    let mint = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "name": "dr", "projectId": project_id }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let token = read_json(mint.into_body()).await["token"]
        .as_str()
        .unwrap()
        .to_string();

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/deep-research"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(json!({ "topic": "Knowledge Graphs" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let task_id = read_json(response.into_body()).await["taskId"]
        .as_str()
        .unwrap()
        .to_string();
    wait_for_task_terminal(&state, &task_id).await;
    let task = store::get_task_by_id(&state, &task_id).await.unwrap();
    assert_eq!(task.status, "succeeded");

    searxng_handle.abort();
}

async fn wait_for_task_terminal(state: &knowledge_server::app::state::AppState, task_id: &str) {
    for _ in 0..120 {
        let progressed = scheduler::run_scheduler_tick(state).await.unwrap_or(false);
        let task = store::get_task_by_id(state, task_id).await.unwrap();
        if matches!(task.status.as_str(), "succeeded" | "failed" | "cancelled") {
            return;
        }
        if !progressed {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
    panic!("deep research task did not reach a terminal state");
}

#[tokio::test]
async fn deep_research_marks_task_failed_when_no_sources_found() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("deep-research-no-sources").await.unwrap();
    let mock_openai = MockOpenAiServer::start(MockScenario::deep_research_success())
        .await
        .unwrap();
    let (searxng_handle, searxng_base) = spawn_mock_searxng_with_empty_results().await;
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("dr-no-sources-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;

    configure_settings(&state, &mock_openai, &searxng_base).await;

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/deep-research"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(json!({ "topic": "Knowledge Graphs" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let task_id = read_json(response.into_body()).await["taskId"]
        .as_str()
        .unwrap()
        .to_string();
    wait_for_task_terminal(&state, &task_id).await;

    let task = store::get_task_by_id(&state, &task_id).await.unwrap();
    assert_eq!(task.status, "failed");
    let error = task.error.expect("failed task should record an error");
    let serialized = serde_json::to_string(&error).unwrap();
    assert!(
        serialized.contains("no research sources found"),
        "expected failure reason in task.error, got: {serialized}"
    );

    searxng_handle.abort();
}

async fn spawn_mock_searxng_with_empty_results() -> (tokio::task::JoinHandle<()>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let _ = socket.read(&mut buf).await;
                let body = json!({ "results": [] }).to_string();
                let payload = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(payload.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    (handle, format!("http://127.0.0.1:{port}"))
}
