use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// backend → sidecar 的 provider 注入块(codex 用它连后台已配连接)。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenderProvider {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

/// backend → sidecar 的一次渲染请求(POST /render 的 body)。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenderRequest {
    pub skill_id: String,
    pub selection: String,
    pub argument: String,
    pub provider: RenderProvider,
}

/// sidecar → backend 的渲染结果(POST /render 的成功 body)。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenderedDeck {
    pub deck_html: String,
}

/// 执行一次技能渲染。唯一生产实现是 CubeExecutor(调 sidecar)。
/// 返回的 String 是给 job error 用的人类可读失败信息。
#[async_trait]
pub trait SkillExecutor: Send + Sync {
    async fn render(&self, req: RenderRequest) -> Result<RenderedDeck, String>;
}

/// 调 skill-runner sidecar 的 POST /render 的生产实现。
pub struct CubeExecutor {
    base_url: String,
    client: reqwest::Client,
}

impl CubeExecutor {
    pub fn new(base_url: String) -> Self {
        // HTTP 总超时须略大于 sidecar 的 RENDER_TIMEOUT(600s)，取 660s。
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(660))
            .build()
            .expect("build skill-runner http client");
        Self { base_url: base_url.trim_end_matches('/').to_string(), client }
    }
}

/// sidecar 失败时的结构化错误 body 形状（尽力解析，失败则用状态码）。
#[derive(Debug, Deserialize)]
struct RunnerError {
    #[serde(default)]
    stage: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[async_trait]
impl SkillExecutor for CubeExecutor {
    async fn render(&self, req: RenderRequest) -> Result<RenderedDeck, String> {
        let url = format!("{}/render", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("skill runner unavailable: {e}"))?;

        let status = resp.status();
        if status.is_success() {
            return resp
                .json::<RenderedDeck>()
                .await
                .map_err(|e| format!("skill runner returned malformed body: {e}"));
        }

        let body = resp.text().await.unwrap_or_default();
        let detail = serde_json::from_str::<RunnerError>(&body)
            .ok()
            .map(|e| {
                let stage = e.stage.unwrap_or_else(|| "unknown".into());
                let msg = e.message.unwrap_or_else(|| body.clone());
                format!("{stage}: {msg}")
            })
            .unwrap_or_else(|| format!("HTTP {status}: {body}"));
        Err(format!("skill render failed ({detail})"))
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Arc;

    /// 测试用:预置一个结果,记录收到的请求。
    #[derive(Clone)]
    pub struct MockExecutor {
        pub result: Arc<Result<RenderedDeck, String>>,
        pub seen: Arc<std::sync::Mutex<Option<RenderRequest>>>,
    }

    impl MockExecutor {
        pub fn ok(html: &str) -> Self {
            Self {
                result: Arc::new(Ok(RenderedDeck { deck_html: html.to_string() })),
                seen: Arc::new(std::sync::Mutex::new(None)),
            }
        }
        pub fn err(message: &str) -> Self {
            Self {
                result: Arc::new(Err(message.to_string())),
                seen: Arc::new(std::sync::Mutex::new(None)),
            }
        }
    }

    #[async_trait]
    impl SkillExecutor for MockExecutor {
        async fn render(&self, req: RenderRequest) -> Result<RenderedDeck, String> {
            *self.seen.lock().unwrap() = Some(req);
            (*self.result).clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::post, Json, Router};

    async fn spawn_stub(handler: Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, handler).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn cube_executor_posts_and_parses_rendered_deck() {
        let app = Router::new().route(
            "/render",
            post(|Json(req): Json<RenderRequest>| async move {
                assert_eq!(req.skill_id, "guizang-ppt");
                Json(RenderedDeck { deck_html: "<!DOCTYPE html><html>ok</html>".into() })
            }),
        );
        let base = spawn_stub(app).await;
        let exec = CubeExecutor::new(base);
        let req = RenderRequest {
            skill_id: "guizang-ppt".into(),
            selection: "s".into(),
            argument: "".into(),
            provider: RenderProvider {
                base_url: "https://api.x/v1".into(),
                api_key: "sk".into(),
                model: "m".into(),
            },
        };
        let out = exec.render(req).await.unwrap();
        assert!(out.deck_html.contains("ok"));
    }

    #[tokio::test]
    async fn cube_executor_maps_5xx_to_error_string() {
        let app = Router::new().route(
            "/render",
            post(|| async {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "stage": "codex", "message": "boom" })),
                )
            }),
        );
        let base = spawn_stub(app).await;
        let exec = CubeExecutor::new(base);
        let req = RenderRequest {
            skill_id: "guizang-ppt".into(),
            selection: "s".into(),
            argument: "".into(),
            provider: RenderProvider { base_url: "u".into(), api_key: "k".into(), model: "m".into() },
        };
        let err = exec.render(req).await.unwrap_err();
        assert!(err.contains("codex") || err.contains("boom"), "got: {err}");
    }

    #[test]
    fn render_request_serde_round_trips() {
        let req = RenderRequest {
            skill_id: "guizang-ppt".into(),
            selection: "内容".into(),
            argument: "swiss".into(),
            provider: RenderProvider {
                base_url: "https://api.x/v1".into(),
                api_key: "sk-1".into(),
                model: "gpt-5.4".into(),
            },
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: RenderRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
        assert!(json.contains("\"skill_id\""));
        assert!(json.contains("\"base_url\""));
    }

    #[test]
    fn rendered_deck_serde_round_trips() {
        let d = RenderedDeck { deck_html: "<!DOCTYPE html>".into() };
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("\"deck_html\""));
        let back: RenderedDeck = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }
}
