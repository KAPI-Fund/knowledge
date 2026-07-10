use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, patch};
use axum::{Json, Router};
use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::agent::context::AgentConversationMessage;
use crate::agent::events::AgentEvent;
use crate::agent::permissions::PermissionPolicy;
use crate::agent::runtime::{AgentEventSink, AgentLoopRequest, run_agent_loop};
use crate::agent::skills::{list_available_skills, load_skills};
use crate::agent::types::{AgentMessageOptions, AgentSkillMode};
use crate::app::state::AppState;
use crate::chat::context::assemble_chat_context;
use crate::chat::store::{
    ConversationRecord, MessageRecord, append_agent_message, append_message, create_conversation,
    delete_conversation, find_conversation, list_conversations, list_messages,
    rename_conversation,
};
use crate::http::error::ApiError;
use crate::projects::routes::{authorized_principal, validate_csrf};
use crate::projects::service::project_root_for_id;
use crate::providers::{load_active_connection, ProviderChatMessage, ProviderChatStreamRequest};
use crate::query::load_query_settings;
use crate::tenancy::access::project_access_role;

type ChatEventStream = BoxStream<'static, Result<Event, Infallible>>;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/projects/{project_id}/conversations",
            get(list_conversations_handler).post(create_conversation_handler),
        )
        .route(
            "/api/projects/{project_id}/conversations/{conversation_id}",
            patch(rename_conversation_handler).delete(delete_conversation_handler),
        )
        .route(
            "/api/projects/{project_id}/conversations/{conversation_id}/messages",
            get(list_messages_handler).post(send_message_handler),
        )
}

const MAX_HISTORY_MESSAGES: usize = 10;

#[derive(Debug, Deserialize)]
struct CreateConversationRequest {
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RenameConversationRequest {
    title: String,
}

#[derive(Debug, Deserialize)]
struct SendMessageRequest {
    content: String,
    // `None` keeps the plain RAG chat path; `Some` routes through the agent loop.
    #[serde(default)]
    agent: Option<AgentMessageOptions>,
}

async fn list_conversations_handler(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let session = authorized_principal(&state, &headers, Some(&project_id)).await?;
    let conversations = list_conversations(&state.pool, &project_id, &session.user_id).await?;
    Ok(Json(json!({
        "conversations": conversations.iter().map(conversation_json).collect::<Vec<_>>()
    })))
}

async fn create_conversation_handler(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<CreateConversationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session = authorized_principal(&state, &headers, Some(&project_id)).await?;
    validate_csrf(&headers, &session)?;
    let title = payload
        .title
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "New conversation".to_string());
    let conversation =
        create_conversation(&state.pool, &project_id, &session.user_id, title.trim()).await?;
    Ok((StatusCode::CREATED, Json(conversation_json(&conversation))))
}

async fn rename_conversation_handler(
    State(state): State<AppState>,
    Path((project_id, conversation_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(payload): Json<RenameConversationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session = authorized_principal(&state, &headers, Some(&project_id)).await?;
    validate_csrf(&headers, &session)?;
    if payload.title.trim().is_empty() {
        return Err(ApiError::bad_request("title must not be empty"));
    }
    let conversation = rename_conversation(
        &state.pool,
        &project_id,
        &conversation_id,
        &session.user_id,
        payload.title.trim(),
    )
    .await?
    .ok_or_else(|| ApiError::not_found("conversation not found"))?;
    Ok(Json(conversation_json(&conversation)))
}

async fn delete_conversation_handler(
    State(state): State<AppState>,
    Path((project_id, conversation_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let session = authorized_principal(&state, &headers, Some(&project_id)).await?;
    validate_csrf(&headers, &session)?;
    let deleted =
        delete_conversation(&state.pool, &project_id, &conversation_id, &session.user_id).await?;
    if !deleted {
        return Err(ApiError::not_found("conversation not found"));
    }
    Ok(Json(json!({ "deleted": true })))
}

async fn list_messages_handler(
    State(state): State<AppState>,
    Path((project_id, conversation_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let session = authorized_principal(&state, &headers, Some(&project_id)).await?;
    find_conversation(&state.pool, &project_id, &conversation_id, &session.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("conversation not found"))?;
    let messages = list_messages(&state.pool, &conversation_id).await?;
    Ok(Json(json!({
        "messages": messages.iter().map(message_json).collect::<Vec<_>>()
    })))
}

async fn send_message_handler(
    State(state): State<AppState>,
    Path((project_id, conversation_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(payload): Json<SendMessageRequest>,
) -> Result<axum::response::Response, ApiError> {
    let session = authorized_principal(&state, &headers, Some(&project_id)).await?;
    validate_csrf(&headers, &session)?;
    let content = payload.content.trim().to_string();
    find_conversation(&state.pool, &project_id, &conversation_id, &session.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("conversation not found"))?;

    if let Some(options) = payload.agent {
        return send_agent_message(
            state,
            project_id,
            conversation_id,
            session.user_id.clone(),
            content,
            options,
        )
        .await;
    }

    if content.is_empty() {
        return Err(ApiError::bad_request("message content must not be empty"));
    }

    let settings = load_query_settings(&state).await?;
    let connection = load_active_connection(&state).await?;

    let root = project_root_for_id(&state, &project_id).await?;
    let history = list_messages(&state.pool, &conversation_id).await?;
    append_message(&state.pool, &conversation_id, "user", &content, None).await?;

    let top_k = settings.default_query_limit.max(1) as usize;
    let assembled = assemble_chat_context(&state, &project_id, &root, &content, top_k).await?;

    let system_prompt = format!(
        "You answer questions using only the provided wiki context. Respond in {}. If the context is insufficient, say so plainly.\n\nContext:\n{}",
        settings.language,
        if assembled.context_blocks.is_empty() {
            "No relevant wiki context was retrieved.".to_string()
        } else {
            assembled.context_blocks.join("\n\n")
        }
    );

    let mut messages: Vec<ProviderChatMessage> = history
        .iter()
        .rev()
        .take(MAX_HISTORY_MESSAGES)
        .rev()
        .map(|message| ProviderChatMessage {
            role: message.role.clone(),
            content: message.content.clone(),
        })
        .collect();
    messages.push(ProviderChatMessage {
        role: "user".to_string(),
        content,
    });

    let provider = connection.provider();

    let context_summary = assembled.context_summary;
    let stream_state = state.clone();

    let event_stream = async_stream::stream! {
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
                            yield Ok(
                                Event::default()
                                    .event("error")
                                    .data(json!({ "message": error.message() }).to_string()),
                            );
                            return;
                        }
                    }
                }
            }
            Err(error) => {
                yield Ok(
                    Event::default()
                        .event("error")
                        .data(json!({ "message": error.message() }).to_string()),
                );
                return;
            }
        }

        match append_message(
            &stream_state.pool,
            &conversation_id,
            "assistant",
            &full_text,
            context_summary.as_deref(),
        )
        .await
        {
            Ok(message) => {
                yield Ok(
                    Event::default().event("done").data(
                        json!({
                            "messageId": message.id,
                            "content": full_text,
                            "contextSummary": context_summary
                        })
                        .to_string(),
                    ),
                );
            }
            Err(_) => {
                yield Ok(
                    Event::default()
                        .event("error")
                        .data(json!({ "message": "failed to persist assistant message" }).to_string()),
                );
            }
        }
    };

    let event_stream: ChatEventStream = event_stream.boxed();
    Ok(Sse::new(event_stream).keep_alive(KeepAlive::default()).into_response())
}

/// Agent chat turn. The loop runs in a spawned task; redacted events are
/// forwarded as `agentEvent` SSE frames while the request stream stays open.
/// A `user.ask` pause ends the run without persisting an assistant message —
/// the frontend resumes by re-POSTing with `resumeRequestId` + `formResult`.
async fn send_agent_message(
    state: AppState,
    project_id: String,
    conversation_id: String,
    user_id: String,
    content: String,
    options: AgentMessageOptions,
) -> Result<axum::response::Response, ApiError> {
    let role = project_access_role(&state.pool, &project_id, &user_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::forbidden("no access to this project"))?;
    let permission_policy = PermissionPolicy::for_role(role);
    let connection = load_active_connection(&state).await?;
    let provider = connection.provider();
    let root = project_root_for_id(&state, &project_id).await?;

    let mut query = content;
    if let Some(form_result) = &options.form_result {
        let request_id = options.resume_request_id.clone().unwrap_or_default();
        let rendered = serde_json::to_string_pretty(form_result)
            .map_err(|_| ApiError::bad_request("invalid formResult payload"))?;
        if !query.is_empty() {
            query.push_str("\n\n");
        }
        query.push_str(&format!(
            "<user_form_response request-id=\"{request_id}\">\n{rendered}\n</user_form_response>"
        ));
    }
    if query.trim().is_empty() {
        return Err(ApiError::bad_request("message content must not be empty"));
    }

    let history = list_messages(&state.pool, &conversation_id)
        .await?
        .iter()
        .rev()
        .take(MAX_HISTORY_MESSAGES)
        .rev()
        .map(|message| AgentConversationMessage {
            role: message.role.clone(),
            content: message.content.clone(),
        })
        .collect::<Vec<_>>();
    append_message(&state.pool, &conversation_id, "user", &query, None).await?;

    let skills = {
        let project_path = root.as_path().to_path_buf();
        let global_dir = state.global_skills_dir.clone();
        let skill_mode = options.skill_mode;
        let requested = options.skill.iter().cloned().collect::<Vec<_>>();
        tokio::task::spawn_blocking(move || match skill_mode {
            AgentSkillMode::Explicit => load_skills(&project_path, global_dir.as_deref(), &requested),
            AgentSkillMode::Auto => {
                let ids = list_available_skills(&project_path, global_dir.as_deref())
                    .into_iter()
                    .map(|skill| skill.id)
                    .collect::<Vec<_>>();
                load_skills(&project_path, global_dir.as_deref(), &ids)
            }
        })
        .await
        .map_err(|_| ApiError::internal("failed to load agent skills"))?
    };
    if options.skill_mode == AgentSkillMode::Explicit && options.skill.is_some() && skills.is_empty()
    {
        return Err(ApiError::bad_request("requested agent skill was not found"));
    }

    let loop_request = AgentLoopRequest {
        query,
        session_id: conversation_id.clone(),
        mode: options.mode,
        skill_mode: options.skill_mode,
        web_enabled: options.web,
        history,
        skills,
        context_files: Vec::new(),
        approved_shell_commands: options.approved_shell_commands.clone(),
    };

    let run_id = Uuid::new_v4().to_string();
    let cancellation = state
        .agent_cancellations
        .start(&project_id, &conversation_id, &run_id);

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>();
    let event_sink: AgentEventSink = Arc::new(move |mut event: AgentEvent| {
        event.redact_for_external_api();
        let _ = event_tx.send(event);
    });

    let agent_mode = options.mode.label().to_string();
    let loop_state = state.clone();
    let loop_project_id = project_id.clone();
    let loop_handle = tokio::spawn(async move {
        run_agent_loop(
            &loop_state,
            &loop_project_id,
            &root,
            &provider,
            &permission_policy,
            &loop_request,
            Some(event_sink),
            Some(&cancellation),
        )
        .await
    });

    let stream_state = state.clone();
    let event_stream = async_stream::stream! {
        while let Some(event) = event_rx.recv().await {
            let Ok(payload) = serde_json::to_string(&event) else {
                continue;
            };
            yield Ok(Event::default().event("agentEvent").data(payload));
        }

        let outcome = match loop_handle.await {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(message)) => {
                yield Ok(
                    Event::default()
                        .event("error")
                        .data(json!({ "message": message }).to_string()),
                );
                return;
            }
            Err(_) => {
                yield Ok(
                    Event::default()
                        .event("error")
                        .data(json!({ "message": "agent run failed unexpectedly" }).to_string()),
                );
                return;
            }
        };

        if let Some(request) = outcome.user_input_request {
            // Paused for user input: nothing is persisted, the frontend
            // resumes with a fresh POST carrying the form result.
            yield Ok(
                Event::default().event("done").data(
                    json!({
                        "messageId": Value::Null,
                        "content": outcome.message,
                        "agentMode": agent_mode,
                        "userInputRequest": request
                    })
                    .to_string(),
                ),
            );
            return;
        }

        let mut events = outcome.events;
        for event in &mut events {
            event.redact_for_external_api();
        }
        let events_value =
            serde_json::to_value(&events).unwrap_or_else(|_| Value::Array(Vec::new()));
        match append_agent_message(
            &stream_state.pool,
            &conversation_id,
            "assistant",
            &outcome.message,
            None,
            Some(&agent_mode),
            Some(&events_value),
        )
        .await
        {
            Ok(message) => {
                yield Ok(
                    Event::default().event("done").data(
                        json!({
                            "messageId": message.id,
                            "content": outcome.message,
                            "agentMode": agent_mode,
                            "references": outcome.references
                        })
                        .to_string(),
                    ),
                );
            }
            Err(_) => {
                yield Ok(
                    Event::default()
                        .event("error")
                        .data(json!({ "message": "failed to persist assistant message" }).to_string()),
                );
            }
        }
    };

    let event_stream: ChatEventStream = event_stream.boxed();
    Ok(Sse::new(event_stream).keep_alive(KeepAlive::default()).into_response())
}

fn conversation_json(record: &ConversationRecord) -> Value {
    json!({
        "id": record.id,
        "projectId": record.project_id,
        "title": record.title,
        "createdAt": record.created_at,
        "updatedAt": record.updated_at
    })
}

pub(crate) fn message_json(record: &MessageRecord) -> Value {
    json!({
        "id": record.id,
        "role": record.role,
        "content": record.content,
        "contextSummary": record.context_summary,
        "agentMode": record.agent_mode,
        "agentEvents": record.agent_events,
        "createdAt": record.created_at
    })
}
