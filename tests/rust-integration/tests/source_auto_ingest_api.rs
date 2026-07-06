mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::app::state::AppState;
use knowledge_server::config::AppConfig;
use knowledge_server::tasks::{scheduler, store};
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{Value, json};
use support::{seed_provider_connection, TestEnvironment};
use tempfile::tempdir;
use tower::util::ServiceExt;

/// Base64 of "# Note\n\nHello from source.\n" — an ingestable text source.
const MARKDOWN_BASE64: &str = "IyBOb3RlCgpIZWxsbyBmcm9tIHNvdXJjZS4K";

/// Importing an ingestable source while an active LLM connection exists must
/// automatically enqueue its ingest, so an upload produces the wiki graph/index
/// with no separate manual step (copies upstream_llm_wiki importSourceFiles →
/// enqueueSourceIngest).
#[tokio::test]
async fn importing_ingestable_source_with_active_connection_enqueues_ingest() {
    let temp = tempdir().unwrap();
    let env = TestEnvironment::start("auto-ingest-enqueue").await.unwrap();
    let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();

    // An active connection is the backend analog of upstream's hasUsableLlm guard.
    seed_provider_connection(&state.pool, "http://127.0.0.1:1/v1", "sk-test", "gpt-4o", 30).await;

    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_id = create_project(state.clone(), &cookie, &csrf, temp.path().join("auto-ingest")).await;

    import_and_wait(&state, &cookie, &csrf, &project_id, "note.md", MARKDOWN_BASE64).await;

    let ingest_tasks = ingest_relative_paths(&state, &project_id).await;
    assert_eq!(
        ingest_tasks,
        vec!["raw/sources/note.md".to_string()],
        "an active connection + ingestable source must enqueue exactly one ingest task"
    );
}

/// Without an active connection the file still imports, but no ingest is queued —
/// we don't pile up tasks that could only fail (upstream's `if (!hasUsableLlm)`
/// guard).
#[tokio::test]
async fn importing_without_active_connection_does_not_enqueue_ingest() {
    let temp = tempdir().unwrap();
    let env = TestEnvironment::start("auto-ingest-no-conn").await.unwrap();
    let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();

    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_id = create_project(state.clone(), &cookie, &csrf, temp.path().join("auto-ingest-no-conn")).await;

    import_and_wait(&state, &cookie, &csrf, &project_id, "note.md", MARKDOWN_BASE64).await;

    assert!(
        ingest_relative_paths(&state, &project_id).await.is_empty(),
        "no active connection must leave the file imported but un-ingested"
    );
}

/// Non-ingestable sources (images/media) import but never auto-ingest, even with
/// an active connection (upstream isIngestableSourcePath / INGESTABLE_SOURCE_EXTENSIONS).
#[tokio::test]
async fn importing_non_ingestable_source_does_not_enqueue_ingest() {
    let temp = tempdir().unwrap();
    let env = TestEnvironment::start("auto-ingest-image").await.unwrap();
    let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();

    seed_provider_connection(&state.pool, "http://127.0.0.1:1/v1", "sk-test", "gpt-4o", 30).await;

    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_id = create_project(state.clone(), &cookie, &csrf, temp.path().join("auto-ingest-image")).await;

    // "iVBORw0KGgo=" is a few bytes of base64; content is irrelevant to the import.
    import_and_wait(&state, &cookie, &csrf, &project_id, "diagram.png", "iVBORw0KGgo=").await;

    assert!(
        ingest_relative_paths(&state, &project_id).await.is_empty(),
        "images are imported but must not be auto-ingested"
    );
}

async fn import_and_wait(
    state: &AppState,
    cookie: &str,
    csrf: &str,
    project_id: &str,
    file_name: &str,
    content_base64: &str,
) {
    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/sources:import"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, cookie)
                .header("x-csrf-token", csrf)
                .body(Body::from(
                    json!({ "fileName": file_name, "contentBase64": content_base64 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let payload = read_json(response.into_body()).await;
    let task_id = payload.get("taskId").and_then(Value::as_str).unwrap();
    wait_for_task_terminal(state, task_id).await;
}

/// Relative paths of every queued/enqueued `project.ingest_source` task for a
/// project, sorted for a stable assertion.
async fn ingest_relative_paths(state: &AppState, project_id: &str) -> Vec<String> {
    let mut paths = sqlx::query_scalar::<_, String>(
        "SELECT payload->>'relativePath' FROM project_tasks
         WHERE project_id = $1 AND task_type = 'project.ingest_source'",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    paths.sort();
    paths
}

async fn login_and_csrf(state: AppState) -> (String, String) {
    let login = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "username": "admin", "password": "secret-password" }).to_string(),
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

async fn create_project(state: AppState, cookie: &str, csrf: &str, project_root: std::path::PathBuf) -> String {
    support::create_project_with_alias(state, cookie, csrf, project_root).await
}

async fn read_json(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn wait_for_task_terminal(state: &AppState, task_id: &str) {
    for _ in 0..20 {
        let progressed = scheduler::run_scheduler_tick(state).await.unwrap_or(false);
        let task = store::get_task_by_id(state, task_id).await.unwrap();
        if matches!(task.status.as_str(), "succeeded" | "failed" | "cancelled") {
            return;
        }
        if !progressed {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    panic!("task did not reach a terminal state");
}
