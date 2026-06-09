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
