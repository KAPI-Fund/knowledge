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
