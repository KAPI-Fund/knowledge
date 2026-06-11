mod support;

use std::fs;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::build_app;
use knowledge_server::config::AppConfig;
use knowledge_server::tasks::{scheduler, store};
use reqwest::Client;
use serde_json::{Value, json};
use support::{TestEnvironment, bootstrap_state_without_scheduler};
use tempfile::tempdir;
use tower::util::ServiceExt;

const DEFAULT_BASE_URL: &str = "https://backend.intelalloc.com";
const DEFAULT_MODEL: &str = "gpt-5.4";
const DEFAULT_TIMEOUT_SECONDS: u64 = 180;
const SMOKE_TIMEOUT: Duration = Duration::from_secs(240);

#[tokio::test]
async fn real_provider_contract_exposes_models_and_chat_completion() -> Result<()> {
    if !smoke_enabled() {
        eprintln!(
            "Skipping live provider smoke. Set KNOWLEDGE_RUN_REAL_PROVIDER_SMOKE=1 to run it."
        );
        return Ok(());
    }

    let provider = provider_config()?;
    let client = provider_client()?;

    let models = get_models(&client, &provider).await?;
    let model_ids = models
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();

    assert!(
        model_ids.contains(&provider.model.as_str()),
        "provider model {} was not listed by /v1/models",
        provider.model
    );

    let completion = chat_completion(&client, &provider, "Reply with a single word: ready").await?;

    let text = assistant_text(&completion);
    assert!(!text.is_empty());
    assert!(text.to_lowercase().contains("ready") || text.len() <= 32);
    assert!(usage_total_tokens(&completion).is_some());

    Ok(())
}

#[tokio::test]
async fn real_provider_query_and_save_round_trip() -> Result<()> {
    if !smoke_enabled() {
        eprintln!(
            "Skipping live provider smoke. Set KNOWLEDGE_RUN_REAL_PROVIDER_SMOKE=1 to run it."
        );
        return Ok(());
    }

    let provider = provider_config()?;
    let env = TestEnvironment::start("real-provider-query-save").await?;
    let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
    let state = bootstrap_state_without_scheduler(&config).await?;
    let (cookie, csrf) = login_and_csrf(state.clone()).await?;
    let project_temp = tempdir()?;
    let project_root = project_temp.path().join("real-provider-query-save-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await?;

    seed_query_source(&project_root)?;
    configure_provider(&state, &provider, None).await?;

    let task_id =
        create_query_task(&state, &cookie, &csrf, &project_id, "What is attention?").await?;
    let task = wait_for_task_terminal(&state, &task_id).await?;
    assert_eq!(task.status, "succeeded");

    let result = task.result.as_ref().context("missing query result")?;
    assert_eq!(
        result
            .get("citations")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        result
            .get("citations")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("path"))
            .and_then(Value::as_str),
        Some("wiki/concepts/attention.md")
    );
    assert!(
        !result
            .get("answer")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .is_empty()
    );

    let save_response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/projects/{project_id}/query-tasks/{task_id}/save"
                ))
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "title": "Attention Notes" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(save_response.status(), StatusCode::ACCEPTED);
    let save_payload = read_json(save_response.into_body()).await;
    let save_task_id = save_payload
        .get("taskId")
        .and_then(Value::as_str)
        .context("missing save task id")?;

    let save_task = wait_for_task_terminal(&state, save_task_id).await?;
    assert_eq!(save_task.status, "succeeded");
    assert_eq!(
        save_task
            .result
            .as_ref()
            .and_then(|result| result.get("relativePath"))
            .and_then(Value::as_str),
        Some("wiki/queries/attention-notes.md"),
    );

    let page = fs::read_to_string(project_root.join("wiki/queries/attention-notes.md"))?;
    assert!(page.contains("type: query"));
    assert!(page.contains("Attention Notes"));
    assert!(page.contains("## Sources"));

    let index = fs::read_to_string(project_root.join("wiki/index.md"))?;
    assert!(index.contains("[[queries/attention-notes]]"));

    let log = fs::read_to_string(project_root.join("wiki/log.md"))?;
    assert!(log.contains("query | Attention Notes"));

    Ok(())
}

#[tokio::test]
async fn real_provider_ingest_search_and_semantic_lint_smoke() -> Result<()> {
    if !smoke_enabled() {
        eprintln!(
            "Skipping live provider smoke. Set KNOWLEDGE_RUN_REAL_PROVIDER_SMOKE=1 to run it."
        );
        return Ok(());
    }

    let provider = provider_config()?;
    let env = TestEnvironment::start("real-provider-ingest-search-lint").await?;
    let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
    let state = bootstrap_state_without_scheduler(&config).await?;
    let (cookie, csrf) = login_and_csrf(state.clone()).await?;

    let search_temp = tempdir()?;
    let search_root = search_temp.path().join("real-provider-search-project");
    let search_project_id =
        create_project(state.clone(), &cookie, &csrf, search_root.clone()).await?;
    configure_provider(&state, &provider, None).await?;
    seed_search_page(&search_root)?;

    let search_response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{search_project_id}/search"))
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                      "query": "RoPE",
                      "topK": 3,
                      "includeContent": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(search_response.status(), StatusCode::OK);
    let search_payload = read_json(search_response.into_body()).await;
    assert_eq!(
        search_payload.get("mode").and_then(Value::as_str),
        Some("keyword")
    );
    assert_eq!(
        search_payload
            .get("vectorHits")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        0
    );
    assert_eq!(
        search_payload
            .get("results")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("path"))
            .and_then(Value::as_str),
        Some("wiki/concepts/rotary-position-embeddings.md"),
    );

    let ingest_temp = tempdir()?;
    let ingest_root = ingest_temp.path().join("real-provider-ingest-project");
    let ingest_project_id =
        create_project(state.clone(), &cookie, &csrf, ingest_root.clone()).await?;
    configure_provider(&state, &provider, None).await?;
    seed_ingest_source(&ingest_root)?;

    let ingest_response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{ingest_project_id}/ingest"))
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "relativePath": "raw/sources/attention.md" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(ingest_response.status(), StatusCode::ACCEPTED);
    let ingest_payload = read_json(ingest_response.into_body()).await;
    let ingest_task_id = ingest_payload
        .get("taskId")
        .and_then(Value::as_str)
        .context("missing ingest task id")?;

    let ingest_task = wait_for_task_terminal(&state, ingest_task_id).await?;
    assert_eq!(ingest_task.status, "succeeded");
    let ingest_result = ingest_task
        .result
        .as_ref()
        .context("missing ingest result")?;
    assert!(
        ingest_result
            .get("summaryPath")
            .and_then(Value::as_str)
            .is_some()
    );

    let summary_path = ingest_result
        .get("summaryPath")
        .and_then(Value::as_str)
        .context("missing summary path")?;
    let summary = fs::read_to_string(ingest_root.join(summary_path))?;
    assert!(summary.contains("type: source"));
    assert!(summary.contains("Attention"));

    let lint_temp = tempdir()?;
    let lint_root = lint_temp.path().join("real-provider-lint-project");
    let lint_project_id = create_project(state.clone(), &cookie, &csrf, lint_root.clone()).await?;
    configure_provider(&state, &provider, None).await?;
    seed_semantic_lint_pages(&lint_root)?;

    let lint_response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{lint_project_id}/lint-tasks"))
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "mode": "semantic" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(lint_response.status(), StatusCode::ACCEPTED);
    let lint_payload = read_json(lint_response.into_body()).await;
    let lint_task_id = lint_payload
        .get("taskId")
        .and_then(Value::as_str)
        .context("missing lint task id")?;

    let lint_task = wait_for_task_terminal(&state, lint_task_id).await?;
    assert_eq!(lint_task.status, "succeeded");
    let issues = lint_task
        .result
        .as_ref()
        .and_then(|result| result.get("issues"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(!issues.is_empty());

    Ok(())
}

async fn wait_for_task_terminal(
    state: &knowledge_server::app::state::AppState,
    task_id: &str,
) -> Result<knowledge_server::tasks::model::TaskRecord> {
    let deadline = tokio::time::Instant::now() + SMOKE_TIMEOUT;
    loop {
        if tokio::time::Instant::now() > deadline {
            let task = store::get_task_by_id(state, task_id).await?;
            anyhow::bail!("task {task_id} did not reach a terminal state: {task:?}");
        }

        let progressed = scheduler::run_scheduler_tick(state).await.unwrap_or(false);
        let task = store::get_task_by_id(state, task_id).await?;
        if matches!(task.status.as_str(), "succeeded" | "failed" | "cancelled") {
            return Ok(task);
        }
        if !progressed {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
}

async fn configure_provider(
    state: &knowledge_server::app::state::AppState,
    provider: &ProviderConfig,
    embedding_model: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE system_settings
     SET provider_mode = $1,
         provider_base_url = $2,
         provider_api_key = $3,
         provider_model = $4,
         provider_embedding_model = $5,
         provider_timeout_seconds = $6",
    )
    .bind("openai-compatible")
    .bind(&provider.base_url)
    .bind(&provider.api_key)
    .bind(&provider.model)
    .bind(embedding_model.map(str::to_string))
    .bind(provider.timeout_seconds as i64)
    .execute(&state.pool)
    .await
    .map_err(anyhow::Error::from)?;

    Ok(())
}

async fn create_query_task(
    state: &knowledge_server::app::state::AppState,
    cookie: &str,
    csrf: &str,
    project_id: &str,
    query: &str,
) -> Result<String> {
    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/query-tasks"))
                .header(header::COOKIE, cookie)
                .header("x-csrf-token", csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                      "query": query,
                      "topK": 3
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let payload = read_json(response.into_body()).await;
    Ok(payload
        .get("taskId")
        .and_then(Value::as_str)
        .context("missing query task id")?
        .to_string())
}

async fn login_and_csrf(state: knowledge_server::app::state::AppState) -> Result<(String, String)> {
    let response = build_app(state)
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

    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .context("missing session cookie")?
        .to_str()
        .context("invalid cookie header")?
        .to_string();
    let body = read_json(response.into_body()).await;
    let csrf = body
        .get("csrfToken")
        .and_then(Value::as_str)
        .context("missing csrf token")?
        .to_string();

    Ok((cookie, csrf))
}

async fn create_project(
    state: knowledge_server::app::state::AppState,
    cookie: &str,
    csrf: &str,
    project_root: std::path::PathBuf,
) -> Result<String> {
    Ok(support::create_project_with_alias(state, cookie, csrf, project_root).await)
}

async fn get_models(client: &Client, provider: &ProviderConfig) -> Result<Value> {
    let response = client
        .get(models_url(&provider.base_url))
        .headers(provider.auth_headers()?)
        .send()
        .await
        .context("provider models request failed")?;
    let status = response.status();
    let body = response
        .text()
        .await
        .context("failed to read provider models response body")?;
    assert!(
        status.is_success(),
        "provider models request failed with HTTP {status}: {body}"
    );
    let payload: Value = serde_json::from_str(&body).context("invalid provider models JSON")?;
    Ok(payload)
}

async fn chat_completion(
    client: &Client,
    provider: &ProviderConfig,
    prompt: &str,
) -> Result<Value> {
    let response = client
        .post(chat_url(&provider.base_url))
        .headers(provider.auth_headers()?)
        .json(&json!({
          "model": provider.model,
          "messages": [
            { "role": "system", "content": "You are a precise assistant." },
            { "role": "user", "content": prompt }
          ],
          "temperature": 0.0
        }))
        .send()
        .await
        .context("provider chat request failed")?;
    let status = response.status();
    let body = response
        .text()
        .await
        .context("failed to read provider chat response body")?;
    assert!(
        status.is_success(),
        "provider chat request failed with HTTP {status}: {body}"
    );
    let payload: Value = serde_json::from_str(&body).context("invalid provider chat JSON")?;
    Ok(payload)
}

fn assistant_text(payload: &Value) -> String {
    payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn usage_total_tokens(payload: &Value) -> Option<u64> {
    payload
        .get("usage")
        .and_then(|usage| usage.get("total_tokens"))
        .and_then(Value::as_u64)
}

fn chat_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{trimmed}/chat/completions")
    } else {
        format!("{trimmed}/v1/chat/completions")
    }
}

fn models_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{trimmed}/models")
    } else {
        format!("{trimmed}/v1/models")
    }
}

fn seed_query_source(project_root: &std::path::Path) -> Result<()> {
    fs::write(
        project_root.join("wiki/concepts/attention.md"),
        [
            "---",
            "type: concept",
            "title: Attention",
            "sources: [\"attention.md\"]",
            "---",
            "",
            "# Attention",
            "",
            "Attention lets models focus on relevant tokens.",
        ]
        .join("\n"),
    )?;

    Ok(())
}

fn seed_search_page(project_root: &std::path::Path) -> Result<()> {
    fs::write(
        project_root.join("wiki/concepts/rotary-position-embeddings.md"),
        [
            "---",
            "type: concept",
            "title: Rotary Position Embeddings",
            "sources: []",
            "---",
            "",
            "# Rotary Position Embeddings",
            "",
            "RoPE is short for rotary positional embeddings.",
        ]
        .join("\n"),
    )?;

    Ok(())
}

fn seed_ingest_source(project_root: &std::path::Path) -> Result<()> {
    fs::write(
        project_root.join("raw/sources/attention.md"),
        [
            "# Attention",
            "",
            "Transformers use attention mechanisms to focus computation on relevant tokens.",
        ]
        .join("\n"),
    )?;

    Ok(())
}

fn seed_semantic_lint_pages(project_root: &std::path::Path) -> Result<()> {
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
            "Attention uses softmax over query and key similarities.",
        ]
        .join("\n"),
    )?;
    fs::write(
        project_root.join("wiki/concepts/attention-variant.md"),
        [
            "---",
            "type: concept",
            "title: Attention Variant",
            "sources: []",
            "---",
            "",
            "# Attention Variant",
            "",
            "Attention uses Gaussian kernels instead of softmax.",
        ]
        .join("\n"),
    )?;

    Ok(())
}

async fn read_json(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn smoke_enabled() -> bool {
    std::env::var("KNOWLEDGE_RUN_REAL_PROVIDER_SMOKE").unwrap_or_default() == "1"
}

fn provider_config() -> Result<ProviderConfig> {
    let base_url = std::env::var("KNOWLEDGE_PROVIDER_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
    let api_key = std::env::var("KNOWLEDGE_PROVIDER_API_KEY")
        .context("KNOWLEDGE_PROVIDER_API_KEY is required for the real provider smoke suite")?;
    let model =
        std::env::var("KNOWLEDGE_PROVIDER_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
    let timeout_seconds = std::env::var("KNOWLEDGE_PROVIDER_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECONDS);

    Ok(ProviderConfig {
        base_url,
        api_key,
        model,
        timeout_seconds,
    })
}

fn provider_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECONDS))
        .build()
        .context("failed to build reqwest client")
}

#[derive(Debug, Clone)]
struct ProviderConfig {
    base_url: String,
    api_key: String,
    model: String,
    timeout_seconds: u64,
}

impl ProviderConfig {
    fn auth_headers(&self) -> Result<reqwest::header::HeaderMap> {
        let mut headers = reqwest::header::HeaderMap::new();
        if self.api_key.trim().is_empty() {
            return Ok(headers);
        }

        let value =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", self.api_key.trim()))
                .context("invalid provider api key")?;
        headers.insert(reqwest::header::AUTHORIZATION, value);
        Ok(headers)
    }
}
