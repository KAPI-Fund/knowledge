use anyhow::Result;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub enum MockScenario {
    Success,
    IngestSuccess,
    IngestWithReviewSuccess,
    IngestThenSweepLlmSuccess,
    RetryableError,
    InvalidRequest,
}

impl MockScenario {
    pub fn success() -> Self {
        Self::Success
    }

    #[allow(dead_code)]
    pub fn ingest_success() -> Self {
        Self::IngestSuccess
    }

    #[allow(dead_code)]
    pub fn ingest_with_review_success() -> Self {
        Self::IngestWithReviewSuccess
    }

    #[allow(dead_code)]
    pub fn ingest_then_sweep_llm_success() -> Self {
        Self::IngestThenSweepLlmSuccess
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
    request_count: Arc<AtomicUsize>,
}

pub struct MockOpenAiServer {
    addr: SocketAddr,
    request_count: Arc<AtomicUsize>,
}

impl MockOpenAiServer {
    pub async fn start(scenario: MockScenario) -> Result<Self> {
        let request_count = Arc::new(AtomicUsize::new(0));
        let state = MockState {
            scenario: Arc::new(Mutex::new(scenario)),
            request_count: request_count.clone(),
        };
        let app = Router::new()
            .route("/v1/chat/completions", post(chat_completions))
            .with_state(state);
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        Ok(Self { addr, request_count })
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn request_count(&self) -> usize {
        self.request_count.load(Ordering::SeqCst)
    }
}

async fn chat_completions(
    State(state): State<MockState>,
    Json(_payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let scenario = state.scenario.lock().await.clone();
    let request_index = state.request_count.fetch_add(1, Ordering::SeqCst) + 1;
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
        MockScenario::IngestSuccess
        | MockScenario::IngestWithReviewSuccess
        | MockScenario::IngestThenSweepLlmSuccess => {
            let content = if request_index == 1 {
                [
                    "## Key Entities",
                    "- Attention mechanism - concept central to the source",
                    "",
                    "## Key Concepts",
                    "- Attention - a mechanism for focusing computation on relevant tokens",
                    "",
                    "## Main Arguments & Findings",
                    "- The source explains that transformers use attention mechanisms.",
                    "",
                    "## Connections to Existing Wiki",
                    "- This should create a source summary and a concept page.",
                    "",
                    "## Contradictions & Tensions",
                    "- None noted.",
                    "",
                    "## Recommendations",
                    "- Create or update an attention concept page.",
                ]
                .join("\n")
            } else if request_index == 2 {
                [
                    "---FILE: wiki/sources/attention.md---",
                    "---",
                    "type: source",
                    "title: Attention",
                    "created: 2026-06-08",
                    "updated: 2026-06-08",
                    "tags: [transformers]",
                    "related: [attention-mechanism]",
                    "sources: [\"attention.md\"]",
                    "---",
                    "",
                    "# Attention",
                    "",
                    "Transformers use attention mechanisms.",
                    "---END FILE---",
                    "",
                    "---FILE: wiki/concepts/attention-mechanism.md---",
                    "---",
                    "type: concept",
                    "title: Attention Mechanism",
                    "created: 2026-06-08",
                    "updated: 2026-06-08",
                    "tags: [transformers, attention]",
                    "related: [attention]",
                    "sources: [\"attention.md\"]",
                    "---",
                    "",
                    "# Attention Mechanism",
                    "",
                    "Attention focuses computation on relevant tokens and links back to [[attention]].",
                    "---END FILE---",
                    "",
                    "---FILE: wiki/index.md---",
                    "# Wiki Index",
                    "",
                    "## Entities",
                    "",
                    "## Concepts",
                    "- [[attention-mechanism]] - Token relevance mechanism",
                    "",
                    "## Sources",
                    "- [[attention]] - Source summary",
                    "",
                    "## Queries",
                    "",
                    "## Comparisons",
                    "",
                    "## Synthesis",
                    "---END FILE---",
                    "",
                    "---FILE: wiki/log.md---",
                    "# Research Log",
                    "",
                    "## 2026-06-08",
                    "",
                    "- Project created",
                    "",
                    "## 2026-06-08 ingest | Attention",
                    "",
                    "---END FILE---",
                    "",
                    "---FILE: wiki/overview.md---",
                    "---",
                    "type: overview",
                    "title: Project Overview",
                    "tags: []",
                    "related: []",
                    "sources: []",
                    "---",
                    "",
                    "# Overview",
                    "",
                    "This wiki covers attention and transformer mechanisms.",
                    "---END FILE---",
                    if matches!(scenario, MockScenario::IngestWithReviewSuccess) {
                        "\n---REVIEW: missing-page | Missing evaluation page---\nCreate a dedicated page for evaluation gaps surfaced by the source.\nOPTIONS: Create Page | Skip\nPAGES: wiki/concepts/attention-mechanism.md\nSEARCH: transformer evaluation gaps | attention mechanism benchmarking\n---END REVIEW---"
                    } else {
                        ""
                    },
                ]
                .join("\n")
            } else if matches!(scenario, MockScenario::IngestThenSweepLlmSuccess) {
                "{\"resolved\": [\"review-context-window\"]}".to_string()
            } else {
                "{\"resolved\": []}".to_string()
            };

            (
                StatusCode::OK,
                Json(json!({
                  "id": format!("chatcmpl-mock-{request_index}"),
                  "object": "chat.completion",
                  "created": 1_717_171_717,
                  "model": "mock-model",
                  "choices": [
                    {
                      "index": 0,
                      "message": {
                        "role": "assistant",
                        "content": content
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
            )
        }
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
