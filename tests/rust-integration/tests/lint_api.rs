mod support;

use std::fs;
use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::config::AppConfig;
use knowledge_server::tasks::{scheduler, store};
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{Value, json};
use support::TestEnvironment;
use support::mock_openai::{MockOpenAiServer, MockScenario};
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn structural_lint_task_returns_orphan_broken_link_and_no_outlinks_issues() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("lint-structural").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("lint-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    fs::write(
        project_root.join("wiki/overview.md"),
        [
            "---",
            "type: overview",
            "title: Project Overview",
            "tags: []",
            "related: []",
            "sources: []",
            "---",
            "",
            "# Overview",
            "",
            "Overview links to [[overview]].",
        ]
        .join("\n"),
    )
    .unwrap();

    fs::write(
        project_root.join("wiki/concepts/attention.md"),
        [
            "---",
            "type: concept",
            "title: Attention",
            "sources: []",
            "---",
            "",
            "# Attention",
            "",
            "Attention references [[missing-page]].",
        ]
        .join("\n"),
    )
    .unwrap();
    fs::write(
        project_root.join("wiki/concepts/orphan-page.md"),
        [
            "---",
            "type: concept",
            "title: Orphan Page",
            "sources: []",
            "---",
            "",
            "# Orphan Page",
            "",
            "Standalone notes with no inbound links.",
        ]
        .join("\n"),
    )
    .unwrap();

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/lint-tasks"))
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "mode": "structural" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let payload = read_json(response.into_body()).await;
    let task_id = payload
        .get("taskId")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();
    wait_for_task_terminal(&state, &task_id).await;

    let detail = store::get_task_by_id(&state, &task_id).await.unwrap();
    assert_eq!(detail.status, "succeeded");
    assert_eq!(
        detail
            .result
            .as_ref()
            .and_then(|result| result.get("mode"))
            .and_then(Value::as_str),
        Some("structural"),
    );

    let issues = detail
        .result
        .as_ref()
        .and_then(|result| result.get("issues"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert_eq!(issues.len(), 4);

    assert!(issues.iter().any(|issue| {
        issue.get("issueType").and_then(Value::as_str) == Some("broken-link")
            && issue.get("page").and_then(Value::as_str) == Some("concepts/attention.md")
    }));
    assert!(issues.iter().any(|issue| {
        issue.get("issueType").and_then(Value::as_str) == Some("orphan")
            && issue.get("page").and_then(Value::as_str) == Some("concepts/attention.md")
    }));
    assert!(issues.iter().any(|issue| {
        issue.get("issueType").and_then(Value::as_str) == Some("orphan")
            && issue.get("page").and_then(Value::as_str) == Some("concepts/orphan-page.md")
    }));
    assert!(issues.iter().any(|issue| {
        issue.get("issueType").and_then(Value::as_str) == Some("no-outlinks")
            && issue.get("page").and_then(Value::as_str) == Some("concepts/orphan-page.md")
    }));
}

#[tokio::test]
async fn semantic_lint_task_uses_provider_and_parses_lint_blocks() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("lint-semantic").await.unwrap();
    let mock = MockOpenAiServer::start(MockScenario::semantic_lint_success())
        .await
        .unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("semantic-lint-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    support::seed_provider_connection(&state.pool, &mock.base_url(), "test-key", "mock-model", 30)
        .await;

    fs::write(
    project_root.join("wiki/concepts/attention.md"),
    "---\ntype: concept\ntitle: Attention\nsources: []\n---\n\n# Attention\n\nAttention focuses computation.\n",
  )
  .unwrap();
    fs::write(
    project_root.join("wiki/concepts/attention-mechanism.md"),
    "---\ntype: concept\ntitle: Attention Mechanism\nsources: []\n---\n\n# Attention Mechanism\n\nAttention is only a transformer subroutine.\n",
  )
  .unwrap();

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/lint-tasks"))
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "mode": "semantic" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let payload = read_json(response.into_body()).await;
    let task_id = payload
        .get("taskId")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();
    wait_for_task_terminal(&state, &task_id).await;

    let detail = store::get_task_by_id(&state, &task_id).await.unwrap();
    assert_eq!(detail.status, "succeeded");
    assert_eq!(
        detail
            .result
            .as_ref()
            .and_then(|result| result.get("mode"))
            .and_then(Value::as_str),
        Some("semantic"),
    );

    let issues = detail
        .result
        .as_ref()
        .and_then(|result| result.get("issues"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues[0].get("issueType").and_then(Value::as_str),
        Some("semantic")
    );
    assert_eq!(
        issues[0].get("severity").and_then(Value::as_str),
        Some("warning")
    );
    assert_eq!(
        issues[0].get("page").and_then(Value::as_str),
        Some("Conflicting attention claims"),
    );
    assert!(
        issues[0]
            .get("detail")
            .and_then(Value::as_str)
            .unwrap()
            .contains("[contradiction]")
    );
    assert_eq!(mock.request_count(), 1);
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

async fn wait_for_task_terminal(state: &knowledge_server::app::state::AppState, task_id: &str) {
    for _ in 0..40 {
        let progressed = scheduler::run_scheduler_tick(state).await.unwrap_or(false);
        let task = store::get_task_by_id(state, task_id).await.unwrap();
        if matches!(task.status.as_str(), "succeeded" | "failed" | "cancelled") {
            return;
        }
        if !progressed {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    panic!("task {task_id} did not reach a terminal state");
}
