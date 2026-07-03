use std::convert::Infallible;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, patch};
use axum::{Json, Router};
use futures_util::{Stream, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::state::AppState;
use crate::chat::context::assemble_chat_context;
use crate::chat::store::{
    ConversationRecord, MessageRecord, append_message, create_conversation, delete_conversation,
    find_conversation, list_conversations, list_messages, rename_conversation,
};
use crate::http::error::ApiError;
use crate::projects::routes::{authorized_principal, validate_csrf};
use crate::projects::service::project_root_for_id;
use crate::providers::{load_active_connection, ProviderChatMessage, ProviderChatStreamRequest};
use crate::query::load_query_settings;

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
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let session = authorized_principal(&state, &headers, Some(&project_id)).await?;
    validate_csrf(&headers, &session)?;
    let content = payload.content.trim().to_string();
    if content.is_empty() {
        return Err(ApiError::bad_request("message content must not be empty"));
    }
    find_conversation(&state.pool, &project_id, &conversation_id, &session.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("conversation not found"))?;

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

    Ok(Sse::new(event_stream).keep_alive(KeepAlive::default()))
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
        "createdAt": record.created_at
    })
}
