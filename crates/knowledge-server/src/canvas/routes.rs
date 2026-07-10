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
    ProviderImageRequest, ProviderTextRequest,
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
        .route("/api/canvas-skill-jobs/{id}", get(get_skill_job_handler))
        .route("/api/canvas-skill-jobs/{id}/retry", post(retry_skill_job_handler))
        .route("/api/canvas/extract-url", post(extract_url_handler))
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillJobStatus {
    pub status: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
    pub progress: Option<serde_json::Value>,
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

async fn get_skill_job_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<SkillJobStatus>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    let job = crate::canvas::skill_jobs::get_job(&state.pool, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("unknown job"))?;
    // Only the creator may poll; never leak existence to others.
    if job.created_by != principal.user_id {
        return Err(ApiError::not_found("unknown job"));
    }
    Ok(Json(SkillJobStatus {
        status: job.status,
        result: job.result,
        error: job.error,
        progress: job.progress,
    }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillJobRetryResponse {
    pub job_id: String,
}

/// Requeue a failed skill render as a fresh job (same node, same input). A new
/// job id is issued instead of resetting the old row: the old id stays a stable
/// record of the failure, and the front-end poller dedupes terminal jobs by id,
/// so reusing it would never be polled again.
async fn retry_skill_job_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<SkillJobRetryResponse>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;
    let job = crate::canvas::skill_jobs::get_job(&state.pool, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("unknown job"))?;
    // Only the creator may retry; never leak existence to others.
    if job.created_by != principal.user_id {
        return Err(ApiError::not_found("unknown job"));
    }
    if job.status != "error" {
        return Err(ApiError::bad_request("only a failed job can be retried"));
    }
    let job_id = crate::canvas::skill_jobs::create_job(
        &state.pool,
        crate::canvas::skill_jobs::NewSkillJob {
            canvas_id: job.canvas_id,
            node_id: job.node_id,
            skill_id: job.skill_id,
            input: job.input,
            created_by: job.created_by,
        },
    )
    .await?;
    Ok(Json(SkillJobRetryResponse { job_id }))
}

async fn save_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SaveCanvasRequest>,
) -> Result<Json<CanvasResponse>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;
    let pruned = body.document.prune_invalid_edges();
    let document_text = serde_json::to_string(&pruned).unwrap_or_else(|_| "{}".to_string());
    let rec = store::update_canvas(
        &state.pool,
        &id,
        &principal.user_id,
        &body.title,
        &document_text,
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

pub fn build_search_done_payload(markdown: &str) -> serde_json::Value {
    json!({ "markdown": markdown })
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

/// Split a chat message into (command, argument) when it starts with `/`.
/// `/ppt swiss style` -> (Some("ppt"), "swiss style"); `hello` -> (None, "hello").
fn parse_slash_command(message: &str) -> (Option<&str>, &str) {
    let trimmed = message.trim_start();
    let Some(rest) = trimmed.strip_prefix('/') else {
        return (None, message.trim());
    };
    match rest.split_once(char::is_whitespace) {
        Some((cmd, arg)) => (Some(cmd), arg.trim()),
        None => (Some(rest.trim()), ""),
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

        let blocks =
            assemble_reference_blocks(&state, &principal, &doc, &node_id, &node_prompt).await?;
        let final_prompt = if blocks.is_empty() {
            node_prompt.clone()
        } else {
            format!("{node_prompt}\n\n参考:\n{}", blocks.join("\n\n"))
        };

        let user_id = principal.user_id.clone();
        let prompt = final_prompt;
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

    if node.r#type == "search" {
        let blocks =
            assemble_reference_blocks(&state, &principal, &doc, &node_id, &node_prompt).await?;
        let guidance = node.data.get("query").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();

        let query = if blocks.is_empty() {
            if guidance.is_empty() {
                return Err(ApiError::bad_request("search node has no query"));
            }
            guidance
        } else {
            let (system_prompt, user_prompt) =
                crate::canvas::service::build_search_query_prompt(&guidance, &blocks);
            let provider = load_active_connection(&state).await?.provider();
            let response = provider
                .complete_text(ProviderTextRequest { system_prompt, user_prompt })
                .await
                // A provider/transport failure is server-side, not a client error.
                .map_err(|e| ApiError::internal(e.message().to_string()))?;
            let synthesized = response.text.lines().next().unwrap_or("").trim().to_string();
            if synthesized.is_empty() {
                return Err(ApiError::bad_request("could not synthesize a search query"));
            }
            synthesized
        };

        let stream_state = state.clone();
        let event_stream: std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> =
            Box::pin(async_stream::stream! {
                match run_web_search_markdown(&stream_state, &query).await {
                    Ok(markdown) => {
                        yield Ok(
                            Event::default().event("done").data(
                                build_search_done_payload(&markdown).to_string(),
                            ),
                        );
                    }
                    Err(message) => yield Ok(sse_error(&message)),
                }
            });
        return Ok(Sse::new(event_stream).keep_alive(KeepAlive::default()));
    }

    // Ordered Note/URL/prior-analysis/search/image references + KB via RAG.
    let blocks =
        assemble_reference_blocks(&state, &principal, &doc, &node_id, &node_prompt).await?;

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

    let (command_opt, argument) = parse_slash_command(&message);
    let argument = argument.to_string();
    let descriptor = command_opt.and_then(|c| state.skill_registry.by_command(c).cloned());
    let user_id = principal.user_id.clone();
    let canvas_id = id.clone();
    let stream_state = state.clone();

    let event_stream = async_stream::stream! {
        match descriptor {
            // No recognized slash command → plain chat over selected-node context.
            None => {
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
                    vec![ProviderChatMessage { role: "user".to_string(), content: message }];

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
            Some(descriptor) => {
                // Any skill that requires a selection is blocked when none exists.
                if descriptor.requires_selection() && selected_ids.is_empty() {
                    yield Ok(sse_error("content empty"));
                    return;
                }
                // Only LlmSkill descriptors reach dispatch (e.g. `/ppt`): create an
                // async skill job and emit a running node the worker fills in later.
                let selection_text = plain_context.join("\n\n");
                let node_id = uuid::Uuid::new_v4().to_string();
                let input = json!({
                    "selection": selection_text,
                    "argument": argument,
                });
                let new_job = crate::canvas::skill_jobs::NewSkillJob {
                    canvas_id,
                    node_id,
                    skill_id: descriptor.id.clone(),
                    input,
                    created_by: user_id.clone(),
                };
                let job_id = match crate::canvas::skill_jobs::create_job(
                    &stream_state.pool,
                    new_job,
                )
                .await
                {
                    Ok(job_id) => job_id,
                    Err(error) => {
                        yield Ok(sse_error(&error.to_string()));
                        return;
                    }
                };
                let node = json!({
                    "type": "html",
                    "data": { "status": "running", "jobId": job_id }
                });
                yield Ok(
                    Event::default().event(skill_node_event_name()).data(
                        build_skill_node_done_payload(node, x, y).to_string(),
                    ),
                );
                yield Ok(Event::default().event("done").data("{}".to_string()));
            }
        }
    };

    Ok(Sse::new(event_stream).keep_alive(KeepAlive::default()))
}

/// Generate an image, persist it as an asset, and return an
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

// ---------------------------------------------------------------------------
// URL extraction: fetch a page and return readable markdown
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ExtractUrlRequest {
    url: String,
}

/// Run web search for `query` and format the hits as markdown. Used by the
/// search node SSE run path.
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

/// Assemble reference blocks for `node_id` in spatial (y, then x) order. Text
/// nodes format synchronously; KB nodes trigger RAG inline with a per-project
/// permission check so every source obeys the same ordering.
async fn assemble_reference_blocks(
    state: &AppState,
    principal: &Principal,
    doc: &CanvasDocument,
    node_id: &str,
    node_prompt: &str,
) -> Result<Vec<String>, ApiError> {
    let mut blocks = Vec::new();
    for src in doc.ordered_incoming_sources(node_id) {
        if src.r#type == "kb" {
            let Some(project_id) = src.data.get("projectId").and_then(|v| v.as_str()) else {
                continue;
            };
            let role = crate::tenancy::access::project_access_role(
                &state.pool,
                project_id,
                &principal.user_id,
            )
            .await
            .map_err(ApiError::from)?;
            match role {
                Some(_role) => {
                    let root =
                        crate::projects::service::project_root_for_id(state, project_id).await?;
                    let assembled = crate::chat::context::assemble_chat_context(
                        state,
                        project_id,
                        &root,
                        node_prompt,
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
        } else if let Some(block) = crate::canvas::service::format_text_reference_block(src) {
            blocks.push(block);
        }
    }
    Ok(blocks)
}

async fn extract_url_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ExtractUrlRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;

    let Some(config) = crate::web_fetch::config::load_fetch_config(&state).await? else {
        return Ok(Json(json!({
            "status": "error", "title": "", "markdown": "",
            "error": "web page fetch is not configured; set a fetch provider in Settings"
        })));
    };

    match crate::web_fetch::firecrawl::scrape(&config, &body.url).await {
        Ok(page) => Ok(Json(json!({
            "status": "ok", "title": page.title, "markdown": page.markdown, "error": null
        }))),
        Err(error) => Ok(Json(json!({
            "status": "error", "title": "", "markdown": "", "error": error.to_string()
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
    fn parse_slash_command_splits_command_and_arg() {
        assert_eq!(parse_slash_command("/ppt swiss"), (Some("ppt"), "swiss"));
        assert_eq!(parse_slash_command("/ppt"), (Some("ppt"), ""));
        assert_eq!(parse_slash_command("no command"), (None, "no command"));
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

    #[test]
    fn search_done_payload_carries_markdown() {
        let payload = build_search_done_payload("Search results for \"cats\":\n- a");
        assert_eq!(payload["markdown"], "Search results for \"cats\":\n- a");
    }
}
