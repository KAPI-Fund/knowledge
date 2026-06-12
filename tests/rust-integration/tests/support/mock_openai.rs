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
    QuerySaveEnrichSuccess,
    SemanticLintSuccess,
    IngestSuccess,
    IngestSanitizeSuccess,
    IngestWithReviewSuccess,
    IngestWithDedicatedReviewStageSuccess,
    IngestWithPageMergeSuccess,
    IngestThenSweepLlmSuccess,
    RetryableError,
    InvalidRequest,
}

impl MockScenario {
    pub fn success() -> Self {
        Self::Success
    }

    #[allow(dead_code)]
    pub fn query_save_enrich_success() -> Self {
        Self::QuerySaveEnrichSuccess
    }

    #[allow(dead_code)]
    pub fn semantic_lint_success() -> Self {
        Self::SemanticLintSuccess
    }

    #[allow(dead_code)]
    pub fn ingest_success() -> Self {
        Self::IngestSuccess
    }

    #[allow(dead_code)]
    pub fn ingest_sanitize_success() -> Self {
        Self::IngestSanitizeSuccess
    }

    #[allow(dead_code)]
    pub fn ingest_with_review_success() -> Self {
        Self::IngestWithReviewSuccess
    }

    #[allow(dead_code)]
    pub fn ingest_with_dedicated_review_stage_success() -> Self {
        Self::IngestWithDedicatedReviewStageSuccess
    }

    #[allow(dead_code)]
    pub fn ingest_with_page_merge_success() -> Self {
        Self::IngestWithPageMergeSuccess
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
            .route("/v1/embeddings", post(embeddings))
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
    Json(payload): Json<Value>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    if payload.get("stream").and_then(Value::as_bool) == Some(true) {
        let _ = state.request_count.fetch_add(1, Ordering::SeqCst);
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Attention \"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"focuses computation on relevant tokens.\"}}]}\n\n",
            "data: [DONE]\n\n",
        );
        return (
            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
            body,
        )
            .into_response();
    }

    let (status, json) = chat_completions_json(state, payload).await;
    (status, json).into_response()
}

async fn chat_completions_json(state: MockState, payload: Value) -> (StatusCode, Json<Value>) {
    let scenario = state.scenario.lock().await.clone();
    if payload_has_image_block(&payload) {
        return (
            StatusCode::OK,
            Json(json!({
              "id": "chatcmpl-mock-caption",
              "object": "chat.completion",
              "created": 1_717_171_717,
              "model": "mock-model",
              "choices": [
                {
                  "index": 0,
                  "message": {
                    "role": "assistant",
                    "content": "A factual caption for an embedded diagram."
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
        );
    }
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
        MockScenario::QuerySaveEnrichSuccess => (
            StatusCode::OK,
            Json(json!({
              "id": "chatcmpl-mock-enrich",
              "object": "chat.completion",
              "created": 1_717_171_717,
              "model": "mock-model",
              "choices": [
                {
                  "index": 0,
                  "message": {
                    "role": "assistant",
                    "content": "{\"links\":[{\"term\":\"Attention\",\"target\":\"attention\"}]}"
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
        MockScenario::SemanticLintSuccess => (
            StatusCode::OK,
            Json(json!({
              "id": "chatcmpl-mock-semantic-lint",
              "object": "chat.completion",
              "created": 1_717_171_717,
              "model": "mock-model",
              "choices": [
                {
                  "index": 0,
                  "message": {
                    "role": "assistant",
                    "content": "---LINT: contradiction | warning | Conflicting attention claims---\nTwo pages describe attention with conflicting scope.\nPAGES: concepts/attention.md, concepts/attention-mechanism.md\n---END LINT---"
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
        | MockScenario::IngestSanitizeSuccess
        | MockScenario::IngestWithReviewSuccess
        | MockScenario::IngestWithDedicatedReviewStageSuccess
        | MockScenario::IngestWithPageMergeSuccess
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
            } else if matches!(scenario, MockScenario::IngestSanitizeSuccess) && request_index == 2 {
                [
                    "---FILE: wiki/sources/attention.md---",
                    "```yaml",
                    "frontmatter:",
                    "---",
                    "type: source",
                    "title: Attention",
                    "created: 2026-06-08",
                    "updated: 2026-06-08",
                    "tags: [transformers]",
                    "related: [[attention-mechanism]], [[attention]]",
                    "sources: [\"attention.md\"]",
                    "---",
                    "",
                    "# Attention",
                    "",
                    "Transformers use attention mechanisms.",
                    "```",
                    "---END FILE---",
                    "",
                    "---FILE: wiki/index.md---",
                    "# Wiki Index",
                    "",
                    "## Entities",
                    "",
                    "## Concepts",
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
            } else if matches!(scenario, MockScenario::IngestWithPageMergeSuccess) && request_index == 4 {
                [
                    "## Key Entities",
                    "- Attention mechanism - extended by a second source",
                    "",
                    "## Key Concepts",
                    "- Attention variants - implementation tradeoffs for attention mechanisms",
                    "",
                    "## Main Arguments & Findings",
                    "- The second source adds optimization tradeoffs.",
                    "",
                    "## Connections to Existing Wiki",
                    "- This should update the existing attention mechanism page.",
                    "",
                    "## Contradictions & Tensions",
                    "- None noted.",
                    "",
                    "## Recommendations",
                    "- Merge new implementation notes into the existing concept page.",
                ]
                .join("\n")
            } else if matches!(scenario, MockScenario::IngestWithPageMergeSuccess) && request_index == 5 {
                [
                    "---FILE: wiki/sources/attention-optimizations.md---",
                    "---",
                    "type: source",
                    "title: Attention Optimizations",
                    "created: 2026-06-08",
                    "updated: 2026-06-08",
                    "tags: [transformers, optimization]",
                    "related: [attention-mechanism]",
                    "sources: [\"attention-optimizations.md\"]",
                    "---",
                    "",
                    "# Attention Optimizations",
                    "",
                    "Optimized attention implementations reduce memory overhead.",
                    "---END FILE---",
                    "",
                    "---FILE: wiki/concepts/attention-mechanism.md---",
                    "---",
                    "type: concept",
                    "title: Attention Mechanism",
                    "created: 2026-06-08",
                    "updated: 2026-06-08",
                    "tags: [optimization, flash-attention]",
                    "related: [attention, efficient-attention]",
                    "sources: [\"attention-optimizations.md\"]",
                    "---",
                    "",
                    "# Attention Mechanism",
                    "",
                    "Efficient implementations reduce memory overhead and improve throughput.",
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
                    "- [[attention-optimizations]] - Optimization summary",
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
                    "## 2026-06-08 ingest | Attention Optimizations",
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
                    "This wiki covers attention mechanisms and efficient variants.",
                    "---END FILE---",
                ]
                .join("\n")
            } else if matches!(scenario, MockScenario::IngestWithPageMergeSuccess) && request_index == 7 {
                [
                    "---",
                    "type: concept",
                    "title: Attention Mechanism",
                    "created: 2026-06-08",
                    "updated: 2026-06-08",
                    "tags: [optimization, flash-attention]",
                    "related: [attention, efficient-attention]",
                    "sources: [\"attention-optimizations.md\"]",
                    "---",
                    "",
                    "# Attention Mechanism",
                    "",
                    "Attention focuses computation on relevant tokens and links back to [[attention]].",
                    "",
                    "Efficient implementations reduce memory overhead and improve throughput.",
                ]
                .join("\n")
            } else if matches!(scenario, MockScenario::IngestWithPageMergeSuccess)
                && (request_index == 3 || request_index == 6)
            {
                String::new()
            } else if matches!(scenario, MockScenario::IngestWithDedicatedReviewStageSuccess) {
                [
                    "---REVIEW: suggestion | Compare attention variants---",
                    "Capture the tradeoffs between standard attention and efficient attention variants.",
                    "OPTIONS: Create Page | Skip",
                    "PAGES: wiki/concepts/attention-mechanism.md",
                    "SEARCH: efficient attention variants comparison | flash attention tradeoffs | linear attention benchmark",
                    "---END REVIEW---",
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

fn payload_has_image_block(payload: &Value) -> bool {
    payload
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|message| {
            message
                .get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|block| block.get("type").and_then(Value::as_str) == Some("image_url"))
        })
}

async fn embeddings(
    State(state): State<MockState>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let _ = state.request_count.fetch_add(1, Ordering::SeqCst);
    let text = payload
        .get("input")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let embedding = fake_embedding_for_text(text);
    (
        StatusCode::OK,
        Json(json!({
          "object": "list",
          "data": [
            {
              "object": "embedding",
              "index": 0,
              "embedding": embedding
            }
          ],
          "model": payload.get("model").and_then(Value::as_str).unwrap_or("mock-embedding"),
          "usage": {
            "prompt_tokens": 8,
            "total_tokens": 8
          }
        })),
    )
}

fn fake_embedding_for_text(text: &str) -> Vec<f32> {
    let lower = text.to_lowercase();
    if lower.contains("rope") {
        return vec![1.0, 0.0, 0.0];
    }
    if lower.contains("rotary position embeddings") || lower.contains("rotary positional embeddings") {
        return vec![1.0, 0.0, 0.0];
    }
    if lower.contains("attention") {
        return vec![0.0, 1.0, 0.0];
    }
    vec![0.0, 0.0, 1.0]
}
