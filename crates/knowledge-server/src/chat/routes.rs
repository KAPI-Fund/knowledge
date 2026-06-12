use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::state::AppState;
use crate::chat::store::{
    ConversationRecord, MessageRecord, create_conversation, delete_conversation,
    find_conversation, list_conversations, list_messages, rename_conversation,
};
use crate::http::error::ApiError;
use crate::projects::routes::{authorized_session, validate_csrf};

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
            get(list_messages_handler),
        )
}

#[derive(Debug, Deserialize)]
struct CreateConversationRequest {
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RenameConversationRequest {
    title: String,
}

async fn list_conversations_handler(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    authorized_session(&state, &headers, Some(&project_id)).await?;
    let conversations = list_conversations(&state.pool, &project_id).await?;
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
    let session = authorized_session(&state, &headers, Some(&project_id)).await?;
    validate_csrf(&headers, &session.csrf_token)?;
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
    let session = authorized_session(&state, &headers, Some(&project_id)).await?;
    validate_csrf(&headers, &session.csrf_token)?;
    if payload.title.trim().is_empty() {
        return Err(ApiError::bad_request("title must not be empty"));
    }
    let conversation = rename_conversation(
        &state.pool,
        &project_id,
        &conversation_id,
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
    let session = authorized_session(&state, &headers, Some(&project_id)).await?;
    validate_csrf(&headers, &session.csrf_token)?;
    let deleted = delete_conversation(&state.pool, &project_id, &conversation_id).await?;
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
    authorized_session(&state, &headers, Some(&project_id)).await?;
    find_conversation(&state.pool, &project_id, &conversation_id)
        .await?
        .ok_or_else(|| ApiError::not_found("conversation not found"))?;
    let messages = list_messages(&state.pool, &conversation_id).await?;
    Ok(Json(json!({
        "messages": messages.iter().map(message_json).collect::<Vec<_>>()
    })))
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
