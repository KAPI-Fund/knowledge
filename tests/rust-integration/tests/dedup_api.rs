mod support;

use std::fs;

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
async fn dedup_detect_finds_candidate_groups() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("dedup-detect").await.unwrap();
    let mock = MockOpenAiServer::start(MockScenario::dedup_success())
        .await
        .unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("dedup-detect-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    configure_mock_provider(&state, &mock).await;
    write_dedup_fixture_pages(&project_root);

    let task_id = enqueue_detect(state.clone(), &cookie, &csrf, &project_id).await;
    wait_for_task_terminal(&state, &task_id).await;
    let task = store::get_task_by_id(&state, &task_id).await.unwrap();
    assert_eq!(task.status, "succeeded");

    let overview = get_dedup_overview(state.clone(), &cookie, &project_id).await;
    let groups = overview.get("groups").and_then(Value::as_array).unwrap();
    assert_eq!(groups.len(), 1);
    let group = &groups[0];
    assert_eq!(
        group.get("slugs").and_then(Value::as_array).map(Vec::len),
        Some(2)
    );
    assert_eq!(group.get("confidence").and_then(Value::as_str), Some("high"));
    assert_eq!(group.get("status").and_then(Value::as_str), Some("candidate"));
    assert!(group.get("id").and_then(Value::as_str).is_some());
}

#[tokio::test]
async fn dedup_merge_unifies_pages_and_rewrites_references() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("dedup-merge").await.unwrap();
    let mock = MockOpenAiServer::start(MockScenario::dedup_success())
        .await
        .unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("dedup-merge-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    configure_mock_provider(&state, &mock).await;
    write_dedup_fixture_pages(&project_root);

    let detect_task = enqueue_detect(state.clone(), &cookie, &csrf, &project_id).await;
    wait_for_task_terminal(&state, &detect_task).await;
    let overview = get_dedup_overview(state.clone(), &cookie, &project_id).await;
    let group_id = overview["groups"][0]["id"].as_str().unwrap().to_string();

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/projects/{project_id}/dedup/groups/{group_id}/merge"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "canonicalSlug": "attention-mechanism" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let payload = read_json(response.into_body()).await;
    let merge_task = payload["taskId"].as_str().unwrap().to_string();
    wait_for_task_terminal(&state, &merge_task).await;
    let task = store::get_task_by_id(&state, &merge_task).await.unwrap();
    assert_eq!(task.status, "succeeded");

    assert!(!project_root.join("wiki/concepts/attention.md").exists());
    let canonical =
        fs::read_to_string(project_root.join("wiki/concepts/attention-mechanism.md")).unwrap();
    assert!(canonical.contains("Attention focuses computation on relevant tokens across the sequence."));
    assert!(canonical.contains("\"a.md\""));
    assert!(canonical.contains("\"b.md\""));

    let paper = fs::read_to_string(project_root.join("wiki/sources/transformer-paper.md")).unwrap();
    assert!(paper.contains("[[attention-mechanism]]"));
    assert!(!paper.contains("[[attention]] "));
    assert!(paper.contains("related: [\"attention-mechanism\"]"));

    let index = fs::read_to_string(project_root.join("wiki/index.md")).unwrap();
    assert!(index.contains("[[attention-mechanism]]"));
    assert!(!index.contains("[[attention]] "));

    let page_ids: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT page_id FROM project_embedding_chunks WHERE project_id = $1 ORDER BY page_id",
    )
    .bind(&project_id)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert!(
        !page_ids
            .iter()
            .any(|(page_id,)| page_id == "wiki/concepts/attention.md")
    );

    let overview = get_dedup_overview(state.clone(), &cookie, &project_id).await;
    assert!(overview["groups"].as_array().unwrap().is_empty());
    let backups_root = project_root.join(".knowledge/dedup/backups");
    assert!(backups_root.exists());
    assert!(fs::read_dir(&backups_root).unwrap().next().is_some());
}

#[tokio::test]
async fn dedup_dismiss_whitelists_group_for_future_detection() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("dedup-dismiss").await.unwrap();
    let mock = MockOpenAiServer::start(MockScenario::dedup_success())
        .await
        .unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("dedup-dismiss-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    configure_mock_provider(&state, &mock).await;
    write_dedup_fixture_pages(&project_root);

    let detect_task = enqueue_detect(state.clone(), &cookie, &csrf, &project_id).await;
    wait_for_task_terminal(&state, &detect_task).await;
    let overview = get_dedup_overview(state.clone(), &cookie, &project_id).await;
    let group_id = overview["groups"][0]["id"].as_str().unwrap().to_string();

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/projects/{project_id}/dedup/groups/{group_id}/dismiss"
                ))
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = read_json(response.into_body()).await;
    assert_eq!(payload.get("dismissed").and_then(Value::as_bool), Some(true));

    let overview = get_dedup_overview(state.clone(), &cookie, &project_id).await;
    assert!(overview["groups"].as_array().unwrap().is_empty());
    let not_duplicates = overview["notDuplicates"].as_array().unwrap();
    assert_eq!(not_duplicates.len(), 1);

    let detect_task = enqueue_detect(state.clone(), &cookie, &csrf, &project_id).await;
    wait_for_task_terminal(&state, &detect_task).await;
    let overview = get_dedup_overview(state, &cookie, &project_id).await;
    assert!(overview["groups"].as_array().unwrap().is_empty());
}

async fn configure_mock_provider(
    state: &knowledge_server::app::state::AppState,
    mock: &MockOpenAiServer,
) {
    support::seed_provider_connection(&state.pool, &mock.base_url(), "test-key", "mock-model", 30)
        .await;
    support::seed_embedding(&state.pool, &mock.base_url(), "test-key", "mock-embedding", 30).await;
}

fn write_dedup_fixture_pages(project_root: &std::path::Path) {
    fs::write(
        project_root.join("wiki/concepts/attention.md"),
        "---\ntype: concept\ntitle: Attention\nsources: [\"a.md\"]\ntags: [transformers]\nrelated: [transformer]\n---\n\n# Attention\n\nAttention weighs token relevance.\n",
    )
    .unwrap();
    fs::write(
        project_root.join("wiki/concepts/attention-mechanism.md"),
        "---\ntype: concept\ntitle: Attention Mechanism\nsources: [\"b.md\"]\n---\n\n# Attention Mechanism\n\nThe attention mechanism scores pairs of tokens.\n",
    )
    .unwrap();
    fs::write(
        project_root.join("wiki/entities/rope.md"),
        "---\ntype: entity\ntitle: RoPE\nsources: []\n---\n\n# RoPE\n\nRoPE rotates query vectors.\n",
    )
    .unwrap();
    fs::write(
        project_root.join("wiki/sources/transformer-paper.md"),
        "---\ntype: source\ntitle: Transformer Paper\nrelated: [attention]\n---\n\n# Transformer Paper\n\nSee [[attention]] for details.\n",
    )
    .unwrap();
    fs::write(
        project_root.join("wiki/index.md"),
        "# Index\n\n- [[attention]] - token relevance weighting\n- [[attention-mechanism]] - pairwise token scoring\n- [[rope]] - rotary embeddings\n",
    )
    .unwrap();
}

async fn enqueue_detect(
    state: knowledge_server::app::state::AppState,
    cookie: &str,
    csrf: &str,
    project_id: &str,
) -> String {
    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/dedup:detect"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let payload = read_json(response.into_body()).await;
    payload
        .get("taskId")
        .and_then(Value::as_str)
        .unwrap()
        .to_string()
}

async fn get_dedup_overview(
    state: knowledge_server::app::state::AppState,
    cookie: &str,
    project_id: &str,
) -> Value {
    let response = build_app(state)
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_id}/dedup"))
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    read_json(response.into_body()).await
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

    panic!("task did not reach a terminal state");
}
