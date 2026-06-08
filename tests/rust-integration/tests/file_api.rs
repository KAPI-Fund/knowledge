mod support;

use std::fs;
use std::io::Write;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use base64::Engine;
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{json, Value};
use support::TestEnvironment;
use tempfile::tempdir;
use tower::util::ServiceExt;
use zip::write::FileOptions;
use zip::ZipWriter;

#[tokio::test]
async fn files_api_lists_public_project_files_and_reads_text_content() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("files-api").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = temp.path().join("files-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  fs::write(
    project_root.join("wiki/concepts/attention.md"),
    "---\ntype: concept\ntitle: Attention\nsources: []\n---\n\n# Attention\n\nFocus tokens.\n",
  )
  .unwrap();
  fs::write(project_root.join("raw/sources/notes.md"), "# Notes\n\nSource content.\n").unwrap();
  fs::write(project_root.join(".knowledge/internal.txt"), "secret").unwrap();

  let list_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/files?root=all"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(list_response.status(), StatusCode::OK);
  let list_payload = read_json(list_response.into_body()).await;
  let files = list_payload.get("files").and_then(Value::as_array).unwrap();
  let listed_paths = collect_paths(files);
  assert!(listed_paths.iter().any(|path| path == "purpose.md"));
  assert!(listed_paths.iter().any(|path| path == "schema.md"));
  assert!(listed_paths.iter().any(|path| path == "wiki/concepts/attention.md"));
  assert!(listed_paths.iter().any(|path| path == "raw/sources/notes.md"));
  assert!(!listed_paths.iter().any(|path| path.starts_with(".knowledge/")));

  let content_response = build_app(state)
    .oneshot(
      Request::builder()
        .uri(format!(
          "/api/projects/{project_id}/files/content?path=wiki/concepts/attention.md"
        ))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(content_response.status(), StatusCode::OK);
  let content_payload = read_json(content_response.into_body()).await;
  assert_eq!(
    content_payload.get("path").and_then(Value::as_str),
    Some("wiki/concepts/attention.md"),
  );
  assert!(
    content_payload
      .get("content")
      .and_then(Value::as_str)
      .unwrap()
      .contains("# Attention"),
  );
}

#[tokio::test]
async fn files_api_extracts_text_preview_from_docx_sources() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("files-api-docx-preview").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = temp.path().join("files-docx-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  fs::write(
    project_root.join("raw/sources/attention.docx"),
    build_minimal_docx("Attention from file preview."),
  )
  .unwrap();

  let content_response = build_app(state)
    .oneshot(
      Request::builder()
        .uri(format!(
          "/api/projects/{project_id}/files/content?path=raw/sources/attention.docx"
        ))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(content_response.status(), StatusCode::OK);
  let content_payload = read_json(content_response.into_body()).await;
  assert_eq!(
    content_payload.get("path").and_then(Value::as_str),
    Some("raw/sources/attention.docx"),
  );
  assert!(
    content_payload
      .get("content")
      .and_then(Value::as_str)
      .unwrap()
      .contains("Attention from file preview."),
  );
}

#[tokio::test]
async fn files_api_rejects_invalid_roots_and_non_text_previews() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("files-api-errors").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = temp.path().join("files-error-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;

  let invalid_root_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/files?root=private"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(invalid_root_response.status(), StatusCode::BAD_REQUEST);

  let binary_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/files/content?path=wiki/media/logo.png"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(binary_response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

  let traversal_response = build_app(state)
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/files/content?path=../secret.md"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(traversal_response.status(), StatusCode::FORBIDDEN);
}

fn collect_paths(nodes: &[Value]) -> Vec<String> {
  let mut paths = Vec::new();
  for node in nodes {
    if let Some(path) = node.get("path").and_then(Value::as_str) {
      paths.push(path.to_string());
    }
    if let Some(children) = node.get("children").and_then(Value::as_array) {
      paths.extend(collect_paths(children));
    }
  }
  paths
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
  let response = build_app(state)
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/projects")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, cookie)
        .header("x-csrf-token", csrf)
        .body(Body::from(
          json!({
            "name": project_root.file_name().unwrap().to_string_lossy(),
            "rootPath": project_root.to_string_lossy()
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  let payload = read_json(response.into_body()).await;
  payload.get("id").and_then(Value::as_str).unwrap().to_string()
}

async fn read_json(body: Body) -> Value {
  let bytes = to_bytes(body, usize::MAX).await.unwrap();
  serde_json::from_slice(&bytes).unwrap()
}

fn build_minimal_docx(text: &str) -> Vec<u8> {
  let _ = base64::engine::general_purpose::STANDARD.encode(text);
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
  zip
    .write_all(
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
