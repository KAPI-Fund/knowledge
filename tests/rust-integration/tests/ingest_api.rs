mod support;

use std::fs;
use std::io::Write;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use base64::Engine;
use knowledge_server::config::AppConfig;
use knowledge_server::tasks::{scheduler, store};
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{Value, json};
use support::TestEnvironment;
use support::mock_openai::{MockOpenAiServer, MockScenario};
use tempfile::tempdir;
use tower::util::ServiceExt;
use zip::ZipWriter;
use zip::write::FileOptions;

#[tokio::test]
async fn ingest_source_generates_summary_and_updates_indexes() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("ingest-source").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("ingest-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    let import_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/sources:import"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({
            "fileName": "attention.md",
            "contentBase64": "IyBBdHRlbnRpb24KClRyYW5zZm9ybWVycyB1c2UgYXR0ZW50aW9uIG1lY2hhbmlzbXMuCg=="
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(import_response.status(), StatusCode::ACCEPTED);
    let import_payload = read_json(import_response.into_body()).await;
    wait_for_task_terminal(
        &state,
        import_payload
            .get("taskId")
            .and_then(Value::as_str)
            .unwrap(),
    )
    .await;

    let ingest_response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/ingest"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "relativePath": "raw/sources/attention.md" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(ingest_response.status(), StatusCode::ACCEPTED);
    let payload = read_json(ingest_response.into_body()).await;
    let task_id = payload
        .get("taskId")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();
    wait_for_task_terminal(&state, &task_id).await;
    let detail = store::get_task_by_id(&state, &task_id).await.unwrap();
    assert_eq!(
        detail
            .result
            .as_ref()
            .and_then(|result| result.get("summaryPath"))
            .and_then(Value::as_str),
        Some("wiki/sources/attention.md"),
    );

    let summary = fs::read_to_string(project_root.join("wiki/sources/attention.md")).unwrap();
    assert!(summary.contains("type: source"));
    assert!(summary.contains("sources: [\"attention.md\"]"));
    assert!(summary.contains("# Attention"));

    let index = fs::read_to_string(project_root.join("wiki/index.md")).unwrap();
    assert!(index.contains("[[attention]]"));

    let log = fs::read_to_string(project_root.join("wiki/log.md")).unwrap();
    assert!(log.contains("ingest | Attention"));

    let overview = fs::read_to_string(project_root.join("wiki/overview.md")).unwrap();
    assert!(overview.contains("Attention"));

    let analysis_checkpoint = fs::read_to_string(
        project_root.join(".knowledge/ingest/checkpoints/attention.analysis.json"),
    )
    .unwrap();
    assert!(analysis_checkpoint.contains("\"title\":\"Attention\""));

    let generation_checkpoint = fs::read_to_string(
        project_root.join(".knowledge/ingest/checkpoints/attention.generation.json"),
    )
    .unwrap();
    assert!(generation_checkpoint.contains("\"summaryPath\":\"wiki/sources/attention.md\""));
}

#[tokio::test]
async fn ingest_source_uses_provider_two_stage_generation_and_writes_multiple_pages() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("ingest-provider").await.unwrap();
    let mock = MockOpenAiServer::start(MockScenario::ingest_success())
        .await
        .unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("ingest-provider-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;
    configure_provider(&state, &mock.base_url(), "mock-model").await;

    fs::write(
        project_root.join("raw/sources/attention.md"),
        "# Attention\n\nTransformers use attention mechanisms.\n",
    )
    .unwrap();

    let ingest_response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/ingest"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "relativePath": "raw/sources/attention.md" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(ingest_response.status(), StatusCode::ACCEPTED);
    let payload = read_json(ingest_response.into_body()).await;
    let task_id = payload
        .get("taskId")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();
    wait_for_task_terminal(&state, &task_id).await;

    let detail = store::get_task_by_id(&state, &task_id).await.unwrap();
    assert_eq!(detail.status, "succeeded", "{detail:?}");
    assert_eq!(
        detail.result.as_ref().unwrap()["summaryPath"],
        Value::String("wiki/sources/attention.md".to_string())
    );
    assert_eq!(
        detail.result.as_ref().unwrap()["cacheHit"],
        Value::Bool(false)
    );

    let summary = fs::read_to_string(project_root.join("wiki/sources/attention.md")).unwrap();
    assert!(summary.contains("Transformers use attention mechanisms."));

    let concept =
        fs::read_to_string(project_root.join("wiki/concepts/attention-mechanism.md")).unwrap();
    assert!(concept.contains("# Attention Mechanism"));
    assert!(concept.contains("sources: [\"attention.md\"]"));

    assert_eq!(mock.request_count(), 3);
}

#[tokio::test]
async fn ingest_source_sanitizes_dirty_provider_frontmatter_before_write() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("ingest-sanitize").await.unwrap();
    let mock = MockOpenAiServer::start(MockScenario::ingest_sanitize_success())
        .await
        .unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("ingest-sanitize-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;
    configure_provider(&state, &mock.base_url(), "mock-model").await;

    fs::write(
        project_root.join("raw/sources/attention.md"),
        "# Attention\n\nTransformers use attention mechanisms.\n",
    )
    .unwrap();

    let task_id = enqueue_ingest(&state, &cookie, &csrf, &project_id).await;
    wait_for_task_terminal(&state, &task_id).await;

    let summary = fs::read_to_string(project_root.join("wiki/sources/attention.md")).unwrap();
    assert!(summary.starts_with("---\n"));
    assert!(!summary.contains("```yaml"));
    assert!(!summary.contains("frontmatter:\n"));
    assert!(summary.contains("related: [\"[[attention-mechanism]]\", \"[[attention]]\"]"));
    assert!(summary.contains("# Attention"));
    assert_eq!(mock.request_count(), 3);
}

#[tokio::test]
async fn ingest_source_skips_provider_when_source_content_hash_is_unchanged() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("ingest-cache-hit").await.unwrap();
    let mock = MockOpenAiServer::start(MockScenario::ingest_success())
        .await
        .unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("ingest-cache-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;
    configure_provider(&state, &mock.base_url(), "mock-model").await;

    fs::write(
        project_root.join("raw/sources/attention.md"),
        "# Attention\n\nTransformers use attention mechanisms.\n",
    )
    .unwrap();

    let first_task_id = enqueue_ingest(&state, &cookie, &csrf, &project_id).await;
    wait_for_task_terminal(&state, &first_task_id).await;
    assert_eq!(mock.request_count(), 3);

    let second_task_id = enqueue_ingest(&state, &cookie, &csrf, &project_id).await;
    wait_for_task_terminal(&state, &second_task_id).await;
    assert_eq!(mock.request_count(), 3);

    let detail = store::get_task_by_id(&state, &second_task_id)
        .await
        .unwrap();
    assert_eq!(detail.status, "succeeded");
    assert_eq!(
        detail.result.as_ref().unwrap()["cacheHit"],
        Value::Bool(true)
    );
}

#[tokio::test]
async fn repeated_provider_ingest_merges_existing_page_body_and_frontmatter() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("ingest-page-merge").await.unwrap();
    let mock = MockOpenAiServer::start(MockScenario::ingest_with_page_merge_success())
        .await
        .unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("ingest-page-merge-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;
    configure_provider(&state, &mock.base_url(), "mock-model").await;

    fs::write(
        project_root.join("raw/sources/attention.md"),
        "# Attention\n\nTransformers use attention mechanisms.\n",
    )
    .unwrap();

    let first_task_id = enqueue_ingest(&state, &cookie, &csrf, &project_id).await;
    wait_for_task_terminal(&state, &first_task_id).await;

    fs::write(
    project_root.join("raw/sources/attention-optimizations.md"),
    "# Attention Optimizations\n\nOptimized attention implementations reduce memory overhead.\n",
  )
  .unwrap();

    let second_task_id = enqueue_custom_ingest(
        &state,
        &cookie,
        &csrf,
        &project_id,
        "raw/sources/attention-optimizations.md",
    )
    .await;
    wait_for_task_terminal(&state, &second_task_id).await;

    let concept =
        fs::read_to_string(project_root.join("wiki/concepts/attention-mechanism.md")).unwrap();
    assert!(concept.contains(
        "Attention focuses computation on relevant tokens and links back to [[attention]]."
    ));
    assert!(
        concept
            .contains("Efficient implementations reduce memory overhead and improve throughput.")
    );
    assert!(concept.contains("sources: [\"attention.md\", \"attention-optimizations.md\"]"));
    assert!(concept.contains(
        "tags: [\"transformers\", \"attention\", \"optimization\", \"flash-attention\"]"
    ));
    assert!(concept.contains("related: [attention, efficient-attention]"));
    assert_eq!(mock.request_count(), 7);
}

#[tokio::test]
async fn ingest_source_extracts_text_from_docx_sources() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("ingest-docx-source").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("ingest-docx-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    let import_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/sources:import"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({
            "fileName": "attention.docx",
            "contentBase64": base64::engine::general_purpose::STANDARD.encode(build_minimal_docx("Attention from DOCX."))
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(import_response.status(), StatusCode::ACCEPTED);
    let import_payload = read_json(import_response.into_body()).await;
    wait_for_task_terminal(
        &state,
        import_payload
            .get("taskId")
            .and_then(Value::as_str)
            .unwrap(),
    )
    .await;

    let ingest_response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/ingest"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "relativePath": "raw/sources/attention.docx" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(ingest_response.status(), StatusCode::ACCEPTED);
    let payload = read_json(ingest_response.into_body()).await;
    let task_id = payload
        .get("taskId")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();
    wait_for_task_terminal(&state, &task_id).await;

    let detail = store::get_task_by_id(&state, &task_id).await.unwrap();
    assert_eq!(detail.status, "succeeded", "{detail:?}");
    assert_eq!(
        detail.result.as_ref().unwrap()["summaryPath"],
        Value::String("wiki/sources/attention.md".to_string())
    );
    let summary = fs::read_to_string(project_root.join("wiki/sources/attention.md")).unwrap();
    assert!(summary.contains("Attention from DOCX."));
    assert!(summary.contains("sources: [\"attention.docx\"]"));
}

#[tokio::test]
async fn ingest_source_captions_embedded_docx_images_into_summary() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("ingest-docx-images").await.unwrap();
    let mock = MockOpenAiServer::start(MockScenario::ingest_success())
        .await
        .unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("ingest-docx-images-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;
    configure_provider(&state, &mock.base_url(), "mock-model").await;

    let docx_bytes = build_minimal_docx_with_image("Document with a figure.", &sample_png_bytes());
    let import_response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/sources:import"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "fileName": "vision.docx",
                      "contentBase64": base64::engine::general_purpose::STANDARD.encode(docx_bytes)
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(import_response.status(), StatusCode::ACCEPTED);
    let import_payload = read_json(import_response.into_body()).await;
    wait_for_task_terminal(
        &state,
        import_payload
            .get("taskId")
            .and_then(Value::as_str)
            .unwrap(),
    )
    .await;

    let ingest_response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/ingest"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "relativePath": "raw/sources/vision.docx" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ingest_response.status(), StatusCode::ACCEPTED);
    let payload = read_json(ingest_response.into_body()).await;
    let task_id = payload
        .get("taskId")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();
    wait_for_task_terminal(&state, &task_id).await;

    let summary = fs::read_to_string(project_root.join("wiki/sources/vision.md")).unwrap();
    assert!(summary.contains("Embedded Images"));
    assert!(summary.contains("A factual caption for an embedded diagram."));
    assert!(summary.contains("../media/vision/img-1.png"));
    assert!(project_root.join("wiki/media/vision/img-1.png").exists());
    assert!(mock.request_count() >= 3);
}

#[tokio::test]
async fn ingest_source_keeps_nested_source_identity_in_summary_paths_and_sources() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("ingest-nested-source-identity")
        .await
        .unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("ingest-nested-source-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    fs::create_dir_all(project_root.join("raw/sources/project-a")).unwrap();
    fs::create_dir_all(project_root.join("raw/sources/project-b")).unwrap();
    fs::write(
        project_root.join("raw/sources/project-a/config.yaml"),
        "name: alpha\nsetting: one\n",
    )
    .unwrap();
    fs::write(
        project_root.join("raw/sources/project-b/config.yaml"),
        "name: beta\nsetting: two\n",
    )
    .unwrap();

    let first_task_id = enqueue_custom_ingest(
        &state,
        &cookie,
        &csrf,
        &project_id,
        "raw/sources/project-a/config.yaml",
    )
    .await;
    wait_for_task_terminal(&state, &first_task_id).await;

    let second_task_id = enqueue_custom_ingest(
        &state,
        &cookie,
        &csrf,
        &project_id,
        "raw/sources/project-b/config.yaml",
    )
    .await;
    wait_for_task_terminal(&state, &second_task_id).await;

    let first_detail = store::get_task_by_id(&state, &first_task_id).await.unwrap();
    let second_detail = store::get_task_by_id(&state, &second_task_id)
        .await
        .unwrap();
    assert_ne!(
        first_detail.result.as_ref().unwrap()["summaryPath"],
        second_detail.result.as_ref().unwrap()["summaryPath"]
    );

    let first_summary_path = first_detail.result.as_ref().unwrap()["summaryPath"]
        .as_str()
        .unwrap()
        .to_string();
    let second_summary_path = second_detail.result.as_ref().unwrap()["summaryPath"]
        .as_str()
        .unwrap()
        .to_string();

    let first_summary = fs::read_to_string(project_root.join(&first_summary_path)).unwrap();
    let second_summary = fs::read_to_string(project_root.join(&second_summary_path)).unwrap();
    assert!(first_summary.contains("sources: [\"project-a/config.yaml\"]"));
    assert!(second_summary.contains("sources: [\"project-b/config.yaml\"]"));
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

async fn enqueue_ingest(
    state: &knowledge_server::app::state::AppState,
    cookie: &str,
    csrf: &str,
    project_id: &str,
) -> String {
    enqueue_custom_ingest(state, cookie, csrf, project_id, "raw/sources/attention.md").await
}

async fn enqueue_custom_ingest(
    state: &knowledge_server::app::state::AppState,
    cookie: &str,
    csrf: &str,
    project_id: &str,
    relative_path: &str,
) -> String {
    let ingest_response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/ingest"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, cookie)
                .header("x-csrf-token", csrf)
                .body(Body::from(
                    json!({ "relativePath": relative_path }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(ingest_response.status(), StatusCode::ACCEPTED);
    let payload = read_json(ingest_response.into_body()).await;
    payload
        .get("taskId")
        .and_then(Value::as_str)
        .unwrap()
        .to_string()
}

async fn configure_provider(
    state: &knowledge_server::app::state::AppState,
    base_url: &str,
    model: &str,
) {
    support::seed_provider_connection(&state.pool, base_url, "test-key", model, 30).await;
}

async fn wait_for_task_terminal(state: &knowledge_server::app::state::AppState, task_id: &str) {
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

fn build_minimal_docx(text: &str) -> Vec<u8> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(cursor);
    let options: FileOptions<'_, ()> = FileOptions::default();

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip
    .write_all(
      br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
    )
    .unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip
    .write_all(
      br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t>{text}</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#
        )
        .as_bytes(),
    )
    .unwrap();

    zip.finish().unwrap().into_inner()
}

fn build_minimal_docx_with_image(text: &str, png_bytes: &[u8]) -> Vec<u8> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(cursor);
    let options: FileOptions<'_, ()> = FileOptions::default();

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip
    .write_all(
      br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
    )
    .unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip
    .write_all(
      br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t>{text}</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#
        )
        .as_bytes(),
    )
    .unwrap();

    zip.start_file("word/media/image1.png", options).unwrap();
    zip.write_all(png_bytes).unwrap();

    zip.finish().unwrap().into_inner()
}

fn sample_png_bytes() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(128, 128, image::Rgba([255, 0, 0, 255]));
    let mut cursor = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, image::ImageFormat::Png)
        .unwrap();
    cursor.into_inner()
}
