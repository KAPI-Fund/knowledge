use std::time::Duration;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::providers::types::{
    ProviderAnswer, ProviderError, ProviderQueryRequest, ProviderTextRequest,
    ProviderTextResponse, ProviderUsage,
};

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        timeout_seconds: i64,
    ) -> Self {
        let timeout_seconds = timeout_seconds.max(1) as u64;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .expect("provider client should build");

        Self {
            client,
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    pub async fn answer_query(
        &self,
        request: ProviderQueryRequest,
    ) -> Result<ProviderAnswer, ProviderError> {
        let response = self
            .complete_text(ProviderTextRequest {
                system_prompt: format!(
                    "You answer questions using only the provided wiki context. Respond in {}. If the context is insufficient, say so plainly.",
                    request.language
                ),
                user_prompt: format!(
                    "Question:\n{}\n\nContext:\n{}",
                    request.query,
                    if request.context_blocks.is_empty() {
                        "No relevant wiki context was retrieved.".to_string()
                    } else {
                        request.context_blocks.join("\n\n")
                    }
                ),
            })
            .await?;

        Ok(ProviderAnswer {
            answer: response.text,
            citations: Vec::new(),
            usage: response.usage,
        })
    }

    pub async fn complete_text(
        &self,
        request: ProviderTextRequest,
    ) -> Result<ProviderTextResponse, ProviderError> {
        let response = self
            .client
            .post(chat_completions_url(&self.base_url))
            .headers(self.auth_headers()?)
            .json(&ChatCompletionRequest::from_text(&self.model, request))
            .send()
            .await
            .map_err(map_transport_error)?;

        let status = response.status();
        let body = response.text().await.map_err(map_transport_error)?;
        let payload: Value = serde_json::from_str(&body).map_err(|error| {
            ProviderError::new(
                "provider_invalid_response",
                format!("provider returned invalid JSON: {error}"),
                false,
            )
        })?;

        if !status.is_success() {
            return Err(map_provider_error(status, &payload));
        }

        let response: ChatCompletionResponse =
            serde_json::from_value(payload).map_err(|error| {
                ProviderError::new(
                    "provider_invalid_response",
                    format!("provider returned unsupported response shape: {error}"),
                    false,
                )
            })?;

        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_deref())
            .map(str::trim)
            .filter(|content| !content.is_empty())
            .ok_or_else(|| {
                ProviderError::new(
                    "provider_invalid_response",
                    "provider did not return assistant content",
                    false,
                )
            })?;

        let usage = response.usage.unwrap_or_default();

        Ok(ProviderTextResponse {
            text: content.to_string(),
            usage: ProviderUsage {
                prompt_tokens: usage.prompt_tokens.unwrap_or_default(),
                completion_tokens: usage.completion_tokens.unwrap_or_default(),
                total_tokens: usage.total_tokens.unwrap_or_default(),
            },
        })
    }

    fn auth_headers(&self) -> Result<reqwest::header::HeaderMap, ProviderError> {
        let mut headers = reqwest::header::HeaderMap::new();
        if self.api_key.trim().is_empty() {
            return Ok(headers);
        }

        let bearer = format!("Bearer {}", self.api_key.trim());
        let value = reqwest::header::HeaderValue::from_str(&bearer).map_err(|error| {
            ProviderError::new(
                "provider_invalid_configuration",
                format!("provider API key is invalid: {error}"),
                false,
            )
        })?;
        headers.insert(reqwest::header::AUTHORIZATION, value);
        Ok(headers)
    }
}

fn chat_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{trimmed}/chat/completions")
    } else {
        format!("{trimmed}/v1/chat/completions")
    }
}

fn map_transport_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        return ProviderError::new("provider_timeout", "provider request timed out", true);
    }

    if error.is_connect() || error.is_request() {
        return ProviderError::new(
            "provider_connection_failed",
            format!("provider request failed: {error}"),
            true,
        );
    }

    ProviderError::new(
        "provider_transport_error",
        format!("provider transport error: {error}"),
        false,
    )
}

fn map_provider_error(status: StatusCode, payload: &Value) -> ProviderError {
    let message = payload
        .get("error")
        .and_then(|value| value.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("provider request failed");
    let error_type = payload
        .get("error")
        .and_then(|value| value.get("type"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    match status {
        StatusCode::TOO_MANY_REQUESTS => {
            ProviderError::new("provider_rate_limited", message.to_string(), true)
                .with_status(status.as_u16())
        }
        StatusCode::BAD_REQUEST => ProviderError::new(
            if error_type == "invalid_request_error" {
                "provider_invalid_request"
            } else {
                "provider_request_failed"
            },
            message.to_string(),
            false,
        )
        .with_status(status.as_u16()),
        _ if status.is_server_error() => {
            ProviderError::new("provider_server_error", message.to_string(), true)
                .with_status(status.as_u16())
        }
        _ => ProviderError::new("provider_request_failed", message.to_string(), false)
            .with_status(status.as_u16()),
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

impl ChatCompletionRequest {
    fn from_text(model: &str, request: ProviderTextRequest) -> Self {
        Self {
            model: model.to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: request.system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: request.user_prompt,
                },
            ],
            temperature: 0.0,
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    content: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ChatUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
}
