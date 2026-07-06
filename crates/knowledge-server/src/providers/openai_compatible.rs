use std::time::Duration;

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::providers::types::{
    ProviderAnswer, ProviderChatStreamRequest, ProviderContentBlock, ProviderEmbeddingRequest,
    ProviderError, ProviderImageRequest, ProviderImageResult, ProviderMultimodalRequest,
    ProviderQueryRequest, ProviderTextRequest, ProviderTextResponse, ProviderUsage,
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
        if !status.is_success() {
            let body = response.text().await.map_err(map_transport_error)?;
            let payload: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            return Err(map_provider_error(status, &payload));
        }

        // Stream the SSE body and reassemble the content deltas into one text.
        // Streaming keeps the connection continuously active, so the gateway's
        // ~60s *idle* timeout never fires on a long generation. A non-streamed
        // read blocks for the whole body with zero bytes flowing and gets cut at
        // 60s (verified live: non-stream died at 60.3s, stream survived 171s and
        // completed). Callers still get a fully-aggregated ProviderTextResponse.
        let mut text = String::new();
        let mut usage = ChatUsage::default();
        let mut bytes_stream = response.bytes_stream();
        let mut buffer = String::new();
        while let Some(chunk) = bytes_stream.next().await {
            let chunk = chunk.map_err(map_transport_error)?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(newline) = buffer.find('\n') {
                let line = buffer[..newline].trim().to_string();
                buffer.drain(..=newline);
                apply_stream_line(&line, &mut text, &mut usage);
            }
        }
        // The final SSE line may arrive without a trailing newline.
        apply_stream_line(buffer.trim(), &mut text, &mut usage);

        let text = text.trim();
        if text.is_empty() {
            return Err(ProviderError::new(
                "provider_invalid_response",
                "provider did not return assistant content",
                false,
            ));
        }

        Ok(ProviderTextResponse {
            text: text.to_string(),
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

    pub fn images_url(&self) -> String {
        let trimmed = self.base_url.trim_end_matches('/');
        if trimmed.ends_with("/v1") {
            format!("{trimmed}/images/generations")
        } else {
            format!("{trimmed}/v1/images/generations")
        }
    }

    pub async fn generate_image(
        &self,
        request: ProviderImageRequest,
    ) -> Result<ProviderImageResult, ProviderError> {
        use base64::Engine;

        // gpt-image-* returns b64_json unconditionally and 400s if response_format
        // is present; other backends (DALL·E, compatible servers) still need it.
        let response_format = if self.model.starts_with("gpt-image") {
            None
        } else {
            Some("b64_json".to_string())
        };
        let body = ImageGenerationRequest {
            model: self.model.clone(),
            prompt: request.prompt,
            size: request.size,
            n: 1,
            response_format,
        };

        let response = self
            .client
            .post(self.images_url())
            .headers(self.auth_headers()?)
            .json(&body)
            .send()
            .await
            .map_err(map_transport_error)?;

        let status = response.status();
        let text = response.text().await.map_err(map_transport_error)?;
        let payload: Value = serde_json::from_str(&text).map_err(|error| {
            ProviderError::new(
                "provider_invalid_response",
                format!("provider returned invalid JSON: {error}"),
                false,
            )
        })?;

        if !status.is_success() {
            return Err(map_provider_error(status, &payload));
        }

        let b64 = payload
            .get("data")
            .and_then(|data| data.get(0))
            .and_then(|item| item.get("b64_json"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ProviderError::new(
                    "provider_invalid_response",
                    "provider did not return data[0].b64_json",
                    false,
                )
            })?;

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|error| {
                ProviderError::new(
                    "provider_invalid_response",
                    format!("provider returned invalid base64: {error}"),
                    false,
                )
            })?;

        Ok(ProviderImageResult {
            mime: "image/png".to_string(),
            bytes,
        })
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

/// Apply one SSE line to the aggregation state used by `complete_text`: append any
/// content delta to `text`, and capture a `usage` object if the chunk carries one
/// (the terminal usage-bearing chunk has an empty `choices` array). Non-data lines,
/// `[DONE]`, and unparseable payloads are ignored.
fn apply_stream_line(line: &str, text: &mut String, usage: &mut ChatUsage) {
    let Some(data) = line.strip_prefix("data:").map(str::trim) else {
        return;
    };
    if data.is_empty() || data == "[DONE]" {
        return;
    }
    let Ok(payload) = serde_json::from_str::<Value>(data) else {
        return;
    };
    if let Some(delta) = payload
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("delta"))
        .and_then(|delta| delta.get("content"))
        .and_then(Value::as_str)
    {
        text.push_str(delta);
    }
    if let Some(chunk_usage) = payload
        .get("usage")
        .and_then(|value| serde_json::from_value::<ChatUsage>(value.clone()).ok())
    {
        *usage = chunk_usage;
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
        // Some gateways (e.g. IntelAlloc) return 404 + model_not_found when an
        // account/route is transiently unavailable under load, using the same
        // shape as a genuinely-missing model. We can't tell the two apart from
        // the response, so treat it as retryable and let max_attempts bound it:
        // transient overload recovers on retry, a real misconfig fails after a
        // few bounded attempts. 408 (request timeout) is likewise transient.
        StatusCode::NOT_FOUND | StatusCode::REQUEST_TIMEOUT => {
            ProviderError::new("provider_upstream_unavailable", message.to_string(), true)
                .with_status(status.as_u16())
        }
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
            // Stream so the gateway's ~60s idle timeout never fires mid-generation
            // (complete_text reassembles the deltas into one text).
            stream: Some(true),
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

#[derive(Debug, Serialize)]
struct ImageGenerationRequest {
    model: String,
    prompt: String,
    size: String,
    n: u8,
    // gpt-image-* models always return b64_json and reject an explicit
    // response_format, so it is omitted for them (None -> not serialized).
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<String>,
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
  fn maps_rate_limit_and_server_errors_as_retryable() {
    let body = serde_json::json!({"error": {"message": "slow down", "type": "rate_limit_error"}});
    let err = map_provider_error(StatusCode::TOO_MANY_REQUESTS, &body);
    assert!(err.retryable(), "429 must be retryable");
    assert_eq!(err.code(), "provider_rate_limited");

    let body = serde_json::json!({"error": {"message": "upstream down"}});
    let err = map_provider_error(StatusCode::SERVICE_UNAVAILABLE, &body);
    assert!(err.retryable(), "503 must be retryable");
  }

  #[test]
  fn maps_gateway_overload_404_as_retryable() {
    // The IntelAlloc gateway returns 404 + model_not_found when an account/route
    // is transiently unavailable under load — indistinguishable from a genuinely
    // wrong model name. Treat it as retryable (bounded by max_attempts): transient
    // overload auto-recovers, a truly-misconfigured model just fails after a few
    // bounded attempts instead of instantly. This is the exact error that killed
    // the ingest batch on the first attempt.
    let body = serde_json::json!({
      "error": {
        "message": "Model \"gpt-5.4\" is not supported by any configured account in this group",
        "type": "model_not_found"
      }
    });
    let err = map_provider_error(StatusCode::NOT_FOUND, &body);
    assert!(err.retryable(), "gateway overload 404 must be retryable");
    assert_eq!(err.provider_status(), Some(404));
  }

  #[test]
  fn maps_invalid_request_400_as_permanent() {
    // A real client-side bad request (malformed payload) must NOT be retried —
    // retrying can't fix it and only wastes the gateway's concurrency budget.
    let body = serde_json::json!({
      "error": {"message": "invalid 'messages'", "type": "invalid_request_error"}
    });
    let err = map_provider_error(StatusCode::BAD_REQUEST, &body);
    assert!(!err.retryable(), "invalid_request 400 must be permanent");
    assert_eq!(err.code(), "provider_invalid_request");
  }

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
    // complete_text now streams too (to dodge the gateway's 60s idle cut), so
    // from_text also sets stream:true.
    assert_eq!(serde_json::to_value(plain).unwrap()["stream"], true);
  }

  #[tokio::test]
  async fn complete_text_streams_and_aggregates_delta_content() {
    // The gateway cuts an *idle* (non-streaming) connection at ~60s: while the
    // model generates, a non-streamed request moves zero bytes and gets killed
    // (verified live — non-stream died at 60.3s, stream survived 171s). So
    // complete_text must request stream:true and reassemble the SSE deltas into
    // one text, keeping the socket alive the whole time. This mock streams three
    // content chunks then a usage-bearing chunk and [DONE]; complete_text must
    // return the concatenated text and the final usage.
    let sse = concat!(
      "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
      "data: {\"choices\":[{\"delta\":{\"content\":\", \"}}]}\n\n",
      "data: {\"choices\":[{\"delta\":{\"content\":\"world\"}}]}\n\n",
      "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":5,\"total_tokens\":8}}\n\n",
      "data: [DONE]\n\n",
    );
    let (handle, base) = spawn_mock_sse_server(sse.to_string()).await;

    let provider = OpenAiCompatibleProvider::new(base, "key".to_string(), "gpt-4o".to_string(), 30);
    let response = provider
      .complete_text(ProviderTextRequest {
        system_prompt: "s".to_string(),
        user_prompt: "u".to_string(),
      })
      .await
      .expect("streamed completion");

    assert_eq!(response.text, "Hello, world");
    assert_eq!(response.usage.prompt_tokens, 3);
    assert_eq!(response.usage.completion_tokens, 5);
    assert_eq!(response.usage.total_tokens, 8);
    handle.abort();
  }

  #[tokio::test]
  async fn complete_text_maps_error_status_from_stream_endpoint() {
    // A non-2xx response on the streaming path must still map through
    // map_provider_error (e.g. 429 -> retryable) rather than being treated as a
    // stream body.
    let (handle, base) = spawn_mock_status_server(
      429,
      "{\"error\":{\"message\":\"slow down\",\"type\":\"rate_limit_error\"}}".to_string(),
    )
    .await;

    let provider = OpenAiCompatibleProvider::new(base, "key".to_string(), "gpt-4o".to_string(), 30);
    let err = provider
      .complete_text(ProviderTextRequest {
        system_prompt: "s".to_string(),
        user_prompt: "u".to_string(),
      })
      .await
      .expect_err("429 must surface as an error");

    assert_eq!(err.code(), "provider_rate_limited");
    assert!(err.retryable());
    handle.abort();
  }

  async fn spawn_mock_sse_server(body: String) -> (tokio::task::JoinHandle<()>, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
      if let Ok((mut sock, _)) = listener.accept().await {
        let mut buf = [0u8; 4096];
        let _ = sock.read(&mut buf).await;
        let resp = format!(
          "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
          body.len(),
          body
        );
        let _ = sock.write_all(resp.as_bytes()).await;
        let _ = sock.flush().await;
      }
    });
    (handle, format!("http://{addr}"))
  }

  async fn spawn_mock_status_server(
    status: u16,
    body: String,
  ) -> (tokio::task::JoinHandle<()>, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
      if let Ok((mut sock, _)) = listener.accept().await {
        let mut buf = [0u8; 4096];
        let _ = sock.read(&mut buf).await;
        let resp = format!(
          "HTTP/1.1 {} STATUS\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
          status,
          body.len(),
          body
        );
        let _ = sock.write_all(resp.as_bytes()).await;
        let _ = sock.flush().await;
      }
    });
    (handle, format!("http://{addr}"))
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

#[cfg(test)]
mod image_tests {
  use super::*;
  use crate::providers::types::ProviderImageRequest;
  use base64::Engine;

  #[test]
  fn image_generation_request_serializes_expected_values() {
    let body = ImageGenerationRequest {
      model: "dall-e-3".to_string(),
      prompt: "a red fox".to_string(),
      size: "1024x1024".to_string(),
      n: 1,
      response_format: Some("b64_json".to_string()),
    };
    let json = serde_json::to_value(&body).unwrap();
    assert_eq!(json["model"], "dall-e-3");
    assert_eq!(json["prompt"], "a red fox");
    assert_eq!(json["n"], 1);
    assert_eq!(json["response_format"], "b64_json");
  }

  #[test]
  fn image_generation_request_omits_none_response_format() {
    let body = ImageGenerationRequest {
      model: "gpt-image-1".to_string(),
      prompt: "a red fox".to_string(),
      size: "1024x1024".to_string(),
      n: 1,
      response_format: None,
    };
    let json = serde_json::to_value(&body).unwrap();
    assert!(
      json.get("response_format").is_none(),
      "response_format must be omitted for gpt-image models"
    );
  }

  #[test]
  fn images_url_appends_v1_images_generations() {
    let provider = OpenAiCompatibleProvider::new(
      "https://api.example.com".to_string(),
      "key".to_string(),
      "gpt-image-1".to_string(),
      30,
    );
    assert_eq!(
      provider.images_url(),
      "https://api.example.com/v1/images/generations"
    );
  }

  #[tokio::test]
  async fn generate_image_decodes_b64_payload() {
    let raw = vec![1u8, 2, 3, 4];
    let b64 = base64::engine::general_purpose::STANDARD.encode(&raw);
    let body = format!("{{\"data\":[{{\"b64_json\":\"{b64}\"}}]}}");
    let (handle, base) = spawn_mock_images_server(body).await;

    let provider =
      OpenAiCompatibleProvider::new(base, "key".to_string(), "gpt-image-1".to_string(), 30);
    let result = provider
      .generate_image(ProviderImageRequest { prompt: "x".to_string(), size: "1024x1024".to_string() })
      .await
      .expect("image generated");

    assert_eq!(result.mime, "image/png");
    assert_eq!(result.bytes, raw);
    handle.abort();
  }

  #[test]
  fn image_generation_request_serializes_size() {
    let body = ImageGenerationRequest {
      model: "gpt-image-1".to_string(),
      prompt: "a fox".to_string(),
      size: "512x512".to_string(),
      n: 1,
      response_format: None,
    };
    let json = serde_json::to_value(&body).unwrap();
    assert_eq!(json["size"], "512x512");
    assert_eq!(json["model"], "gpt-image-1");
  }

  async fn spawn_mock_images_server(json_body: String) -> (tokio::task::JoinHandle<()>, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
      if let Ok((mut sock, _)) = listener.accept().await {
        let mut buf = [0u8; 2048];
        let _ = sock.read(&mut buf).await;
        let resp = format!(
          "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
          json_body.len(),
          json_body
        );
        let _ = sock.write_all(resp.as_bytes()).await;
        let _ = sock.flush().await;
      }
    });
    (handle, format!("http://{addr}"))
  }
}
