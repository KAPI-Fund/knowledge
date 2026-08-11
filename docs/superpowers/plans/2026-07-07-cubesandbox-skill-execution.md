# CubeSandbox 技能执行架构 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 canvas `/ppt`(`LlmSkill`)的执行从进程内 `complete_text` 单次生成,切换到通过 Go sidecar 在 CubeSandbox 微虚拟机里跑 codex CLI 做 agentic 生成;干净删除 `deck_renderer` 旧路径。

**Architecture:** backend 侧新增极薄的 `SkillExecutor` trait(唯一实现 `CubeExecutor` = reqwest client),`skill_worker::execute()` 改调它;新增 Go `skill-runner` sidecar 用官方 CubeSandbox SDK 管沙箱生命周期并暴露 `POST /render`;新增 VM 模板镜像(烤 codex CLI + 技能文件 + 引导 PROMPT)。前端/DB schema/job 生命周期契约完全不变。

**Tech Stack:** Rust(axum/sqlx/reqwest/serde)、Go(net/http + CubeSandbox Go SDK)、Docker、codex CLI、CubeSandbox(RustVMM+KVM,E2B 兼容)。

**Spec:** `docs/superpowers/specs/2026-07-07-cubesandbox-skill-execution-design.md`

**关键约束:** CI/开发机无 KVM,单测绝不依赖真 CubeSandbox。真 VM 只在集成冒烟(手动,KVM 主机)验证。

---

## File Structure

**backend(Rust,crates/knowledge-server/src/)**
- `canvas/executor.rs` — **新增**。`SkillExecutor` trait、`RenderRequest`/`RenderProvider`/`RenderedDeck` DTO、`CubeExecutor`(reqwest 实现)、`MockExecutor`(cfg(test))。执行层的唯一新单元。
- `canvas/deck_renderer.rs` — **删除**。旧进程内单次生成。
- `canvas/skill_worker.rs` — **改**。`execute()` 改调 `state.executor.render()`;`LEASE_SECONDS` 30→900。
- `canvas/mod.rs` — **改**。`-pub mod deck_renderer;` `+pub mod executor;`。
- `app/state.rs` — **改**。`AppState` 加 `executor: Arc<dyn SkillExecutor>`。
- `config.rs` — **改**。加 `skill_runner_url`。
- `lib.rs` — **改**。`bootstrap_state` 构造 `CubeExecutor` 填进 `AppState`。
- `skills/descriptor.rs` — **改**。`LlmSkill` 文档注释语义更新。

**sidecar(Go,services/skill-runner/)**
- `main.go` — HTTP server,`POST /render` handler。
- `render.go` — 一次沙箱生命周期编排(create→write→run→read→kill)。
- `codexconfig.go` — `renderCodexConfig(provider)` 生成 config.toml。
- `codexconfig_test.go` / `render_test.go` — 纯 Go 单测(fake sandbox client)。
- `sandbox.go` — CubeSandbox client 接口 + 官方 SDK 适配 + fake(测试用)。
- `go.mod` / `Dockerfile` / `README.md`。

**VM 模板 & 脚本**
- `Dockerfile.skill-runner-vm` — 烤 codex CLI + 技能文件 + PROMPT。
- `services/skill-runner/vm/PROMPT.md` — 引导 codex 的固定指令(构建时 COPY 进镜像)。
- `scripts/skill-runner/build-template.sh` — 构建镜像 + `cubemastercli tpl create-from-image`。
- `docs/skill-runner/deploy.md` — KVM 主机部署 + 冒烟清单。

**任务依赖顺序:** Task 1(DTO+trait+Mock)→ Task 2(skill_worker 接线 + 删 deck_renderer)→ Task 3(AppState/config/lib 接线)→ Task 4(CubeExecutor reqwest 实现 + 契约测试)→ Task 5-8(Go sidecar)→ Task 9(VM 模板+脚本)→ Task 10(部署文档)→ Task 11(全量校验)。Backend(1-4)与 sidecar(5-8)相互独立,可并行,但都在 Task 11 汇合。

---

## Task 1: backend — SkillExecutor trait + DTO + MockExecutor

**Files:**
- Create: `crates/knowledge-server/src/canvas/executor.rs`
- Modify: `crates/knowledge-server/src/canvas/mod.rs`

- [ ] **Step 1: 在 mod.rs 注册新模块(暂不删 deck_renderer)**

Modify `crates/knowledge-server/src/canvas/mod.rs`,在现有 `pub mod deck_renderer;` 那行**之后**加一行:

```rust
pub mod executor;
```

（deck_renderer 的删除放到 Task 2,避免本任务中途编译断裂。）

- [ ] **Step 2: 写 executor.rs 的 DTO 与 trait(含失败测试)**

Create `crates/knowledge-server/src/canvas/executor.rs`:

```rust
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
        // camelCase 不强制:sidecar 侧用 snake_case 对齐(见 Go DTO)。
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
```

- [ ] **Step 3: 确保 async-trait 依赖存在**

Run: `cd E:/Projects/Js/knowledge && grep -n "async-trait" crates/knowledge-server/Cargo.toml`
Expected: 若无输出，则在 `crates/knowledge-server/Cargo.toml` 的 `[dependencies]` 加：`async-trait = "0.1"`（版本对齐 workspace 其它 crate；若 workspace 已声明则用 `async-trait.workspace = true`）。

- [ ] **Step 4: 编译 + 跑本模块测试**

Run: `cd E:/Projects/Js/knowledge && cargo test -p knowledge-server --lib canvas::executor`
Expected: PASS（两个 serde round-trip 测试通过）。

- [ ] **Step 5: Commit**

```bash
cd E:/Projects/Js/knowledge
git add crates/knowledge-server/src/canvas/executor.rs crates/knowledge-server/src/canvas/mod.rs crates/knowledge-server/Cargo.toml
git commit -m "feat(canvas): add SkillExecutor trait + render DTOs + MockExecutor"
```

---

## Task 2: backend — skill_worker 接线到 executor + 删除 deck_renderer

**Files:**
- Modify: `crates/knowledge-server/src/canvas/skill_worker.rs`
- Delete: `crates/knowledge-server/src/canvas/deck_renderer.rs`
- Modify: `crates/knowledge-server/src/canvas/mod.rs`
- Modify: `crates/knowledge-server/src/skills/descriptor.rs`

> 说明：本任务会让 `AppState.executor` 字段被引用，但该字段要到 Task 3 才加。为保证每步可编译，本任务先写 `skill_worker` 的**测试**(用 MockExecutor 直接测一个抽出的纯函数)，再改 `execute()`；`execute()` 依赖的 `state.executor` 字段在 Task 3 落地后整体编译才通过。因此本任务的 Step 4 只跑 `cargo check` 会**预期失败**(缺 executor 字段)，Task 3 Step 4 才会真正编译通过。这是有意的跨任务顺序，实现者照做即可。

- [ ] **Step 1: 把 execute() 拆出可测的纯组装 + 存储函数**

Modify `crates/knowledge-server/src/canvas/skill_worker.rs`。用下面整段替换现有 `execute` 函数(第 61-103 行)：

```rust
use crate::canvas::executor::{RenderProvider, RenderRequest};

/// 从 active 连接与 job 输入组装 RenderRequest(纯函数，可单测)。
fn build_render_request(
    skill_id: &str,
    selection: &str,
    argument: &str,
    active: &crate::providers::ActiveConnection,
) -> RenderRequest {
    RenderRequest {
        skill_id: skill_id.to_string(),
        selection: selection.to_string(),
        argument: argument.to_string(),
        provider: RenderProvider {
            base_url: active.base_url.clone(),
            api_key: active.api_key.clone(),
            model: active.model.clone(),
        },
    }
}

async fn execute(
    state: &crate::AppState,
    job: &skill_jobs::SkillJob,
) -> Result<serde_json::Value, String> {
    let descriptor = state
        .skill_registry
        .all()
        .iter()
        .find(|s| s.id == job.skill_id && matches!(s.runtime, SkillRuntime::LlmSkill))
        .cloned()
        .ok_or_else(|| format!("unknown llm skill: {}", job.skill_id))?;

    let selection = job.input.get("selection").and_then(|v| v.as_str()).unwrap_or_default();
    let argument = job.input.get("argument").and_then(|v| v.as_str()).unwrap_or_default();

    // 复用与 ingest 相同的 active 连接解析。
    let connections =
        crate::providers::list_connections(&state.pool).await.map_err(|e| e.to_string())?;
    let active =
        crate::providers::resolve_active(&connections).ok_or("no active LLM connection")?;
    let active = crate::providers::ActiveConnection::from(active);

    let req = build_render_request(&descriptor.id, selection, argument, &active);
    let rendered = state.executor.render(req).await?;

    // 把 deck.html 作为 job creator 拥有的 text/html asset 存下。
    let asset = crate::assets::store::NewAsset::new(
        &job.created_by,
        "text/html",
        rendered.deck_html.into_bytes(),
    );
    let asset_id =
        crate::assets::store::insert_asset(&state.pool, &asset).await.map_err(|e| e.to_string())?;
    let url = crate::assets::store::asset_url(&asset_id);

    Ok(serde_json::json!({
        "assetId": asset_id,
        "url": url,
        "title": "PPT",
    }))
}
```

- [ ] **Step 2: 调整 LEASE_SECONDS**

Modify `crates/knowledge-server/src/canvas/skill_worker.rs` 第 10 行：

```rust
const LEASE_SECONDS: i64 = 30;
```
改为：
```rust
// 一次 CubeSandbox+codex 渲染可达数分钟；租约须覆盖真实执行墙钟(> sidecar HTTP 660s)。
const LEASE_SECONDS: i64 = 900;
```

- [ ] **Step 3: 为 build_render_request 加单测**

在 `crates/knowledge-server/src/canvas/skill_worker.rs` 末尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_render_request_maps_active_connection() {
        let active = crate::providers::ActiveConnection {
            base_url: "https://api.x/v1".into(),
            api_key: "sk-9".into(),
            model: "gpt-5.4".into(),
            timeout_seconds: 30,
        };
        let req = build_render_request("guizang-ppt", "sel", "arg", &active);
        assert_eq!(req.skill_id, "guizang-ppt");
        assert_eq!(req.selection, "sel");
        assert_eq!(req.argument, "arg");
        assert_eq!(req.provider.base_url, "https://api.x/v1");
        assert_eq!(req.provider.api_key, "sk-9");
        assert_eq!(req.provider.model, "gpt-5.4");
    }
}
```

- [ ] **Step 4: 删除 deck_renderer 并摘掉模块声明**

```bash
cd E:/Projects/Js/knowledge
git rm crates/knowledge-server/src/canvas/deck_renderer.rs
```

Modify `crates/knowledge-server/src/canvas/mod.rs`：删除 `pub mod deck_renderer;` 那一行（保留 Task 1 加的 `pub mod executor;`）。

- [ ] **Step 5: 更新 LlmSkill 文档注释**

Modify `crates/knowledge-server/src/skills/descriptor.rs` 第 7-9 行，把：
```rust
    /// In-process bounded LLM loop (guizang-style): inline template + content,
    /// call complete_text, produce a single HTML file. No Node, no subprocess.
    LlmSkill,
```
改为：
```rust
    /// 异步技能：在 CubeSandbox 微虚拟机里由 codex CLI 做 agentic 生成，
    /// 产出单文件 HTML。经 canvas_skill_jobs 队列 + skill_worker + SkillExecutor 执行。
    LlmSkill,
```

- [ ] **Step 6: 验证（预期编译失败，缺 executor 字段——由 Task 3 修复）**

Run: `cd E:/Projects/Js/knowledge && cargo check -p knowledge-server 2>&1 | grep -i "executor\|error\[" | head`
Expected: 出现 `no field \`executor\` on type` 之类错误——**这是预期的**，Task 3 加字段后修复。不要在此任务里 commit（留到 Task 3 一起编译通过后提交，避免提交不可编译的中间态）。

> **实现者注意：** Task 2 与 Task 3 是一个不可分割的编译单元。按 subagent-driven 执行时，把 Task 2+Task 3 作为同一个 subagent 的连续工作交付，最后在 Task 3 Step 5 统一编译 + 提交。

---

## Task 3: backend — AppState / config / lib 接线

**Files:**
- Modify: `crates/knowledge-server/src/app/state.rs`
- Modify: `crates/knowledge-server/src/config.rs`
- Modify: `crates/knowledge-server/src/lib.rs`

- [ ] **Step 1: AppState 加 executor 字段**

Modify `crates/knowledge-server/src/app/state.rs`，整文件替换为：

```rust
use std::sync::Arc;

use sqlx::PgPool;

use crate::cache::CacheStore;
use crate::canvas::executor::SkillExecutor;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub cache: CacheStore,
    pub project_root: String,
    pub session_ttl_hours: u64,
    pub skill_registry: crate::skills::SkillRegistry,
    pub executor: Arc<dyn SkillExecutor>,
}
```

（`AppState` 仍是 `Clone`：`Arc<dyn SkillExecutor>` 是 `Clone`。）

- [ ] **Step 2: config 加 skill_runner_url**

Modify `crates/knowledge-server/src/config.rs`：

在 `AppConfig` struct 加字段（`admin_password` 之后）：
```rust
  pub skill_runner_url: String,
```

在 `for_tests` 的初始化里（`admin_password` 之后）加：
```rust
      skill_runner_url: "http://127.0.0.1:4600".to_string(),
```

在 `from_env` 里，`admin_password` 读取之后加：
```rust
    let skill_runner_url = std::env::var("KNOWLEDGE_SKILL_RUNNER_URL")
      .unwrap_or_else(|_| "http://127.0.0.1:4600".to_string());
```
并在 `Self { ... }` 初始化里（`admin_password` 之后）加 `skill_runner_url,`。

- [ ] **Step 3: lib.rs 构造 CubeExecutor 填进 AppState**

Modify `crates/knowledge-server/src/lib.rs` 的 `bootstrap_state`，把 `let state = AppState { ... };`（第 50-56 行）替换为：

```rust
    let executor: std::sync::Arc<dyn crate::canvas::executor::SkillExecutor> =
        std::sync::Arc::new(crate::canvas::executor::CubeExecutor::new(
            config.skill_runner_url.clone(),
        ));
    let state = AppState {
        pool,
        cache,
        project_root: config.project_root.clone(),
        session_ttl_hours: config.session_ttl_hours,
        skill_registry,
        executor,
    };
```

（`CubeExecutor::new` 在 Task 4 实现；本任务先引用，Task 4 补上后整体编译通过。若按 subagent 顺序执行，Task 4 与本任务同批交付。）

- [ ] **Step 4: 提供一个临时 stub 让 Task 1-3 可独立编译**

为让 Task 2+3 在 Task 4 之前就能编译通过（TDD 需要绿），在 `executor.rs` 里**先加一个最小的 `CubeExecutor` 骨架**（Task 4 再填真实 reqwest 逻辑）：

在 `crates/knowledge-server/src/canvas/executor.rs` 的 trait 定义之后、`#[cfg(test)]` 之前插入：

```rust
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

#[async_trait]
impl SkillExecutor for CubeExecutor {
    async fn render(&self, _req: RenderRequest) -> Result<RenderedDeck, String> {
        // Task 4 填真实实现。
        Err("skill runner not yet implemented".to_string())
    }
}
```

- [ ] **Step 5: 全量编译 + 跑 backend 库测试**

Run: `cd E:/Projects/Js/knowledge && cargo test -p knowledge-server --lib canvas`
Expected: 编译通过；`canvas::executor` 与 `skill_worker` 的测试 PASS。

Run: `cd E:/Projects/Js/knowledge && cargo check -p knowledge-server`
Expected: 干净编译（无 deck_renderer 残留引用）。

- [ ] **Step 6: Commit（Task 2+3 合并提交）**

```bash
cd E:/Projects/Js/knowledge
git add -A crates/knowledge-server
git commit -m "feat(canvas): route skill_worker through SkillExecutor; drop deck_renderer

- skill_worker.execute() 组 RenderRequest 调 executor,不再进程内 render_deck
- LEASE_SECONDS 30->900 覆盖沙箱执行墙钟
- AppState 加 executor 字段;config 加 skill_runner_url
- 删除 deck_renderer.rs;更新 LlmSkill 文档注释"
```

---

## Task 4: backend — CubeExecutor 真实实现 + 契约测试

**Files:**
- Modify: `crates/knowledge-server/src/canvas/executor.rs`

- [ ] **Step 1: 写针对 stub HTTP server 的契约测试(先失败)**

在 `crates/knowledge-server/src/canvas/executor.rs` 的 `#[cfg(test)] mod tests` 里追加。测试用 `tokio` + 一个极简 hyper/axum stub。项目已用 axum，测试里起一个临时 axum server：

```rust
    use axum::{routing::post, Json, Router};
    use std::net::SocketAddr;

    async fn spawn_stub<F>(handler: Router) -> String {
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
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd E:/Projects/Js/knowledge && cargo test -p knowledge-server --lib canvas::executor::tests::cube_executor 2>&1 | tail`
Expected: FAIL（当前 stub `render` 返回 `"skill runner not yet implemented"`）。

- [ ] **Step 3: 用真实实现替换 CubeExecutor::render**

在 `crates/knowledge-server/src/canvas/executor.rs` 里把 Task 3 Step 4 加的那段 `#[async_trait] impl SkillExecutor for CubeExecutor` 替换为：

```rust
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

        // 尽力从错误 body 提取 stage/message,拿不到就用状态码。
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
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd E:/Projects/Js/knowledge && cargo test -p knowledge-server --lib canvas::executor`
Expected: PASS（serde round-trip + 两个契约测试全绿）。

- [ ] **Step 5: 确认 axum 在 dev-dependencies 或 dependencies 可用于测试**

Run: `cd E:/Projects/Js/knowledge && cargo test -p knowledge-server --lib canvas::executor 2>&1 | tail -3`
Expected: 编译通过。若报 `axum` 不可用，确认 `crates/knowledge-server/Cargo.toml` 里 axum 已是普通依赖（它是 web 框架，必在 dependencies）——无需改。

- [ ] **Step 6: Commit**

```bash
cd E:/Projects/Js/knowledge
git add crates/knowledge-server/src/canvas/executor.rs
git commit -m "feat(canvas): implement CubeExecutor reqwest client + contract tests"
```

---

## Task 5: sidecar — Go 骨架 + codex config 生成(TDD)

**Files:**
- Create: `services/skill-runner/go.mod`
- Create: `services/skill-runner/codexconfig.go`
- Create: `services/skill-runner/codexconfig_test.go`

- [ ] **Step 1: 初始化 Go module**

```bash
cd E:/Projects/Js/knowledge && mkdir -p services/skill-runner
cd E:/Projects/Js/knowledge/services/skill-runner && go mod init github.com/knowledge/skill-runner
```

- [ ] **Step 2: 写 codexconfig 失败测试**

Create `services/skill-runner/codexconfig_test.go`:

```go
package main

import "testing"

func TestRenderCodexConfig(t *testing.T) {
	cfg := renderCodexConfig(Provider{
		BaseURL: "https://api.x/v1",
		Model:   "gpt-5.4",
	})
	for _, want := range []string{
		`model = "gpt-5.4"`,
		`model_provider = "knowledge"`,
		`base_url = "https://api.x/v1"`,
		`env_key = "PROVIDER_API_KEY"`,
		`wire_api = "chat"`,
		`requires_openai_auth = false`,
	} {
		if !contains(cfg, want) {
			t.Fatalf("config missing %q\n---\n%s", want, cfg)
		}
	}
	// api_key 绝不能写进 config.toml。
	if contains(cfg, "api_key") {
		t.Fatalf("api_key must not appear in config.toml:\n%s", cfg)
	}
}

func contains(haystack, needle string) bool {
	return len(haystack) >= len(needle) && (func() bool {
		for i := 0; i+len(needle) <= len(haystack); i++ {
			if haystack[i:i+len(needle)] == needle {
				return true
			}
		}
		return false
	})()
}
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cd E:/Projects/Js/knowledge/services/skill-runner && go test ./...`
Expected: FAIL（`renderCodexConfig`/`Provider` 未定义）。

- [ ] **Step 4: 实现 codexconfig.go**

Create `services/skill-runner/codexconfig.go`:

```go
package main

import "fmt"

// Provider 是 backend 注入、codex 要连的 OpenAI 兼容连接。
type Provider struct {
	BaseURL string `json:"base_url"`
	APIKey  string `json:"api_key"`
	Model   string `json:"model"`
}

// renderCodexConfig 生成写入沙箱 ~/.codex/config.toml 的内容。
// api_key 不写这里——它经 PROVIDER_API_KEY 环境变量注入(见 render.go)。
func renderCodexConfig(p Provider) string {
	return fmt.Sprintf(`model = "%s"
model_provider = "knowledge"

[model_providers.knowledge]
name = "knowledge"
base_url = "%s"
env_key = "PROVIDER_API_KEY"
wire_api = "chat"
requires_openai_auth = false
`, p.Model, p.BaseURL)
}
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cd E:/Projects/Js/knowledge/services/skill-runner && go test ./...`
Expected: PASS。

- [ ] **Step 6: Commit**

```bash
cd E:/Projects/Js/knowledge
git add services/skill-runner/go.mod services/skill-runner/codexconfig.go services/skill-runner/codexconfig_test.go
git commit -m "feat(skill-runner): codex config.toml generator + tests"
```

---

## Task 6: sidecar — sandbox client 接口 + fake

**Files:**
- Create: `services/skill-runner/sandbox.go`

- [ ] **Step 1: 定义与官方 SDK 解耦的最小接口 + fake**

Create `services/skill-runner/sandbox.go`:

```go
package main

import (
	"context"
	"fmt"
)

// CommandResult 是一次沙箱内命令执行的结果。
type CommandResult struct {
	ExitCode int
	Stdout   string
	Stderr   string
}

// Sandbox 是渲染流程需要的最小沙箱能力(便于 fake 测试;真实现包官方 SDK)。
type Sandbox interface {
	WriteFile(ctx context.Context, path, content string) error
	RunCommand(ctx context.Context, cmd string, env map[string]string) (CommandResult, error)
	ReadFile(ctx context.Context, path string) (string, error)
	Kill(ctx context.Context) error
}

// SandboxFactory 创建一个新沙箱(每次 render 一个)。
type SandboxFactory interface {
	Create(ctx context.Context) (Sandbox, error)
}

// --- 测试用 fake ---

type fakeSandbox struct {
	writes       map[string]string
	files        map[string]string // ReadFile 的返回内容
	runResult    CommandResult
	runErr       error
	killed       bool
	failReadPath string
}

func (f *fakeSandbox) WriteFile(_ context.Context, path, content string) error {
	if f.writes == nil {
		f.writes = map[string]string{}
	}
	f.writes[path] = content
	return nil
}

func (f *fakeSandbox) RunCommand(_ context.Context, _ string, _ map[string]string) (CommandResult, error) {
	return f.runResult, f.runErr
}

func (f *fakeSandbox) ReadFile(_ context.Context, path string) (string, error) {
	if path == f.failReadPath {
		return "", fmt.Errorf("no such file: %s", path)
	}
	if v, ok := f.files[path]; ok {
		return v, nil
	}
	return "", fmt.Errorf("no such file: %s", path)
}

func (f *fakeSandbox) Kill(_ context.Context) error {
	f.killed = true
	return nil
}

type fakeFactory struct {
	sb        *fakeSandbox
	createErr error
}

func (f *fakeFactory) Create(_ context.Context) (Sandbox, error) {
	if f.createErr != nil {
		return nil, f.createErr
	}
	return f.sb, nil
}
```

- [ ] **Step 2: 编译**

Run: `cd E:/Projects/Js/knowledge/services/skill-runner && go build ./...`
Expected: 编译通过（未使用的 fake 类型不报错，因为它们会被 Task 7 测试引用；若 `go vet`/build 因 unused 报错则忽略——Go 不对未使用的包级类型报错）。

- [ ] **Step 3: Commit**

```bash
cd E:/Projects/Js/knowledge
git add services/skill-runner/sandbox.go
git commit -m "feat(skill-runner): sandbox client interface + test fake"
```

---

## Task 7: sidecar — render 编排(TDD,四条分支)

**Files:**
- Create: `services/skill-runner/render.go`
- Create: `services/skill-runner/render_test.go`

- [ ] **Step 1: 写 render 编排失败测试(成功 + 三种失败,且都 kill)**

Create `services/skill-runner/render_test.go`:

```go
package main

import (
	"context"
	"errors"
	"testing"
)

func baseReq() RenderRequest {
	return RenderRequest{
		SkillID:   "guizang-ppt",
		Selection: "内容",
		Argument:  "swiss",
		Provider:  Provider{BaseURL: "https://api.x/v1", APIKey: "sk-1", Model: "gpt-5.4"},
	}
}

func TestRenderSuccess(t *testing.T) {
	sb := &fakeSandbox{
		runResult: CommandResult{ExitCode: 0},
		files:     map[string]string{"/work/out/deck.html": "<!DOCTYPE html><html>ok</html>"},
	}
	out, err := runRender(context.Background(), &fakeFactory{sb: sb}, "tpl-1", baseReq())
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if out.DeckHTML != "<!DOCTYPE html><html>ok</html>" {
		t.Fatalf("bad html: %q", out.DeckHTML)
	}
	if !sb.killed {
		t.Fatal("sandbox must be killed on success")
	}
	// 校验输入被写入 + config.toml + api_key 经 env 注入(不落 config)。
	if sb.writes["/work/input/selection.md"] != "内容" {
		t.Fatalf("selection not written: %v", sb.writes)
	}
	if !contains(sb.writes["/root/.codex/config.toml"], `base_url = "https://api.x/v1"`) {
		t.Fatalf("codex config not written: %v", sb.writes["/root/.codex/config.toml"])
	}
}

func TestRenderCodexNonZeroExit(t *testing.T) {
	sb := &fakeSandbox{runResult: CommandResult{ExitCode: 1, Stderr: "model refused"}}
	_, err := runRender(context.Background(), &fakeFactory{sb: sb}, "tpl-1", baseReq())
	if err == nil {
		t.Fatal("expected error on non-zero exit")
	}
	re := asRenderError(t, err)
	if re.Stage != "codex" {
		t.Fatalf("stage = %q, want codex", re.Stage)
	}
	if !sb.killed {
		t.Fatal("sandbox must be killed on codex failure")
	}
}

func TestRenderMissingOutput(t *testing.T) {
	sb := &fakeSandbox{
		runResult:    CommandResult{ExitCode: 0},
		failReadPath: "/work/out/deck.html",
	}
	_, err := runRender(context.Background(), &fakeFactory{sb: sb}, "tpl-1", baseReq())
	if err == nil {
		t.Fatal("expected error on missing output")
	}
	if asRenderError(t, err).Stage != "output" {
		t.Fatalf("stage = %q, want output", asRenderError(t, err).Stage)
	}
	if !sb.killed {
		t.Fatal("sandbox must be killed on missing output")
	}
}

func TestRenderCreateFailure(t *testing.T) {
	_, err := runRender(context.Background(), &fakeFactory{createErr: errors.New("kvm down")}, "tpl-1", baseReq())
	if err == nil {
		t.Fatal("expected error on create failure")
	}
	if asRenderError(t, err).Stage != "create" {
		t.Fatalf("stage = %q, want create", asRenderError(t, err).Stage)
	}
}

func asRenderError(t *testing.T, err error) *RenderError {
	t.Helper()
	var re *RenderError
	if !errors.As(err, &re) {
		t.Fatalf("error is not *RenderError: %v", err)
	}
	return re
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd E:/Projects/Js/knowledge/services/skill-runner && go test ./...`
Expected: FAIL（`runRender`/`RenderRequest`/`RenderError`/`asRenderError` 未定义）。

- [ ] **Step 3: 实现 render.go**

Create `services/skill-runner/render.go`:

```go
package main

import (
	"context"
	"fmt"
	"strings"
)

const (
	skillsRoot     = "/skills"
	inputSelection = "/work/input/selection.md"
	inputArgument  = "/work/input/argument.txt"
	codexConfig    = "/root/.codex/config.toml"
	promptPath     = "/work/PROMPT.md"
	outputPath     = "/work/out/deck.html"
)

// RenderRequest 是 POST /render 的 body(与 backend 的 serde snake_case 对齐)。
type RenderRequest struct {
	SkillID   string   `json:"skill_id"`
	Selection string   `json:"selection"`
	Argument  string   `json:"argument"`
	Provider  Provider `json:"provider"`
}

// RenderedDeck 是成功响应。
type RenderedDeck struct {
	DeckHTML string `json:"deck_html"`
}

// RenderError 带阶段标签,map 到 HTTP + backend 的 job error。
type RenderError struct {
	Stage   string // create | codex | timeout | output
	Message string
}

func (e *RenderError) Error() string {
	return fmt.Sprintf("%s: %s", e.Stage, e.Message)
}

// runRender 编排一次沙箱生命周期。无论成功失败都 kill(create 失败除外——没有沙箱)。
func runRender(ctx context.Context, factory SandboxFactory, templateID string, req RenderRequest) (RenderedDeck, error) {
	_ = templateID // 真实 factory 用它 Create;fake 忽略。
	sb, err := factory.Create(ctx)
	if err != nil {
		return RenderedDeck{}, &RenderError{Stage: "create", Message: err.Error()}
	}
	defer sb.Kill(context.Background())

	if err := sb.WriteFile(ctx, inputSelection, req.Selection); err != nil {
		return RenderedDeck{}, &RenderError{Stage: "create", Message: "write selection: " + err.Error()}
	}
	if err := sb.WriteFile(ctx, inputArgument, req.Argument); err != nil {
		return RenderedDeck{}, &RenderError{Stage: "create", Message: "write argument: " + err.Error()}
	}
	if err := sb.WriteFile(ctx, codexConfig, renderCodexConfig(req.Provider)); err != nil {
		return RenderedDeck{}, &RenderError{Stage: "create", Message: "write codex config: " + err.Error()}
	}

	cmd := fmt.Sprintf(
		`codex exec --skip-git-repo-check --dangerously-bypass-approvals-and-sandbox --cd %s/%s "$(cat %s)"`,
		skillsRoot, req.SkillID, promptPath,
	)
	res, err := sb.RunCommand(ctx, cmd, map[string]string{"PROVIDER_API_KEY": req.Provider.APIKey})
	if err != nil {
		// 上下文取消/超时也走这
		if ctx.Err() == context.DeadlineExceeded {
			return RenderedDeck{}, &RenderError{Stage: "timeout", Message: "codex render timed out"}
		}
		return RenderedDeck{}, &RenderError{Stage: "codex", Message: err.Error()}
	}
	if res.ExitCode != 0 {
		return RenderedDeck{}, &RenderError{Stage: "codex", Message: summarize(res.Stderr)}
	}

	html, err := sb.ReadFile(ctx, outputPath)
	if err != nil {
		return RenderedDeck{}, &RenderError{Stage: "output", Message: "read output: " + err.Error()}
	}
	if strings.TrimSpace(html) == "" {
		return RenderedDeck{}, &RenderError{Stage: "output", Message: "codex produced empty output"}
	}
	return RenderedDeck{DeckHTML: html}, nil
}

// summarize 截断长 stderr,避免 job error 塞爆。
func summarize(s string) string {
	const max = 2000
	s = strings.TrimSpace(s)
	if len(s) > max {
		return s[:max] + "…(truncated)"
	}
	if s == "" {
		return "codex exited non-zero with no stderr"
	}
	return s
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd E:/Projects/Js/knowledge/services/skill-runner && go test ./...`
Expected: PASS（四条分支全绿，含 kill 断言）。

- [ ] **Step 5: Commit**

```bash
cd E:/Projects/Js/knowledge
git add services/skill-runner/render.go services/skill-runner/render_test.go
git commit -m "feat(skill-runner): render orchestration with staged errors + always-kill"
```

---

## Task 8: sidecar — HTTP server + 真实 CubeSandbox factory + Dockerfile

**Files:**
- Create: `services/skill-runner/main.go`
- Create: `services/skill-runner/cube.go`
- Create: `services/skill-runner/Dockerfile`
- Create: `services/skill-runner/README.md`

- [ ] **Step 1: HTTP server(main.go)**

Create `services/skill-runner/main.go`:

```go
package main

import (
	"context"
	"encoding/json"
	"log"
	"net/http"
	"os"
	"time"
)

// renderTimeout 是 codex 整个 agentic 生成的墙钟上限(见 spec 第4节)。
const renderTimeout = 600 * time.Second

type server struct {
	factory    SandboxFactory
	templateID string
}

func (s *server) handleRender(w http.ResponseWriter, r *http.Request) {
	var req RenderRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeErr(w, http.StatusBadRequest, "create", "bad request body: "+err.Error())
		return
	}
	if req.SkillID == "" {
		writeErr(w, http.StatusBadRequest, "create", "skill_id required")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), renderTimeout)
	defer cancel()

	start := time.Now()
	out, err := runRender(ctx, s.factory, s.templateID, req)
	if err != nil {
		re, ok := err.(*RenderError)
		stage, msg := "codex", err.Error()
		if ok {
			stage, msg = re.Stage, re.Message
		}
		status := http.StatusInternalServerError
		if stage == "timeout" {
			status = http.StatusGatewayTimeout
		}
		log.Printf("render failed skill=%s stage=%s dur=%s: %s", req.SkillID, stage, time.Since(start), msg)
		writeErr(w, status, stage, msg)
		return
	}
	log.Printf("render ok skill=%s dur=%s bytes=%d", req.SkillID, time.Since(start), len(out.DeckHTML))
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(out)
}

func writeErr(w http.ResponseWriter, status int, stage, message string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(map[string]string{"stage": stage, "message": message})
}

func (s *server) handleHealth(w http.ResponseWriter, _ *http.Request) {
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write([]byte("ok"))
}

func main() {
	addr := envOr("SKILL_RUNNER_ADDR", ":4600")
	templateID := os.Getenv("CUBE_TEMPLATE_ID")
	if templateID == "" {
		log.Fatal("CUBE_TEMPLATE_ID is required")
	}
	factory, err := newCubeFactory(templateID)
	if err != nil {
		log.Fatalf("init cube factory: %v", err)
	}
	if v := os.Getenv("SKILLS_VERSION"); v != "" {
		log.Printf("skill-runner starting; skills version=%s template=%s", v, templateID)
	}
	s := &server{factory: factory, templateID: templateID}
	mux := http.NewServeMux()
	mux.HandleFunc("/render", s.handleRender)
	mux.HandleFunc("/health", s.handleHealth)
	log.Printf("skill-runner listening on %s", addr)
	log.Fatal(http.ListenAndServe(addr, mux))
}

func envOr(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}
```

- [ ] **Step 2: 真实 CubeSandbox factory(cube.go)**

Create `services/skill-runner/cube.go`。**实现时对照 `pkg.go.dev/github.com/tencentcloud/CubeSandbox/sdk/go` 的确切签名**(`NewClient`/`NewConfigFromEnv`/`Create`/`Commands().Run`/`Files`/`Kill`),下面是按调研到的 SDK 形状写的适配层，方法名以实际 SDK 为准微调：

```go
package main

import (
	"context"
	"fmt"

	cubesandbox "github.com/tencentcloud/CubeSandbox/sdk/go"
)

// cubeFactory 用官方 SDK 每次 Create 一个真实微 VM。
type cubeFactory struct {
	client     *cubesandbox.Client
	templateID string
}

func newCubeFactory(templateID string) (SandboxFactory, error) {
	// CUBE_API_URL / CUBE_API_KEY(或 E2B_ 变量)从环境读。
	client := cubesandbox.NewClient(cubesandbox.NewConfigFromEnv())
	return &cubeFactory{client: client, templateID: templateID}, nil
}

func (f *cubeFactory) Create(ctx context.Context) (Sandbox, error) {
	sb, err := f.client.Create(ctx, cubesandbox.CreateOptions{TemplateID: f.templateID})
	if err != nil {
		return nil, err
	}
	return &cubeSandbox{sb: sb}, nil
}

type cubeSandbox struct {
	sb *cubesandbox.Sandbox
}

func (c *cubeSandbox) WriteFile(ctx context.Context, path, content string) error {
	return c.sb.Files().Write(ctx, path, []byte(content))
}

func (c *cubeSandbox) RunCommand(ctx context.Context, cmd string, env map[string]string) (CommandResult, error) {
	res, err := c.sb.Commands().Run(ctx, cmd, cubesandbox.CommandOptions{Env: env})
	if err != nil {
		return CommandResult{}, err
	}
	return CommandResult{ExitCode: res.ExitCode, Stdout: res.Stdout, Stderr: res.Stderr}, nil
}

func (c *cubeSandbox) ReadFile(ctx context.Context, path string) (string, error) {
	b, err := c.sb.Files().Read(ctx, path)
	if err != nil {
		return "", err
	}
	return string(b), nil
}

func (c *cubeSandbox) Kill(ctx context.Context) error {
	return c.sb.Kill(ctx)
}

var _ = fmt.Sprintf // 保底避免 import 未用(实现细化后可删)
```

- [ ] **Step 3: go build(SDK 依赖会在此拉取)**

Run: `cd E:/Projects/Js/knowledge/services/skill-runner && go get github.com/tencentcloud/CubeSandbox/sdk/go && go build ./...`
Expected: 编译通过。**若 SDK 实际方法签名与上面不同,以编译器 + pkg.go.dev 为准调整 cube.go(只改这一个文件,接口 sandbox.go 不动)。** 这是唯一需要对着真 SDK 校准的地方。

- [ ] **Step 4: 单测仍绿(不碰真 SDK)**

Run: `cd E:/Projects/Js/knowledge/services/skill-runner && go test ./...`
Expected: PASS（codexconfig + render 测试用 fake，不受 cube.go 影响）。

- [ ] **Step 5: sidecar 自身 Dockerfile**

Create `services/skill-runner/Dockerfile`:

```dockerfile
# skill-runner sidecar:跑在 KVM 主机上,和 CubeSandbox 同网。
FROM golang:1.23 AS build
WORKDIR /src
COPY go.mod ./
RUN go mod download || true
COPY . .
RUN CGO_ENABLED=0 go build -o /skill-runner ./...

FROM gcr.io/distroless/static-debian12
COPY --from=build /skill-runner /skill-runner
EXPOSE 4600
ENTRYPOINT ["/skill-runner"]
```

- [ ] **Step 6: README**

Create `services/skill-runner/README.md`，写明：用途(backend↔CubeSandbox 之间的执行 sidecar)、必需环境变量(`CUBE_TEMPLATE_ID`、`CUBE_API_URL`、`CUBE_API_KEY`、可选 `SKILL_RUNNER_ADDR`、`SKILLS_VERSION`)、`POST /render` 契约(RenderRequest→RenderedDeck / 错误 body `{stage,message}`)、本地 `go test ./...`、构建镜像。指向 `docs/skill-runner/deploy.md`。

- [ ] **Step 7: Commit**

```bash
cd E:/Projects/Js/knowledge
git add services/skill-runner/main.go services/skill-runner/cube.go services/skill-runner/Dockerfile services/skill-runner/README.md services/skill-runner/go.mod services/skill-runner/go.sum
git commit -m "feat(skill-runner): HTTP server + CubeSandbox factory + Dockerfile + README"
```

---

## Task 9: VM 模板镜像 + 构建脚本

**Files:**
- Create: `Dockerfile.skill-runner-vm`
- Create: `services/skill-runner/vm/PROMPT.md`
- Create: `scripts/skill-runner/build-template.sh`

- [ ] **Step 1: 引导 PROMPT.md**

Create `services/skill-runner/vm/PROMPT.md`:

```
你是幻灯片生成 agent。当前工作目录是某个技能目录(如 guizang-ppt),
其中有 SKILL.md、模板文件与 references/ 配套规范。请严格按以下步骤:

1. 读 SKILL.md 与 template-swiss.html(若不存在则 template.html),理解风格、
   版式与结构约束;需要时查阅 references/ 下的规范文件。
2. 读 /work/input/selection.md(要做成幻灯片的源内容)与
   /work/input/argument.txt(额外要求,可能为空)。
3. 依据 SKILL.md 的规则,把源内容生成为一份自包含的单文件 HTML 幻灯片:
   内联所有 CSS/JS,不引用任何外部资源。
4. 把最终 HTML 写入 /work/out/deck.html,文件必须以 <!DOCTYPE html> 开头。
   不要把 HTML 输出到 stdout。完成后结束。
```

- [ ] **Step 2: VM 模板 Dockerfile**

Create `Dockerfile.skill-runner-vm`:

```dockerfile
# CubeSandbox 模板镜像:烤 codex CLI + 技能文件 + 引导 PROMPT。
# 由 scripts/skill-runner/build-template.sh 构建并 cubemastercli tpl create-from-image。
# 基础镜像用 CubeSandbox 官方 sandbox-code(含 envd);tag 以实际可拉版本为准。
FROM cube-sandbox-int.tencentcloudcr.com/cube-sandbox/sandbox-code:latest

# 1) 安装 codex CLI(以官方安装方式为准;此处示意 npm 全局安装)。
RUN npm install -g @openai/codex || true

# 2) 烤入技能文件(单一真相源:仓库 skills/)。
COPY crates/knowledge-server/skills/guizang-ppt /skills/guizang-ppt

# 3) 引导 prompt 与工作目录骨架。
COPY services/skill-runner/vm/PROMPT.md /work/PROMPT.md
RUN mkdir -p /work/input /work/out /root/.codex

# 4) 版本戳(构建脚本用 --build-arg 传入技能目录 git 短哈希)。
ARG SKILLS_VERSION=unknown
RUN echo "$SKILLS_VERSION" > /skills/VERSION
```

- [ ] **Step 3: 构建脚本**

Create `scripts/skill-runner/build-template.sh`:

```bash
#!/usr/bin/env bash
# 在 KVM 主机上构建 skill-runner VM 模板镜像并注册为 CubeSandbox 模板。
# 用法: REGISTRY=<registry> TAG=<tag> ./scripts/skill-runner/build-template.sh
set -euo pipefail

REGISTRY="${REGISTRY:?set REGISTRY, e.g. myreg.example.com/knowledge}"
TAG="${TAG:-$(date +%Y%m%d-%H%M%S)}"
IMAGE="$REGISTRY/skill-runner-vm:$TAG"

# 技能目录的 git 短哈希,用于漂移核对。
SKILLS_VERSION="$(git rev-parse --short HEAD)"

echo ">> building $IMAGE (skills=$SKILLS_VERSION)"
docker build \
  -f Dockerfile.skill-runner-vm \
  --build-arg "SKILLS_VERSION=$SKILLS_VERSION" \
  -t "$IMAGE" .

echo ">> pushing $IMAGE"
docker push "$IMAGE"

echo ">> registering CubeSandbox template"
# --expose-port/--probe 端口按 CubeSandbox 部署实际调整。
cubemastercli tpl create-from-image \
  --image "$IMAGE" \
  --writable-layer-size 1G \
  --expose-port 49999 \
  --probe 49999

echo ">> done. 取输出里的 template_id,填进 skill-runner 的 CUBE_TEMPLATE_ID。"
```

- [ ] **Step 4: 标记脚本可执行 + 语法自检**

Run: `cd E:/Projects/Js/knowledge && bash -n scripts/skill-runner/build-template.sh && echo "syntax ok"`
Expected: `syntax ok`。

（此任务无自动化单测——镜像构建需 KVM 主机,归入 Task 11 的手动冒烟。此步只做 shell 语法检查。）

- [ ] **Step 5: Commit**

```bash
cd E:/Projects/Js/knowledge
git add Dockerfile.skill-runner-vm services/skill-runner/vm/PROMPT.md scripts/skill-runner/build-template.sh
git commit -m "feat(skill-runner): VM template image + PROMPT + build-template script"
```

---

## Task 10: docker-compose 接线 + 部署文档

**Files:**
- Modify: `docker-compose.yml`
- Create: `docs/skill-runner/deploy.md`

- [ ] **Step 1: backend 环境变量指向 skill-runner**

Modify `docker-compose.yml` 的 `backend.environment`,在末尾加一行(sidecar 跑在 KVM 主机上,值由部署者填真实地址;compose 里给占位默认):

```yaml
      # skill-runner sidecar(跑在带 KVM 的 Linux 主机上,执行 CubeSandbox+codex)。
      # 本地无 KVM 时 /ppt 会失败,属预期;配置真实地址后生效。
      KNOWLEDGE_SKILL_RUNNER_URL: ${KNOWLEDGE_SKILL_RUNNER_URL:-http://host.docker.internal:4600}
```

（不把 skill-runner 加进本 compose 的 services——它必须在 KVM 主机上跑,不在开发机 Docker 里。这点在部署文档说明。）

- [ ] **Step 2: 部署文档**

Create `docs/skill-runner/deploy.md`,内容涵盖：

1. **前置**:x86_64 Linux + KVM 主机;装 CubeSandbox(指向官方 quickstart);装 Docker。
2. **烤模板**:`REGISTRY=... TAG=... ./scripts/skill-runner/build-template.sh` → 记下 `template_id`。
3. **起 sidecar**:
   ```bash
   docker build -f services/skill-runner/Dockerfile -t skill-runner:latest services/skill-runner
   docker run -d --name skill-runner -p 4600:4600 \
     -e CUBE_TEMPLATE_ID=<template_id> \
     -e CUBE_API_URL=http://127.0.0.1:3000 \
     -e CUBE_API_KEY=dummy \
     skill-runner:latest
   ```
4. **backend 指向它**:设 `KNOWLEDGE_SKILL_RUNNER_URL=http://<kvm-host>:4600`。
5. **冒烟清单**(手动集成验证,对应 spec 5.1-C):
   - `curl -s localhost:4600/health` → `ok`。
   - `curl -X POST localhost:4600/render -H 'content-type: application/json' -d '{"skill_id":"guizang-ppt","selection":"# 测试\n三点内容","argument":"","provider":{"base_url":"<你的>/v1","api_key":"<你的>","model":"<你的>"}}'` → 返回 `{"deck_html":"<!DOCTYPE html>..."}`。
   - 前端跑真 `/ppt`:选内容 → `/ppt` → 轮询 job → 得到 HTML asset。
   - 失败演练:停掉 sidecar → `/ppt` job 报 `skill runner unavailable`。
6. **版本核对**:`docker logs skill-runner | grep "skills version"` 对上仓库 `git rev-parse --short HEAD`。

- [ ] **Step 3: 校验 compose 语法**

Run: `cd E:/Projects/Js/knowledge && docker compose config >/dev/null 2>&1 && echo "compose ok" || docker compose config 2>&1 | tail -5`
Expected: `compose ok`（若本机无 docker,跳过,不阻塞——部署文档已给出手动步骤）。

- [ ] **Step 4: Commit**

```bash
cd E:/Projects/Js/knowledge
git add docker-compose.yml docs/skill-runner/deploy.md
git commit -m "feat(skill-runner): wire backend env + KVM host deploy guide"
```

---

## Task 11: 全量校验

- [ ] **Step 1: backend 全量**

Run: `cd E:/Projects/Js/knowledge && cargo test -p knowledge-server --lib 2>&1 | tail -15`
Expected: 全绿。特别确认 `canvas::executor` 与 `skill_worker` 测试通过,且无 deck_renderer 残留引用。

Run: `cd E:/Projects/Js/knowledge && cargo check -p knowledge-server 2>&1 | tail -5`
Expected: 干净编译。

- [ ] **Step 2: sidecar 全量**

Run: `cd E:/Projects/Js/knowledge/services/skill-runner && go test ./... && go vet ./...`
Expected: 测试全绿,vet 无警告。

- [ ] **Step 3: 确认删除干净**

Run: `cd E:/Projects/Js/knowledge && grep -rn "deck_renderer\|render_deck\|build_system_prompt" crates/knowledge-server/src --include=*.rs`
Expected: 无输出（旧路径彻底移除）。

Run: `cd E:/Projects/Js/knowledge && grep -rn "complete_text" crates/knowledge-server/src/canvas --include=*.rs`
Expected: 无输出（canvas 下不再有进程内 LLM 单次生成；analyze 的 complete_text 在 routes.rs 是另一条 builtin 路径,若仍需保留则确认它属于 analyze 而非 skill 渲染——按现状 routes.rs:357 属 analyze builtin,保留)。

> 注:若 Step 3 第二条在 `routes.rs` 仍有命中,核对它是 `analyze` builtin 的一次性 chat（保留,不属本次删除范围）。

- [ ] **Step 4: 手动集成冒烟(需 KVM 主机,不进 CI)**

按 `docs/skill-runner/deploy.md` 的冒烟清单在 KVM 主机执行,逐条勾选：健康检查、直接 `curl /render`、前端真 `/ppt` 端到端、sidecar 宕机失败演练、版本核对。

- [ ] **Step 5: 最终提交(若前述有零散改动)**

```bash
cd E:/Projects/Js/knowledge
git add -A && git commit -m "chore(skill-runner): final verification fixups" || echo "nothing to commit"
```

---

## Self-Review 结论(计划作者已核)

- **Spec 覆盖**:第1节架构→Task1/3;第2节数据流→Task2(backend)/Task7(sidecar编排)/Task5(codex config);第3节模板镜像→Task9;第4节超时/失败→Task2(LEASE)/Task4(错误映射)/Task7(staged errors)/Task8(HTTP status);第5节测试/文件→各任务 TDD + Task11。全部有对应任务。
- **占位符**:`<registry>`/`<tag>`/`<template_id>`/`--expose-port` 为部署期真值,已在脚本/文档显式标注来源;codex flag 与 CubeSandbox SDK 方法名标注"以实际版本为准"并限定只改 cube.go 一处。非遗漏。
- **类型一致**:`RenderRequest`/`RenderProvider`/`RenderedDeck`(Rust)与 `RenderRequest`/`Provider`/`RenderedDeck`(Go)字段用 snake_case JSON 对齐(`skill_id`/`base_url`/`deck_html`),两侧测试都断言了 tag 名。`SkillExecutor.render` 签名在 Task1 定义、Task2 调用、Task3/4 实现一致。
- **编译顺序**:Task2 单独 `cargo check` 预期失败已显式说明,与 Task3 合并提交;这是唯一的跨任务编译耦合,已在两处标注。
