use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderQueryRequest {
    pub query: String,
    pub context_blocks: Vec<String>,
    pub language: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTextRequest {
  pub system_prompt: String,
  pub user_prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderMultimodalRequest {
  pub system_prompt: String,
  pub content_blocks: Vec<ProviderContentBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderContentBlock {
  Text { text: String },
  Image { media_type: String, data_base64: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderChatStreamRequest {
    pub system_prompt: String,
    pub messages: Vec<ProviderChatMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderEmbeddingRequest {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAnswer {
    pub answer: String,
    pub citations: Vec<ProviderCitation>,
    pub usage: ProviderUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTextResponse {
    pub text: String,
    pub usage: ProviderUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCitation {
    pub path: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct ProviderImageRequest {
    pub prompt: String,
    pub size: String,
}

#[derive(Debug, Clone)]
pub struct ProviderImageResult {
    /// e.g. "image/png"
    pub mime: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct ProviderError {
  code: String,
  message: String,
  retryable: bool,
  provider_status: Option<u16>,
}

impl ProviderError {
  pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
    Self {
      code: code.into(),
      message: message.into(),
      retryable,
      provider_status: None,
    }
  }

  pub fn with_status(mut self, provider_status: u16) -> Self {
    self.provider_status = Some(provider_status);
    self
  }

  pub fn code(&self) -> &str {
    &self.code
  }

  pub fn message(&self) -> &str {
    &self.message
  }

  pub fn retryable(&self) -> bool {
    self.retryable
  }

  pub fn provider_status(&self) -> Option<u16> {
    self.provider_status
  }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_image_request_holds_prompt_and_size() {
        let req = ProviderImageRequest {
            prompt: "a red fox".to_string(),
            size: "512x512".to_string(),
        };
        assert_eq!(req.prompt, "a red fox");
        assert_eq!(req.size, "512x512");
    }

    #[test]
    fn provider_image_result_carries_mime_and_bytes() {
        let res = ProviderImageResult { mime: "image/png".to_string(), bytes: vec![9, 9, 9] };
        assert_eq!(res.mime, "image/png");
        assert_eq!(res.bytes, vec![9, 9, 9]);
    }
}
