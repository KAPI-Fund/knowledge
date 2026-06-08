use anyhow::Result;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub enum MockScenario {
    Success,
    RetryableError,
    InvalidRequest,
}

impl MockScenario {
    pub fn success() -> Self {
        Self::Success
    }

    #[allow(dead_code)]
    pub fn retryable_error() -> Self {
        Self::RetryableError
    }

    #[allow(dead_code)]
    pub fn invalid_request() -> Self {
        Self::InvalidRequest
    }
}

#[derive(Clone)]
struct MockState {
    scenario: Arc<Mutex<MockScenario>>,
}

pub struct MockOpenAiServer {
    addr: SocketAddr,
}

impl MockOpenAiServer {
    pub async fn start(scenario: MockScenario) -> Result<Self> {
        let state = MockState {
            scenario: Arc::new(Mutex::new(scenario)),
        };
        let app = Router::new()
            .route("/v1/chat/completions", post(chat_completions))
            .with_state(state);
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        Ok(Self { addr })
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

async fn chat_completions(
    State(state): State<MockState>,
    Json(_payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let scenario = state.scenario.lock().await.clone();
    match scenario {
        MockScenario::Success => (
            StatusCode::OK,
            Json(json!({
              "id": "chatcmpl-mock-1",
              "object": "chat.completion",
              "created": 1_717_171_717,
              "model": "mock-model",
              "choices": [
                {
                  "index": 0,
                  "message": {
                    "role": "assistant",
                    "content": "Attention focuses computation on relevant tokens."
                  },
                  "finish_reason": "stop"
                }
              ],
              "usage": {
                "prompt_tokens": 17,
                "completion_tokens": 25,
                "total_tokens": 42
              }
            })),
        ),
        MockScenario::RetryableError => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
              "error": {
                "message": "rate limit",
                "type": "rate_limit_error"
              }
            })),
        ),
        MockScenario::InvalidRequest => (
            StatusCode::BAD_REQUEST,
            Json(json!({
              "error": {
                "message": "invalid request",
                "type": "invalid_request_error"
              }
            })),
        ),
    }
}
