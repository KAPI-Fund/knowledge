use sqlx::PgPool;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::http::error::ApiError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConversationRecord {
    pub id: String,
    pub project_id: String,
    pub user_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MessageRecord {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub context_summary: Option<String>,
    pub agent_mode: Option<String>,
    pub agent_events: Option<serde_json::Value>,
    pub created_at: String,
}

pub async fn create_conversation(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    title: &str,
) -> Result<ConversationRecord, ApiError> {
    let now = now_rfc3339()?;
    let record = ConversationRecord {
        id: Uuid::new_v4().to_string(),
        project_id: project_id.to_string(),
        user_id: user_id.to_string(),
        title: title.to_string(),
        created_at: now.clone(),
        updated_at: now,
    };

    sqlx::query(
        "INSERT INTO conversations (id, project_id, user_id, title, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&record.id)
    .bind(&record.project_id)
    .bind(&record.user_id)
    .bind(&record.title)
    .bind(&record.created_at)
    .bind(&record.updated_at)
    .execute(pool)
    .await
    .map_err(ApiError::from)?;

    Ok(record)
}

pub async fn list_conversations(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
) -> Result<Vec<ConversationRecord>, ApiError> {
    sqlx::query_as::<_, ConversationRecord>(
        "SELECT id, project_id, user_id, title, created_at, updated_at
         FROM conversations
         WHERE project_id = $1 AND user_id = $2
         ORDER BY updated_at DESC",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(ApiError::from)
}

pub async fn find_conversation(
    pool: &PgPool,
    project_id: &str,
    conversation_id: &str,
    user_id: &str,
) -> Result<Option<ConversationRecord>, ApiError> {
    sqlx::query_as::<_, ConversationRecord>(
        "SELECT id, project_id, user_id, title, created_at, updated_at
         FROM conversations
         WHERE id = $1 AND project_id = $2 AND user_id = $3",
    )
    .bind(conversation_id)
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::from)
}

pub async fn rename_conversation(
    pool: &PgPool,
    project_id: &str,
    conversation_id: &str,
    user_id: &str,
    title: &str,
) -> Result<Option<ConversationRecord>, ApiError> {
    let now = now_rfc3339()?;
    sqlx::query_as::<_, ConversationRecord>(
        "UPDATE conversations
         SET title = $1, updated_at = $2
         WHERE id = $3 AND project_id = $4 AND user_id = $5
         RETURNING id, project_id, user_id, title, created_at, updated_at",
    )
    .bind(title)
    .bind(now)
    .bind(conversation_id)
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::from)
}

pub async fn delete_conversation(
    pool: &PgPool,
    project_id: &str,
    conversation_id: &str,
    user_id: &str,
) -> Result<bool, ApiError> {
    let result =
        sqlx::query("DELETE FROM conversations WHERE id = $1 AND project_id = $2 AND user_id = $3")
            .bind(conversation_id)
            .bind(project_id)
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(ApiError::from)?;
    Ok(result.rows_affected() > 0)
}

pub async fn list_messages(
    pool: &PgPool,
    conversation_id: &str,
) -> Result<Vec<MessageRecord>, ApiError> {
    sqlx::query_as::<_, MessageRecord>(
        "SELECT id, conversation_id, role, content, context_summary, agent_mode, agent_events, created_at
         FROM conversation_messages
         WHERE conversation_id = $1
         ORDER BY created_at ASC, id ASC",
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await
    .map_err(ApiError::from)
}

/// Scoped like `find_conversation`: the JOIN enforces that the message belongs
/// to the given conversation, project, and requesting user.
pub async fn find_message_by_id(
    pool: &PgPool,
    project_id: &str,
    conversation_id: &str,
    user_id: &str,
    message_id: &str,
) -> Result<Option<MessageRecord>, ApiError> {
    sqlx::query_as::<_, MessageRecord>(
        "SELECT m.id, m.conversation_id, m.role, m.content, m.context_summary, m.agent_mode, m.agent_events, m.created_at
         FROM conversation_messages m
         JOIN conversations c ON c.id = m.conversation_id
         WHERE m.id = $1 AND m.conversation_id = $2 AND c.project_id = $3 AND c.user_id = $4",
    )
    .bind(message_id)
    .bind(conversation_id)
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::from)
}

pub async fn append_message(
    pool: &PgPool,
    conversation_id: &str,
    role: &str,
    content: &str,
    context_summary: Option<&str>,
) -> Result<MessageRecord, ApiError> {
    append_agent_message(pool, conversation_id, role, content, context_summary, None, None).await
}

pub async fn append_agent_message(
    pool: &PgPool,
    conversation_id: &str,
    role: &str,
    content: &str,
    context_summary: Option<&str>,
    agent_mode: Option<&str>,
    agent_events: Option<&serde_json::Value>,
) -> Result<MessageRecord, ApiError> {
    let now = now_rfc3339()?;
    let agent_events_json = agent_events
        .map(|value| serde_json::to_string(value))
        .transpose()
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let record = MessageRecord {
        id: Uuid::new_v4().to_string(),
        conversation_id: conversation_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        context_summary: context_summary.map(str::to_string),
        agent_mode: agent_mode.map(str::to_string),
        agent_events: agent_events.cloned(),
        created_at: now.clone(),
    };

    sqlx::query(
        "INSERT INTO conversation_messages (id, conversation_id, role, content, context_summary, agent_mode, agent_events, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb, $8)",
    )
    .bind(&record.id)
    .bind(&record.conversation_id)
    .bind(&record.role)
    .bind(&record.content)
    .bind(&record.context_summary)
    .bind(&record.agent_mode)
    .bind(&agent_events_json)
    .bind(&record.created_at)
    .execute(pool)
    .await
    .map_err(ApiError::from)?;

    sqlx::query("UPDATE conversations SET updated_at = $1 WHERE id = $2")
        .bind(&now)
        .bind(conversation_id)
        .execute(pool)
        .await
        .map_err(ApiError::from)?;

    Ok(record)
}

fn now_rfc3339() -> Result<String, ApiError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format timestamp"))
}
