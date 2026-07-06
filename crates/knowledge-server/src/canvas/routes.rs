use std::convert::Infallible;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::app::state::AppState;
use crate::auth::principal::{resolve_principal, Principal};
use crate::canvas::document::CanvasDocument;
use crate::canvas::store;
use crate::http::error::ApiError;
use crate::providers::{
    load_active_connection, load_image_config, ProviderChatMessage, ProviderChatStreamRequest,
    ProviderImageRequest,
};
use crate::query::load_query_settings;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/canvases", get(list_handler).post(create_handler))
        .route(
            "/api/canvases/{id}",
            get(get_handler).put(save_handler).delete(delete_handler),
        )
        .route("/api/canvases/{id}/nodes/{node_id}/run", post(run_node_handler))
        .route("/api/canvases/{id}/chat", post(chat_handler))
        .route("/api/canvas/extract-url", post(extract_url_handler))
        .route("/api/canvas/search", post(search_handler))
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateCanvasRequest {
    #[serde(default)]
    pub title: Option<String>,
}

impl CreateCanvasRequest {
    pub fn resolved_title(&self) -> String {
        match &self.title {
            Some(t) => {
                let trimmed = t.trim();
                if trimmed.is_empty() {
                    "Untitled canvas".to_string()
                } else {
                    trimmed.to_string()
                }
            }
            None => "Untitled canvas".to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SaveCanvasRequest {
    pub title: String,
    pub document: CanvasDocument,
}

impl SaveCanvasRequest {
    pub fn document_text(&self) -> String {
        serde_json::to_string(&self.document).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Debug, Serialize)]
pub struct CanvasResponse {
    pub id: String,
    pub title: String,
    pub document: CanvasDocument,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// CSRF helper
// ---------------------------------------------------------------------------

fn require_csrf(principal: &Principal, headers: &HeaderMap) -> Result<(), ApiError> {
    if principal.requires_csrf() {
        let expected = principal
            .csrf_token
            .as_deref()
            .ok_or_else(|| ApiError::unauthorized("missing csrf token"))?;
        let supplied = headers
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if supplied.is_empty() || supplied != expected {
            return Err(ApiError::unauthorized("invalid csrf token"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn list_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<store::CanvasSummary>>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    let records = store::list_canvases(&state.pool, &principal.user_id).await?;
    let summaries: Vec<store::CanvasSummary> = records.iter().map(store::CanvasSummary::from).collect();
    Ok(Json(summaries))
}

async fn create_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateCanvasRequest>,
) -> Result<Json<CanvasResponse>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;
    let rec = store::create_canvas(&state.pool, &principal.user_id, &body.resolved_title()).await?;
    let document = rec.parse_document().map_err(|_| ApiError::internal("corrupt document"))?;
    Ok(Json(CanvasResponse {
        id: rec.id,
        title: rec.title,
        document,
        created_at: rec.created_at,
        updated_at: rec.updated_at,
    }))
}

async fn get_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<CanvasResponse>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    let rec = store::get_canvas(&state.pool, &id, &principal.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("canvas not found"))?;
    let document = rec.parse_document().map_err(|_| ApiError::internal("corrupt document"))?;
    Ok(Json(CanvasResponse {
        id: rec.id,
        title: rec.title,
        document,
        created_at: rec.created_at,
        updated_at: rec.updated_at,
    }))
}

async fn save_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SaveCanvasRequest>,
) -> Result<Json<CanvasResponse>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;
    let rec = store::update_canvas(
        &state.pool,
        &id,
        &principal.user_id,
        &body.title,
        &body.document_text(),
    )
    .await?
    .ok_or_else(|| ApiError::not_found("canvas not found"))?;
    let document = rec.parse_document().map_err(|_| ApiError::internal("corrupt document"))?;
    Ok(Json(CanvasResponse {
        id: rec.id,
        title: rec.title,
        document,
        created_at: rec.created_at,
        updated_at: rec.updated_at,
    }))
}

async fn delete_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;
    let deleted = store::delete_canvas(&state.pool, &id, &principal.user_id).await?;
    if !deleted {
        return Err(ApiError::not_found("canvas not found"));
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}

// ---------------------------------------------------------------------------
// SSE done-payload builders (pure) + shared helpers
// ---------------------------------------------------------------------------

pub fn build_analyze_done_payload(
    version_id: &str,
    content: &str,
    created_at: &str,
) -> serde_json::Value {
    json!({ "versionId": version_id, "content": content, "createdAt": created_at })
}

pub fn build_image_done_payload(version_id: &str, url: &str, created_at: &str) -> serde_json::Value {
    json!({ "versionId": version_id, "url": url, "createdAt": created_at })
}

pub fn build_skill_node_done_payload(node: serde_json::Value, x: f64, y: f64) -> serde_json::Value {
    json!({ "node": node, "x": x, "y": y })
}

/// The SSE event name that carries a freshly-built skill node to the client.
pub fn skill_node_event_name() -> &'static str {
    "node"
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default()
}

/// A canvas-chat message is either a slash-command skill or a plain prompt.
#[derive(Debug, PartialEq, Eq)]
enum ChatCommand {
    Search(String),
    Image(String),
    Analyze(String),
    Plain(String),
}

impl ChatCommand {
    fn parse(message: &str) -> ChatCommand {
        let trimmed = message.trim();
        if let Some(rest) = trimmed.strip_prefix("/search") {
            ChatCommand::Search(rest.trim().to_string())
        } else if let Some(rest) = trimmed.strip_prefix("/image") {
            ChatCommand::Image(rest.trim().to_string())
        } else if let Some(rest) = trimmed.strip_prefix("/analyze") {
            ChatCommand::Analyze(rest.trim().to_string())
        } else {
            ChatCommand::Plain(trimmed.to_string())
        }
    }
}

fn sse_error(message: &str) -> Event {
    Event::default().event("error").data(json!({ "message": message }).to_string())
}

// ---------------------------------------------------------------------------
// Node-run SSE: analyze a node using its incoming references (+ KB via RAG)
// ---------------------------------------------------------------------------

async fn run_node_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, node_id)): Path<(String, String)>,
) -> Result<Sse<axum::response::sse::KeepAliveStream<std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>>>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;

    let rec = store::get_canvas(&state.pool, &id, &principal.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("canvas not found"))?;
    let doc = rec.parse_document().map_err(|_| ApiError::internal("corrupt document"))?;
    let node =
        doc.node(&node_id).cloned().ok_or_else(|| ApiError::not_found("node not found"))?;

    let node_prompt =
        node.data.get("prompt").and_then(|v| v.as_str()).unwrap_or("").to_string();

    if node.r#type == "ai_image" {
        if node_prompt.trim().is_empty() {
            return Err(ApiError::bad_request("image node has no prompt"));
        }
        load_image_config(&state).await?; // validate image config before streaming
        let user_id = principal.user_id.clone();
        let prompt = node_prompt.clone();
        let stream_state = state.clone();
        let event_stream: std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> =
            Box::pin(async_stream::stream! {
                match run_image_skill(&stream_state, &user_id, &prompt).await {
                    Ok(node_json) => {
                        let url = node_json
                            .get("data")
                            .and_then(|d| d.get("url"))
                            .and_then(|u| u.as_str())
                            .unwrap_or("")
                            .to_string();
                        let version_id = uuid::Uuid::new_v4().to_string();
                        let created_at = now_rfc3339();
                        yield Ok(
                            Event::default().event("done").data(
                                build_image_done_payload(&version_id, &url, &created_at)
                                    .to_string(),
                            ),
                        );
                    }
                    Err(message) => yield Ok(sse_error(&message)),
                }
            });
        return Ok(Sse::new(event_stream).keep_alive(KeepAlive::default()));
    }

    // Note/URL/prior-analysis references.
    let mut blocks = crate::canvas::service::collect_reference_blocks(&doc, &node_id);

    // KB nodes -> RAG, with a per-project permission check; excluded if no access.
    for project_id in crate::canvas::service::referenced_kb_project_ids(&doc, &node_id) {
        let role = crate::tenancy::access::project_access_role(
            &state.pool,
            &project_id,
            &principal.user_id,
        )
        .await
        .map_err(ApiError::from)?;
        match role {
            Some(_role) => {
                let root =
                    crate::projects::service::project_root_for_id(&state, &project_id).await?;
                let assembled = crate::chat::context::assemble_chat_context(
                    &state,
                    &project_id,
                    &root,
                    &node_prompt,
                    8,
                )
                .await?;
                for b in assembled.context_blocks {
                    blocks.push(format!("Knowledge base ({project_id}):\n{b}"));
                }
            }
            None => {
                blocks.push(format!("[Knowledge base {project_id}: no access, excluded]"));
            }
        }
    }

    let prompt = crate::canvas::service::build_analyze_prompt(&node_prompt, &blocks);

    let settings = load_query_settings(&state).await?;
    let provider = load_active_connection(&state).await?.provider();
    let system_prompt = format!(
        "You are an analysis assistant reasoning over a knowledge canvas. Respond in {}. Use only the provided sources; if they are insufficient, say so plainly.",
        settings.language
    );
    let messages = vec![ProviderChatMessage { role: "user".to_string(), content: prompt }];

    let event_stream: std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> =
        Box::pin(async_stream::stream! {
            let mut full_text = String::new();
            match provider
                .stream_chat(ProviderChatStreamRequest { system_prompt, messages })
                .await
            {
                Ok(mut deltas) => {
                    while let Some(delta) = deltas.next().await {
                        match delta {
                            Ok(text) => {
                                full_text.push_str(&text);
                                yield Ok(
                                    Event::default()
                                        .event("delta")
                                        .data(json!({ "text": text }).to_string()),
                                );
                            }
                            Err(error) => {
                                yield Ok(sse_error(error.message()));
                                return;
                            }
                        }
                    }
                }
                Err(error) => {
                    yield Ok(sse_error(error.message()));
                    return;
                }
            }

            let version_id = uuid::Uuid::new_v4().to_string();
            let created_at = now_rfc3339();
            yield Ok(
                Event::default().event("done").data(
                    build_analyze_done_payload(&version_id, &full_text, &created_at).to_string(),
                ),
            );
        });

    Ok(Sse::new(event_stream).keep_alive(KeepAlive::default()))
}

// ---------------------------------------------------------------------------
// Canvas chat SSE: plain answer over selected nodes, or a slash-command skill
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CanvasChatRequest {
    message: String,
    #[serde(default)]
    selected_node_ids: Vec<String>,
    #[serde(default)]
    x: Option<f64>,
    #[serde(default)]
    y: Option<f64>,
}

async fn chat_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<CanvasChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;

    let rec = store::get_canvas(&state.pool, &id, &principal.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("canvas not found"))?;
    let doc = rec.parse_document().map_err(|_| ApiError::internal("corrupt document"))?;

    let message = body.message.trim().to_string();
    if message.is_empty() {
        return Err(ApiError::bad_request("message must not be empty"));
    }
    let x = body.x.unwrap_or(0.0);
    let y = body.y.unwrap_or(0.0);
    let selected_ids = body.selected_node_ids.clone();

    // Text of explicitly selected nodes, used as context for a plain answer.
    let mut plain_context: Vec<String> = Vec::new();
    for sid in &selected_ids {
        if let Some(n) = doc.node(sid) {
            let block = match n.r#type.as_str() {
                "note" => n
                    .data
                    .get("markdown")
                    .and_then(|v| v.as_str())
                    .map(|m| format!("Note:\n{m}")),
                "url" => {
                    let title = n.data.get("title").and_then(|v| v.as_str()).unwrap_or("");
                    n.data
                        .get("markdown")
                        .and_then(|v| v.as_str())
                        .map(|m| format!("Web page: {title}\n{m}"))
                }
                "ai_analyze" => crate::canvas::document::active_version_content(&n.data)
                    .map(|c| format!("Prior analysis:\n{c}")),
                _ => None,
            };
            if let Some(b) = block
                && !b.trim().is_empty()
            {
                plain_context.push(b);
            }
        }
    }

    let command = ChatCommand::parse(&message);
    let user_id = principal.user_id.clone();
    let stream_state = state.clone();

    let event_stream = async_stream::stream! {
        match command {
            ChatCommand::Search(query) => {
                if query.is_empty() {
                    yield Ok(sse_error("search query must not be empty"));
                    return;
                }
                match run_search_skill(&stream_state, &query).await {
                    Ok(node) => {
                        yield Ok(
                            Event::default().event(skill_node_event_name()).data(
                                build_skill_node_done_payload(node, x, y).to_string(),
                            ),
                        );
                        yield Ok(Event::default().event("done").data("{}".to_string()));
                    }
                    Err(message) => yield Ok(sse_error(&message)),
                }
            }
            ChatCommand::Image(prompt) => {
                if prompt.is_empty() {
                    yield Ok(sse_error("image prompt must not be empty"));
                    return;
                }
                match run_image_skill(&stream_state, &user_id, &prompt).await {
                    Ok(node) => {
                        yield Ok(
                            Event::default().event(skill_node_event_name()).data(
                                build_skill_node_done_payload(node, x, y).to_string(),
                            ),
                        );
                        yield Ok(Event::default().event("done").data("{}".to_string()));
                    }
                    Err(message) => yield Ok(sse_error(&message)),
                }
            }
            ChatCommand::Analyze(prompt) => {
                let node = build_analyze_node(&prompt, &selected_ids);
                yield Ok(
                    Event::default()
                        .event(skill_node_event_name())
                        .data(build_skill_node_done_payload(node, x, y).to_string()),
                );
                yield Ok(Event::default().event("done").data("{}".to_string()));
            }
            ChatCommand::Plain(text) => {
                let settings = match load_query_settings(&stream_state).await {
                    Ok(settings) => settings,
                    Err(error) => {
                        yield Ok(sse_error(&error.to_string()));
                        return;
                    }
                };
                let provider = match load_active_connection(&stream_state).await {
                    Ok(connection) => connection.provider(),
                    Err(error) => {
                        yield Ok(sse_error(&error.to_string()));
                        return;
                    }
                };
                let system_prompt = format!(
                    "You are a helpful assistant reasoning over a knowledge canvas. Respond in {}.{}",
                    settings.language,
                    if plain_context.is_empty() {
                        String::new()
                    } else {
                        format!("\n\nSelected context:\n{}", plain_context.join("\n\n"))
                    }
                );
                let messages =
                    vec![ProviderChatMessage { role: "user".to_string(), content: text }];

                let mut full_text = String::new();
                match provider
                    .stream_chat(ProviderChatStreamRequest { system_prompt, messages })
                    .await
                {
                    Ok(mut deltas) => {
                        while let Some(delta) = deltas.next().await {
                            match delta {
                                Ok(chunk) => {
                                    full_text.push_str(&chunk);
                                    yield Ok(
                                        Event::default()
                                            .event("delta")
                                            .data(json!({ "text": chunk }).to_string()),
                                    );
                                }
                                Err(error) => {
                                    yield Ok(sse_error(error.message()));
                                    return;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        yield Ok(sse_error(error.message()));
                        return;
                    }
                }

                yield Ok(
                    Event::default()
                        .event("done")
                        .data(json!({ "content": full_text }).to_string()),
                );
            }
        }
    };

    Ok(Sse::new(event_stream).keep_alive(KeepAlive::default()))
}

/// `/search` skill: run web search and return a `note` node carrying the
/// results as markdown (no URL of its own).
async fn run_search_skill(state: &AppState, query: &str) -> Result<serde_json::Value, String> {
    let markdown = run_web_search_markdown(state, query).await?;
    Ok(json!({
        "type": "note",
        "data": { "title": format!("Search: {query}"), "markdown": markdown }
    }))
}

/// `/image` skill: generate an image, persist it as an asset, and return an
/// `ai_image` node referencing the asset by id + url.
async fn run_image_skill(
    state: &AppState,
    user_id: &str,
    prompt: &str,
) -> Result<serde_json::Value, String> {
    let config = load_image_config(state).await.map_err(|error| error.to_string())?;
    let result = config
        .provider()
        .generate_image(ProviderImageRequest { prompt: prompt.to_string(), size: config.size.clone() })
        .await
        .map_err(|error| error.message().to_string())?;
    let asset = crate::assets::store::NewAsset::new(user_id, &result.mime, result.bytes);
    let asset_id = crate::assets::store::insert_asset(&state.pool, &asset)
        .await
        .map_err(|error| error.to_string())?;
    let url = crate::assets::store::asset_url(&asset_id);
    let version_id = uuid::Uuid::new_v4().to_string();
    let created_at = now_rfc3339();
    Ok(build_image_node(&asset_id, &url, prompt, &version_id, &created_at))
}

/// Build an `ai_image` node whose first version is already populated so the
/// client renders the image immediately. `data.url` is kept alongside the
/// version so the node-run path can extract it.
fn build_image_node(
    asset_id: &str,
    url: &str,
    prompt: &str,
    version_id: &str,
    created_at: &str,
) -> serde_json::Value {
    json!({
        "type": "ai_image",
        "data": {
            "assetId": asset_id,
            "url": url,
            "prompt": prompt,
            "versions": [{ "id": version_id, "url": url, "createdAt": created_at }],
            "activeVersionId": version_id,
            "status": "idle",
            "error": null
        }
    })
}

/// `/analyze` skill: build an idle `ai_analyze` node referencing the selected
/// nodes; the client wires the reference edges.
fn build_analyze_node(prompt: &str, selected_ids: &[String]) -> serde_json::Value {
    json!({
        "type": "ai_analyze",
        "data": {
            "prompt": prompt,
            "versions": [],
            "activeVersionId": null,
            "status": "idle",
            "error": null,
            "sourceNodeIds": selected_ids
        }
    })
}

// ---------------------------------------------------------------------------
// URL extraction: fetch a page and return readable markdown
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ExtractUrlRequest {
    url: String,
}

// ---------------------------------------------------------------------------
// Web search: run a query and return results as markdown (in-place node fill)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SearchRequest {
    query: String,
}

async fn search_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<SearchRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;

    let query = body.query.trim();
    if query.is_empty() {
        return Err(ApiError::bad_request("search query must not be empty"));
    }

    match run_web_search_markdown(&state, query).await {
        Ok(markdown) => Ok(Json(json!({
            "status": "ok", "query": query, "markdown": markdown, "error": null
        }))),
        Err(error) => Ok(Json(json!({
            "status": "error", "query": query, "markdown": "", "error": error
        }))),
    }
}

/// Run web search for `query` and format the hits as markdown. Shared by the
/// REST search endpoint and the `/search` chat skill.
async fn run_web_search_markdown(state: &AppState, query: &str) -> Result<String, String> {
    let config = crate::web_search::config::load_web_search_config(state)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "web search is not configured".to_string())?;
    let hits = crate::web_search::provider::web_search(&config, query, 5)
        .await
        .map_err(|error| error.to_string())?;
    let entries: Vec<crate::canvas::service::SearchResultEntry> = hits
        .into_iter()
        .map(|hit| crate::canvas::service::SearchResultEntry {
            title: hit.title,
            url: hit.url,
            snippet: hit.snippet,
        })
        .collect();
    Ok(crate::canvas::service::search_results_to_markdown(query, &entries))
}

async fn extract_url_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ExtractUrlRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;

    let client = crate::canvas::service::build_extractor_client(state.allow_private_fetch)
        .map_err(|error| ApiError::internal(format!("http client init failed: {error}")))?;

    match crate::canvas::service::fetch_url(&client, &body.url, state.allow_private_fetch).await {
        Ok(page) => Ok(Json(json!({
            "status": "ok", "title": page.title, "markdown": page.markdown, "error": null
        }))),
        Err(error) => Ok(Json(json!({
            "status": "error", "title": "", "markdown": "", "error": error
        }))),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_request_defaults_title_when_absent() {
        let req: CreateCanvasRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(req.resolved_title(), "Untitled canvas");
    }

    #[test]
    fn create_request_uses_provided_title() {
        let req: CreateCanvasRequest =
            serde_json::from_str(r#"{"title":"Research B"}"#).unwrap();
        assert_eq!(req.resolved_title(), "Research B");
    }

    #[test]
    fn save_request_serializes_document_to_text() {
        let req: SaveCanvasRequest = serde_json::from_str(
            r#"{"title":"T","document":{"nodes":[],"edges":[],"viewport":{"x":0,"y":0,"zoom":1}}}"#,
        )
        .unwrap();
        let text = req.document_text();
        assert!(text.contains("\"viewport\""));
        let _doc: crate::canvas::document::CanvasDocument =
            serde_json::from_str(&text).unwrap();
    }

    #[test]
    fn analyze_done_payload_carries_version_fields() {
        let payload = build_analyze_done_payload("v-new", "the answer", "2026-06-30T00:00:00Z");
        assert_eq!(payload["versionId"], "v-new");
        assert_eq!(payload["content"], "the answer");
        assert_eq!(payload["createdAt"], "2026-06-30T00:00:00Z");
    }

    #[test]
    fn skill_node_done_payload_describes_new_node() {
        let node = serde_json::json!({ "type": "url", "data": { "title": "T" } });
        let payload = build_skill_node_done_payload(node, 120.0, 240.0);
        assert_eq!(payload["node"]["type"], "url");
        assert_eq!(payload["x"], 120.0);
        assert_eq!(payload["y"], 240.0);
    }

    #[test]
    fn chat_command_parse_recognizes_slash_skills() {
        assert_eq!(ChatCommand::parse("/search cats"), ChatCommand::Search("cats".to_string()));
        assert_eq!(ChatCommand::parse("/image a fox"), ChatCommand::Image("a fox".to_string()));
        assert_eq!(ChatCommand::parse("/analyze"), ChatCommand::Analyze(String::new()));
        assert_eq!(
            ChatCommand::parse("just chatting"),
            ChatCommand::Plain("just chatting".to_string())
        );
    }

    #[test]
    fn build_analyze_node_references_selected_ids() {
        let node = build_analyze_node("summarize", &["a".to_string(), "b".to_string()]);
        assert_eq!(node["type"], "ai_analyze");
        assert_eq!(node["data"]["prompt"], "summarize");
        assert_eq!(node["data"]["sourceNodeIds"][0], "a");
        assert_eq!(node["data"]["status"], "idle");
    }

    #[test]
    fn search_request_deserializes_query() {
        let req: SearchRequest = serde_json::from_str(r#"{"query":"cats"}"#).unwrap();
        assert_eq!(req.query, "cats");
    }

    #[test]
    fn chat_request_deserializes_camel_case_selected_ids() {
        let req: CanvasChatRequest = serde_json::from_str(
            r#"{"message":"hi","selectedNodeIds":["a","b"],"x":10.0,"y":20.0}"#,
        )
        .unwrap();
        assert_eq!(req.message, "hi");
        assert_eq!(req.selected_node_ids, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(req.x, Some(10.0));
        assert_eq!(req.y, Some(20.0));
    }

    #[test]
    fn skill_node_uses_node_event_not_done() {
        assert_eq!(skill_node_event_name(), "node");
    }

    #[test]
    fn image_done_payload_carries_url_and_version_fields() {
        let payload = build_image_done_payload("v-img", "/api/assets/abc", "2026-07-01T00:00:00Z");
        assert_eq!(payload["versionId"], "v-img");
        assert_eq!(payload["url"], "/api/assets/abc");
        assert_eq!(payload["createdAt"], "2026-07-01T00:00:00Z");
    }

    #[test]
    fn image_node_seeds_active_version() {
        let node = build_image_node(
            "asset-1",
            "/api/assets/asset-1",
            "a fox",
            "v-1",
            "2026-07-01T00:00:00Z",
        );
        assert_eq!(node["type"], "ai_image");
        assert_eq!(node["data"]["assetId"], "asset-1");
        assert_eq!(node["data"]["url"], "/api/assets/asset-1");
        assert_eq!(node["data"]["prompt"], "a fox");
        assert_eq!(node["data"]["activeVersionId"], "v-1");
        assert_eq!(node["data"]["status"], "idle");
        let versions = node["data"]["versions"].as_array().unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0]["id"], "v-1");
        assert_eq!(versions[0]["url"], "/api/assets/asset-1");
        assert_eq!(versions[0]["createdAt"], "2026-07-01T00:00:00Z");
    }
}
