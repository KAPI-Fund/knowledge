use std::time::Duration;

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::providers::types::{
    ProviderAnswer, ProviderChatStreamRequest, ProviderContentBlock, ProviderEmbeddingRequest,
    ProviderError, ProviderMultimodalRequest, ProviderQueryRequest, ProviderTextRequest,
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

    pub async fn embed_text(
        &self,
        request: ProviderEmbeddingRequest,
    ) -> Result<Vec<f32>, ProviderError> {
        let response = self
            .client
            .post(embeddings_url(&self.base_url))
            .headers(self.auth_headers()?)
            .json(&EmbeddingRequest {
                model: self.model.clone(),
                input: request.text,
            })
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

        let response: EmbeddingResponse = serde_json::from_value(payload).map_err(|error| {
            ProviderError::new(
                "provider_invalid_response",
                format!("provider returned unsupported embedding response shape: {error}"),
                false,
            )
        })?;

        let embedding = response
            .data
            .first()
            .map(|item| item.embedding.clone())
            .filter(|embedding| !embedding.is_empty())
            .ok_or_else(|| {
                ProviderError::new(
                    "provider_invalid_response",
                    "provider did not return an embedding vector",
                    false,
                )
            })?;

        if embedding.iter().any(|value| !value.is_finite()) {
            return Err(ProviderError::new(
                "provider_invalid_response",
                "provider returned a non-finite embedding value",
                false,
            ));
        }

        Ok(embedding)
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

    pub async fn complete_multimodal(
      &self,
      request: ProviderMultimodalRequest,
    ) -> Result<ProviderTextResponse, ProviderError> {
      let response = self
        .client
        .post(chat_completions_url(&self.base_url))
        .headers(self.auth_headers()?)
        .json(&ChatCompletionRequest::from_content_blocks(
          &self.model,
          request.system_prompt,
          request.content_blocks,
        ))
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

      let response: ChatCompletionResponse = serde_json::from_value(payload).map_err(|error| {
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

    pub async fn stream_chat(
        &self,
        request: ProviderChatStreamRequest,
    ) -> Result<BoxStream<'static, Result<String, ProviderError>>, ProviderError> {
        let response = self
            .client
            .post(chat_completions_url(&self.base_url))
            .headers(self.auth_headers()?)
            .json(&ChatCompletionRequest::from_chat_messages(
                &self.model,
                request,
            ))
            .send()
            .await
            .map_err(map_transport_error)?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.map_err(map_transport_error)?;
            let payload: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            return Err(map_provider_error(status, &payload));
        }

        let stream = async_stream::try_stream! {
            let mut bytes_stream = response.bytes_stream();
            let mut buffer = String::new();
            while let Some(chunk) = bytes_stream.next().await {
                let chunk = chunk.map_err(map_transport_error)?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(newline) = buffer.find('\n') {
                    let line = buffer[..newline].trim().to_string();
                    buffer.drain(..=newline);
                    if let Some(delta) = parse_stream_data_line(&line) {
                        yield delta;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
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

fn embeddings_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{trimmed}/embeddings")
    } else {
        format!("{trimmed}/v1/embeddings")
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

fn parse_stream_data_line(line: &str) -> Option<String> {
    let data = line.strip_prefix("data:")?.trim();
    if data.is_empty() || data == "[DONE]" {
        return None;
    }
    let payload: Value = serde_json::from_str(data).ok()?;
    let delta = payload
        .get("choices")?
        .get(0)?
        .get("delta")?
        .get("content")?
        .as_str()?;
    if delta.is_empty() {
        None
    } else {
        Some(delta.to_string())
    }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

impl ChatCompletionRequest {
    fn from_text(model: &str, request: ProviderTextRequest) -> Self {
        Self {
            model: model.to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: ChatMessageContent::Text(request.system_prompt),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: ChatMessageContent::Text(request.user_prompt),
                },
            ],
            temperature: 0.0,
            stream: None,
        }
    }

    fn from_chat_messages(model: &str, request: ProviderChatStreamRequest) -> Self {
        let mut messages = vec![ChatMessage {
            role: "system".to_string(),
            content: ChatMessageContent::Text(request.system_prompt),
        }];
        messages.extend(request.messages.into_iter().map(|message| ChatMessage {
            role: message.role,
            content: ChatMessageContent::Text(message.content),
        }));
        Self {
            model: model.to_string(),
            messages,
            temperature: 0.0,
            stream: Some(true),
        }
    }

    fn from_content_blocks(model: &str, system_prompt: String, content: Vec<ProviderContentBlock>) -> Self {
        Self {
            model: model.to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: ChatMessageContent::Text(system_prompt),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: ChatMessageContent::Blocks(
                        content
                            .into_iter()
                            .map(|block| match block {
                                ProviderContentBlock::Text { text } => ChatMessageBlock::Text { text },
                                ProviderContentBlock::Image {
                                    media_type,
                                    data_base64,
                                } => ChatMessageBlock::Image {
                                    image_url: ChatMessageImageUrl {
                                        url: format!(
                                            "data:{media_type};base64,{data_base64}"
                                        ),
                                    },
                                },
                            })
                            .collect(),
                    ),
                },
            ],
            temperature: 0.0,
            stream: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: ChatMessageContent,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ChatMessageContent {
  Text(String),
  Blocks(Vec<ChatMessageBlock>),
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ChatMessageBlock {
  #[serde(rename = "text")]
  Text { text: String },
  #[serde(rename = "image_url")]
  Image { image_url: ChatMessageImageUrl },
}

#[derive(Debug, Serialize)]
struct ChatMessageImageUrl {
  url: String,
}

#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    input: String,
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

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingResponseItem>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponseItem {
    embedding: Vec<f32>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::providers::types::ProviderContentBlock;
  use crate::providers::types::{ProviderChatMessage, ProviderChatStreamRequest};

  #[test]
  fn parses_stream_data_lines() {
    assert_eq!(
      parse_stream_data_line(r#"data: {"choices":[{"delta":{"content":"Hel"}}]}"#),
      Some("Hel".to_string())
    );
    assert_eq!(parse_stream_data_line("data: [DONE]"), None);
    assert_eq!(parse_stream_data_line(""), None);
    assert_eq!(parse_stream_data_line(": keep-alive"), None);
    assert_eq!(
      parse_stream_data_line(r#"data: {"choices":[{"delta":{}}]}"#),
      None
    );
  }

  #[test]
  fn chat_stream_request_serializes_history_and_stream_flag() {
    let request = ChatCompletionRequest::from_chat_messages(
      "model",
      ProviderChatStreamRequest {
        system_prompt: "system".to_string(),
        messages: vec![
          ProviderChatMessage {
            role: "user".to_string(),
            content: "first".to_string(),
          },
          ProviderChatMessage {
            role: "assistant".to_string(),
            content: "reply".to_string(),
          },
        ],
      },
    );
    let value = serde_json::to_value(request).unwrap();
    assert_eq!(value["stream"], true);
    assert_eq!(value["messages"][0]["role"], "system");
    assert_eq!(value["messages"][2]["role"], "assistant");

    let plain = ChatCompletionRequest::from_text(
      "model",
      crate::providers::types::ProviderTextRequest {
        system_prompt: "s".to_string(),
        user_prompt: "u".to_string(),
      },
    );
    assert!(serde_json::to_value(plain).unwrap().get("stream").is_none());
  }

  #[test]
  fn serializes_image_content_block_for_openai_compatible_chat() {
    let request = ChatCompletionRequest::from_content_blocks(
      "model",
      "system".to_string(),
      vec![
        ProviderContentBlock::Text {
          text: "caption this".to_string(),
        },
        ProviderContentBlock::Image {
          media_type: "image/png".to_string(),
          data_base64: "Zm9v".to_string(),
        },
      ],
    );

    let value = serde_json::to_value(request).unwrap();
    let content = &value["messages"][1]["content"];
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[1]["type"], "image_url");
    assert_eq!(content[1]["image_url"]["url"], "data:image/png;base64,Zm9v");
  }
}
