# Canvas Node Dataflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify canvas edges into a directional dataflow — a left (upstream/producer) node's output feeds the right (downstream/consumer) node's input; clicking a consumer's Run processes all upstream content with that consumer's own prompt.

**Architecture:** Reuse the existing `edge.source → edge.target` convention and the `incoming_source_ids` mechanism (today only `ai_analyze` uses it). Generalise it to every consumer (`search`, `ai_analyze`, `ai_image`), fill the missing upstream reference blocks (`search`, `ai_image`), unify the Run execution path (drop the separate REST `/api/canvas/search`), add medium-strength connection validation, and order inputs by spatial position (y then x). Edges live in the `CanvasDocument` JSON blob (single `document` text column) — **no SQL migration**.

**Tech Stack:** Rust / axum / sqlx (`crates/knowledge-server/src/canvas`) + React 19 / React Flow (`@xyflow/react`) / Vite (`apps/admin/src/features/canvas`).

**Spec:** `docs/superpowers/specs/2026-07-06-canvas-node-dataflow-design.md`

**Verification commands:**
- Backend: from `E:\Projects\Js\knowledge` → `cargo test -p knowledge-server --lib`
- Frontend: from `apps/admin` → `npx vitest run src/features/canvas` and `npx tsc --noEmit`

---

## Task 1: Backend `CanvasEdge` optional handle/kind fields

**Files:**
- Modify: `crates/knowledge-server/src/canvas/document.rs`
- Modify: `crates/knowledge-server/src/canvas/service.rs` (test helper only)

Reserve `source_handle` / `target_handle` / `kind` on the edge for future multi-port / typed connections. Runtime ignores them; they only need to serialize when present and be omitted when absent. Derive `Default` so struct literals can use `..Default::default()`.

- [ ] **Step 1: Write the failing roundtrip test**

Add to the `tests` module in `document.rs`:

```rust
    #[test]
    fn edge_handles_roundtrip_and_omit_when_absent() {
        // Absent optional fields must not appear in the JSON.
        let bare = CanvasEdge {
            id: "e1".into(),
            source: "a".into(),
            target: "b".into(),
            ..Default::default()
        };
        let json = serde_json::to_string(&bare).unwrap();
        assert!(!json.contains("sourceHandle"));
        assert!(!json.contains("kind"));

        // Present optional fields roundtrip through camelCase keys.
        let full = CanvasEdge {
            id: "e2".into(),
            source: "a".into(),
            target: "b".into(),
            source_handle: Some("out".into()),
            target_handle: Some("in".into()),
            kind: Some("text".into()),
        };
        let json = serde_json::to_string(&full).unwrap();
        assert!(json.contains("\"sourceHandle\":\"out\""));
        assert!(json.contains("\"targetHandle\":\"in\""));
        assert!(json.contains("\"kind\":\"text\""));
        let back: CanvasEdge = serde_json::from_str(&json).unwrap();
        assert_eq!(back, full);
    }
```

- [ ] **Step 2: Run it — expect fail**

Run: `cargo test -p knowledge-server --lib edge_handles_roundtrip`
Expected: FAIL — `CanvasEdge` has no field `source_handle`, and no `Default` impl.

- [ ] **Step 3: Add the fields + Default derive**

Replace the `CanvasEdge` struct in `document.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CanvasEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_handle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_handle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}
```

- [ ] **Step 4: Fix existing struct literals**

In `document.rs` the `incoming_source_ids_...` test builds three edges. Add `..Default::default()` to each:

```rust
            edges: vec![
                CanvasEdge { id: "e1".into(), source: "a".into(), target: "c".into(), ..Default::default() },
                CanvasEdge { id: "e2".into(), source: "b".into(), target: "c".into(), ..Default::default() },
                CanvasEdge { id: "e3".into(), source: "a".into(), target: "b".into(), ..Default::default() },
            ],
```

In `service.rs` update the `edge` helper in `context_tests`:

```rust
    fn edge(id: &str, source: &str, target: &str) -> CanvasEdge {
        CanvasEdge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            ..Default::default()
        }
    }
```

- [ ] **Step 5: Run tests to verify pass**

Run: `cargo test -p knowledge-server --lib canvas::`
Expected: PASS — new roundtrip test green, existing canvas tests still green.

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/src/canvas/document.rs crates/knowledge-server/src/canvas/service.rs
git commit -m "feat(canvas): add optional handle/kind fields to CanvasEdge"
```

---

## Task 2: Frontend `canvasEdgeSchema` optional fields

**Files:**
- Modify: `apps/admin/src/features/canvas/types.ts`

Mirror the backend edge with optional camelCase fields so a persisted document carrying handles round-trips through the zod parse. `onConnect`/`onEdgesChange` keep mapping only `{id, source, target}` (Task 12 leaves that untouched) — these fields are placeholder-only for now.

- [ ] **Step 1: Add the optional fields**

Replace `canvasEdgeSchema` in `types.ts`:

```ts
export const canvasEdgeSchema = z.object({
  id: z.string(),
  source: z.string(),
  target: z.string(),
  sourceHandle: z.string().optional(),
  targetHandle: z.string().optional(),
  kind: z.string().optional(),
});
export type CanvasEdge = z.infer<typeof canvasEdgeSchema>;
```

- [ ] **Step 2: Verify types compile**

Run (CWD `apps/admin`): `npx tsc --noEmit`
Expected: clean (schema addition is backward-compatible; existing `{id, source, target}` edges still parse).

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/canvas/types.ts
git commit -m "feat(canvas): allow optional handle/kind on edge schema"
```

---

## Task 3: `ordered_incoming_sources` — spatial (y, then x) ordering

**Files:**
- Modify: `crates/knowledge-server/src/canvas/document.rs`

Return the upstream nodes of `node_id` sorted top-to-bottom, then left-to-right, so reference blocks assemble in reading order rather than edge-insertion order.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `document.rs` (extend the local `node` helper to accept coordinates first):

```rust
    fn node_at(id: &str, ty: &str, x: f64, y: f64) -> CanvasNode {
        CanvasNode { id: id.to_string(), r#type: ty.to_string(), x, y, w: 280.0, h: 160.0, data: serde_json::json!({}) }
    }

    #[test]
    fn ordered_incoming_sources_sorts_by_y_then_x() {
        let doc = CanvasDocument {
            nodes: vec![
                node_at("t", "ai_analyze", 500.0, 500.0),
                node_at("low", "note", 0.0, 300.0),   // lower on canvas
                node_at("hi_right", "note", 200.0, 0.0), // top, right
                node_at("hi_left", "note", 0.0, 0.0),    // top, left (same y as hi_right)
            ],
            edges: vec![
                CanvasEdge { id: "e1".into(), source: "low".into(), target: "t".into(), ..Default::default() },
                CanvasEdge { id: "e2".into(), source: "hi_right".into(), target: "t".into(), ..Default::default() },
                CanvasEdge { id: "e3".into(), source: "hi_left".into(), target: "t".into(), ..Default::default() },
            ],
            viewport: Viewport::default(),
        };
        let ids: Vec<&str> = doc.ordered_incoming_sources("t").iter().map(|n| n.id.as_str()).collect();
        // y ascending: hi_* (y=0) before low (y=300); within y=0, x ascending: hi_left before hi_right.
        assert_eq!(ids, vec!["hi_left", "hi_right", "low"]);
    }
```

- [ ] **Step 2: Run it — expect fail**

Run: `cargo test -p knowledge-server --lib ordered_incoming_sources`
Expected: FAIL — method `ordered_incoming_sources` not found.

- [ ] **Step 3: Implement the method**

Add inside `impl CanvasDocument` in `document.rs` (after `node`):

```rust
    /// Upstream nodes feeding `node_id`, sorted top-to-bottom then left-to-right
    /// (y ascending, x ascending) so reference blocks assemble in reading order.
    pub fn ordered_incoming_sources(&self, node_id: &str) -> Vec<&CanvasNode> {
        let mut sources: Vec<&CanvasNode> = self
            .incoming_source_ids(node_id)
            .iter()
            .filter_map(|src| self.node(src))
            .collect();
        sources.sort_by(|a, b| {
            a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x))
        });
        sources
    }
```

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p knowledge-server --lib ordered_incoming_sources`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/canvas/document.rs
git commit -m "feat(canvas): order incoming sources by spatial position"
```

---

## Task 4: `format_text_reference_block` — pure per-type formatter (adds search + ai_image)

**Files:**
- Modify: `crates/knowledge-server/src/canvas/service.rs`

Extract a single pure function that formats one upstream **text** node into a reference block, filling the two missing arms (`search`, `ai_image`). Returns `None` for KB (handled async elsewhere) and for empty content.

- [ ] **Step 1: Write the failing tests**

Add to the `context_tests` module in `service.rs`:

```rust
    use crate::canvas::document::CanvasNode;

    fn tnode(ty: &str, data: serde_json::Value) -> CanvasNode {
        CanvasNode { id: "n".into(), r#type: ty.into(), x: 0.0, y: 0.0, w: 280.0, h: 160.0, data }
    }

    #[test]
    fn format_text_reference_block_covers_each_text_type() {
        assert_eq!(
            format_text_reference_block(&tnode("note", serde_json::json!({ "markdown": "hi" }))),
            Some("Note:\nhi".to_string())
        );
        assert_eq!(
            format_text_reference_block(&tnode("url", serde_json::json!({ "title": "T", "markdown": "body" }))),
            Some("Web page: T\nbody".to_string())
        );
        let analysis = serde_json::json!({
            "versions": [{ "id": "v1", "content": "result" }], "activeVersionId": "v1"
        });
        assert_eq!(
            format_text_reference_block(&tnode("ai_analyze", analysis)),
            Some("Prior analysis:\nresult".to_string())
        );
        assert_eq!(
            format_text_reference_block(&tnode("search", serde_json::json!({ "markdown": "res md" }))),
            Some("Search results:\nres md".to_string())
        );
        assert_eq!(
            format_text_reference_block(&tnode(
                "ai_image",
                serde_json::json!({ "prompt": "a fox", "url": "/api/assets/x" })
            )),
            Some("Generated image (prompt: a fox): /api/assets/x".to_string())
        );
    }

    #[test]
    fn format_text_reference_block_skips_empty_and_kb() {
        assert_eq!(format_text_reference_block(&tnode("note", serde_json::json!({ "markdown": "  " }))), None);
        assert_eq!(format_text_reference_block(&tnode("search", serde_json::json!({}))), None);
        assert_eq!(format_text_reference_block(&tnode("kb", serde_json::json!({ "projectId": "p" }))), None);
    }
```

- [ ] **Step 2: Run — expect fail**

Run: `cargo test -p knowledge-server --lib format_text_reference_block`
Expected: FAIL — function not defined.

- [ ] **Step 3: Implement the formatter**

Add to `service.rs` (above `collect_reference_blocks`):

```rust
use crate::canvas::document::CanvasNode;

/// Format one upstream *text* node into a reference block. Returns `None` for KB
/// (handled via async RAG) and whenever the node has no usable text.
pub fn format_text_reference_block(node: &CanvasNode) -> Option<String> {
    match node.r#type.as_str() {
        "note" => node
            .data
            .get("markdown")
            .and_then(|v| v.as_str())
            .filter(|m| !m.trim().is_empty())
            .map(|m| format!("Note:\n{m}")),
        "url" => {
            let md = node.data.get("markdown").and_then(|v| v.as_str()).unwrap_or("");
            if md.trim().is_empty() {
                return None;
            }
            let title = node.data.get("title").and_then(|v| v.as_str()).unwrap_or("");
            Some(format!("Web page: {title}\n{md}"))
        }
        "ai_analyze" => crate::canvas::document::active_version_content(&node.data)
            .filter(|c| !c.trim().is_empty())
            .map(|c| format!("Prior analysis:\n{c}")),
        "search" => node
            .data
            .get("markdown")
            .and_then(|v| v.as_str())
            .filter(|m| !m.trim().is_empty())
            .map(|m| format!("Search results:\n{m}")),
        // Text-only reference: the downstream LLM sees the prompt + asset URL, not
        // the pixels (no vision/multimodal — YAGNI per spec §8).
        "ai_image" => {
            let url = node.data.get("url").and_then(|v| v.as_str()).unwrap_or("");
            if url.trim().is_empty() {
                return None;
            }
            let prompt = node.data.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
            Some(format!("Generated image (prompt: {prompt}): {url}"))
        }
        _ => None,
    }
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p knowledge-server --lib format_text_reference_block`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/canvas/service.rs
git commit -m "feat(canvas): pure text reference-block formatter with search+image arms"
```

---

## Task 5: `build_search_query_prompt` — pure prompt builder for query synthesis

**Files:**
- Modify: `crates/knowledge-server/src/canvas/service.rs`

When a `search` node has upstream inputs, an LLM synthesises one web query from the user's guidance + the upstream blocks. Extract the prompt construction as a pure, testable pair `(system, user)`.

- [ ] **Step 1: Write the failing test**

Add to `context_tests` in `service.rs`:

```rust
    #[test]
    fn build_search_query_prompt_embeds_guidance_and_sources() {
        let (system, user) = build_search_query_prompt("find recent news", &["block A".into(), "block B".into()]);
        assert!(system.to_lowercase().contains("search query"));
        assert!(user.contains("find recent news"));
        assert!(user.contains("block A"));
        assert!(user.contains("block B"));
    }
```

- [ ] **Step 2: Run — expect fail**

Run: `cargo test -p knowledge-server --lib build_search_query_prompt`
Expected: FAIL — function not defined.

- [ ] **Step 3: Implement**

Add to `service.rs`:

```rust
/// Build the (system, user) prompt pair that synthesises a single web-search
/// query from the node's guidance text and its upstream reference blocks.
pub fn build_search_query_prompt(guidance: &str, blocks: &[String]) -> (String, String) {
    let system =
        "Output exactly one concise web search query and nothing else. Do not explain, quote, or add punctuation beyond the query itself."
            .to_string();
    let mut user = format!("Guidance: {guidance}\n\nSources:\n");
    for (i, b) in blocks.iter().enumerate() {
        user.push_str(&format!("--- Source {} ---\n{}\n\n", i + 1, b));
    }
    (system, user)
}
```

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p knowledge-server --lib build_search_query_prompt`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/canvas/service.rs
git commit -m "feat(canvas): pure search-query synthesis prompt builder"
```

---

## Task 6: `build_search_done_payload` — SSE done payload for search

**Files:**
- Modify: `crates/knowledge-server/src/canvas/routes.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `routes.rs`:

```rust
    #[test]
    fn search_done_payload_carries_markdown() {
        let payload = build_search_done_payload("Search results for \"cats\":\n- a");
        assert_eq!(payload["markdown"], "Search results for \"cats\":\n- a");
    }
```

- [ ] **Step 2: Run — expect fail**

Run: `cargo test -p knowledge-server --lib search_done_payload_carries_markdown`
Expected: FAIL — `build_search_done_payload` not defined.

- [ ] **Step 3: Implement**

Add near the other done-payload builders in `routes.rs` (after `build_image_done_payload`):

```rust
pub fn build_search_done_payload(markdown: &str) -> serde_json::Value {
    json!({ "markdown": markdown })
}
```

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p knowledge-server --lib search_done_payload_carries_markdown`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/canvas/routes.rs
git commit -m "feat(canvas): search-run done payload builder"
```

---

## Task 7: `run_node_handler` — ordered assembly, ai_image reference injection, search branch

**Files:**
- Modify: `crates/knowledge-server/src/canvas/routes.rs`

This is the core backend change. Rework the handler so:
1. Reference blocks are assembled in **one ordered traversal** (`ordered_incoming_sources`), dispatching text nodes to `format_text_reference_block` and KB nodes to inline async RAG (keeping the per-project permission check + "no access, excluded" placeholder).
2. `ai_image` appends the ordered blocks as a `参考:` suffix to its own prompt when inputs are present (unchanged when none).
3. A new `search` branch: no inputs → `data.query.trim()` (bad_request if empty); with inputs → synthesise a query via the active connection's `complete_text`, take the first line; then `run_web_search_markdown` → done payload `{markdown}` without overwriting `data.query`.

There are no unit tests for the streaming handler itself (SSE + live provider); correctness of its pure helpers is covered by Tasks 3–6. Verification here is compile + full suite.

- [ ] **Step 1: Extract the ordered block-assembly into a helper**

Add this async free function in `routes.rs` (below `run_web_search_markdown`), replacing the inline `collect_reference_blocks` + `referenced_kb_project_ids` loop that currently lives in `run_node_handler`:

```rust
/// Assemble reference blocks for `node_id` in spatial (y, then x) order. Text
/// nodes format synchronously; KB nodes trigger RAG inline with a per-project
/// permission check so every source obeys the same ordering.
async fn assemble_reference_blocks(
    state: &AppState,
    principal: &Principal,
    doc: &CanvasDocument,
    node_id: &str,
    node_prompt: &str,
) -> Result<Vec<String>, ApiError> {
    let mut blocks = Vec::new();
    for src in doc.ordered_incoming_sources(node_id) {
        if src.r#type == "kb" {
            let Some(project_id) = src.data.get("projectId").and_then(|v| v.as_str()) else {
                continue;
            };
            let role = crate::tenancy::access::project_access_role(
                &state.pool,
                project_id,
                &principal.user_id,
            )
            .await
            .map_err(ApiError::from)?;
            match role {
                Some(_role) => {
                    let root =
                        crate::projects::service::project_root_for_id(state, project_id).await?;
                    let assembled = crate::chat::context::assemble_chat_context(
                        state,
                        project_id,
                        &root,
                        node_prompt,
                        8,
                    )
                    .await?;
                    for b in assembled.context_blocks {
                        blocks.push(format!("Knowledge base ({project_id}):\n{b}"));
                    }
                }
                None => {
                    blocks.push(format!("[Knowledge base {project_id}: no access, excluded]"));
                }
            }
        } else if let Some(block) = crate::canvas::service::format_text_reference_block(src) {
            blocks.push(block);
        }
    }
    Ok(blocks)
}
```

- [ ] **Step 2: Rewrite the `ai_image` branch to inject references**

In `run_node_handler`, replace the `if node.r#type == "ai_image" { ... }` block so it assembles ordered blocks and appends them to the prompt when present. The `final_prompt` is what gets sent to `run_image_skill`:

```rust
    if node.r#type == "ai_image" {
        if node_prompt.trim().is_empty() {
            return Err(ApiError::bad_request("image node has no prompt"));
        }
        load_image_config(&state).await?; // validate image config before streaming

        // Upstream references (if any) are appended as plain text so the image
        // prompt can draw on prior analysis / notes. No inputs => prompt unchanged.
        let blocks =
            assemble_reference_blocks(&state, &principal, &doc, &node_id, &node_prompt).await?;
        let final_prompt = if blocks.is_empty() {
            node_prompt.clone()
        } else {
            format!("{node_prompt}\n\n参考:\n{}", blocks.join("\n\n"))
        };

        let user_id = principal.user_id.clone();
        let prompt = final_prompt;
        let stream_state = state.clone();
        let event_stream: std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> =
            Box::pin(async_stream::stream! {
                match run_image_skill(&stream_state, &user_id, &prompt).await {
                    Ok(node_json) => {
                        let url = node_json
                            .get("data")
                            .and_then(|d| d.get("url"))
                            .and_then(|u| u.as_str())
                            .unwrap_or("")
                            .to_string();
                        let version_id = uuid::Uuid::new_v4().to_string();
                        let created_at = now_rfc3339();
                        yield Ok(
                            Event::default().event("done").data(
                                build_image_done_payload(&version_id, &url, &created_at)
                                    .to_string(),
                            ),
                        );
                    }
                    Err(message) => yield Ok(sse_error(&message)),
                }
            });
        return Ok(Sse::new(event_stream).keep_alive(KeepAlive::default()));
    }
```

- [ ] **Step 3: Add the `search` branch**

Immediately after the `ai_image` branch (before the analyze path), add a `search` branch. It resolves the query synchronously (non-streaming), runs the web search, and emits a single `done` with `{markdown}`:

```rust
    if node.r#type == "search" {
        let blocks =
            assemble_reference_blocks(&state, &principal, &doc, &node_id, &node_prompt).await?;
        let guidance = node.data.get("query").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();

        // Resolve the effective query: upstream inputs let an LLM synthesise one
        // from the guidance; with no inputs the guidance itself is the query.
        let query = if blocks.is_empty() {
            if guidance.is_empty() {
                return Err(ApiError::bad_request("search node has no query"));
            }
            guidance
        } else {
            let (system_prompt, user_prompt) =
                crate::canvas::service::build_search_query_prompt(&guidance, &blocks);
            let provider = load_active_connection(&state).await?.provider();
            let response = provider
                .complete_text(crate::providers::ProviderTextRequest { system_prompt, user_prompt })
                .await
                .map_err(|e| ApiError::bad_request(e.message().to_string()))?;
            let synthesized = response.text.lines().next().unwrap_or("").trim().to_string();
            if synthesized.is_empty() {
                return Err(ApiError::bad_request("could not synthesize a search query"));
            }
            synthesized
        };

        let stream_state = state.clone();
        let event_stream: std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> =
            Box::pin(async_stream::stream! {
                match run_web_search_markdown(&stream_state, &query).await {
                    Ok(markdown) => {
                        yield Ok(
                            Event::default().event("done").data(
                                build_search_done_payload(&markdown).to_string(),
                            ),
                        );
                    }
                    Err(message) => yield Ok(sse_error(&message)),
                }
            });
        return Ok(Sse::new(event_stream).keep_alive(KeepAlive::default()));
    }
```

Note: `ProviderTextRequest` must be importable — it is re-exported from `crate::providers`. If the `crate::providers::ProviderTextRequest` path does not resolve, add it to the existing `use crate::providers::{...}` group at the top of `routes.rs`.

- [ ] **Step 4: Replace the analyze path's block collection**

The analyze path currently calls `collect_reference_blocks` then loops `referenced_kb_project_ids`. Replace both (lines ~310–342) with a single call so analyze also uses ordered assembly:

```rust
    // Ordered Note/URL/prior-analysis/search/image references + KB via RAG.
    let blocks =
        assemble_reference_blocks(&state, &principal, &doc, &node_id, &node_prompt).await?;

    let prompt = crate::canvas::service::build_analyze_prompt(&node_prompt, &blocks);
```

Delete the now-obsolete `let mut blocks = ...collect_reference_blocks...;` line and the entire `for project_id in ...referenced_kb_project_ids... { ... }` loop.

- [ ] **Step 5: Verify compile + full suite**

Run: `cargo test -p knowledge-server --lib`
Expected: compiles; all tests pass. (Note: `collect_reference_blocks` and `referenced_kb_project_ids` may now be unused — they are removed in Task 8; a dead-code warning here is acceptable and resolved next.)

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/src/canvas/routes.rs
git commit -m "feat(canvas): unify run_node_handler with ordered assembly, image refs, search branch"
```

---

## Task 8: Remove the REST search endpoint + obsolete service functions

**Files:**
- Modify: `crates/knowledge-server/src/canvas/routes.rs`
- Modify: `crates/knowledge-server/src/canvas/service.rs`

Clean cutover (no compat): drop the `/api/canvas/search` route, `search_handler`, `SearchRequest`, and its test. Keep `run_web_search_markdown` (used by the `/search` chat skill). Remove `collect_reference_blocks` and `referenced_kb_project_ids` (superseded by `assemble_reference_blocks` + `format_text_reference_block`) and their now-dead tests.

- [ ] **Step 1: routes.rs — drop route, handler, request, test**

In `router()` remove the line:
```rust
        .route("/api/canvas/search", post(search_handler))
```
Delete the `SearchRequest` struct (the `#[derive(Debug, Deserialize)] struct SearchRequest { query: String }` block and its `// Web search:` comment banner) and the entire `search_handler` async fn. Keep `run_web_search_markdown` intact. In the `tests` module delete:
```rust
    #[test]
    fn search_request_deserializes_query() { ... }
```

- [ ] **Step 2: service.rs — remove superseded functions + their tests**

Delete `collect_reference_blocks` and `referenced_kb_project_ids` functions. In `context_tests` delete these three tests that exercised them:
- `collect_reference_blocks_includes_note_and_url_text`
- `collect_reference_blocks_uses_active_version_of_prior_analysis`
- `referenced_kb_project_ids_collects_incoming_kb_projects`

Keep `search_results_to_markdown`, `build_analyze_prompt`, `format_text_reference_block`, `build_search_query_prompt`, `SearchResultEntry`, and their tests.

- [ ] **Step 3: Verify no dangling references**

Run: `cargo test -p knowledge-server --lib`
Expected: compiles with no unused-function warnings for the deleted items; all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/src/canvas/routes.rs crates/knowledge-server/src/canvas/service.rs
git commit -m "refactor(canvas): drop REST search endpoint and superseded block collectors"
```

---

## Task 9: Backend save-time edge prune

**Files:**
- Modify: `crates/knowledge-server/src/canvas/document.rs`
- Modify: `crates/knowledge-server/src/canvas/routes.rs`

Defensive cleanup on `PUT`: silently prune self-loops, duplicates, edges whose `target` is not a consumer, and dangling edges (source/target missing). Silent prune (not 400) because autosave must not fail a whole save over one stale edge. Cycle detection is NOT pruned here (runtime reads one predecessor level; front-end blocks cycles).

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `document.rs` (uses the `node_at` helper added in Task 3):

```rust
    #[test]
    fn prune_invalid_edges_removes_illegal_edges() {
        let doc = CanvasDocument {
            nodes: vec![
                node_at("note1", "note", 0.0, 0.0),
                node_at("an", "ai_analyze", 100.0, 0.0),
                node_at("kb1", "kb", 0.0, 100.0),
            ],
            edges: vec![
                // legal: note -> ai_analyze (consumer target)
                CanvasEdge { id: "ok".into(), source: "note1".into(), target: "an".into(), ..Default::default() },
                // self-loop
                CanvasEdge { id: "self".into(), source: "an".into(), target: "an".into(), ..Default::default() },
                // duplicate of "ok"
                CanvasEdge { id: "dup".into(), source: "note1".into(), target: "an".into(), ..Default::default() },
                // target not a consumer (kb cannot be a target)
                CanvasEdge { id: "bad_target".into(), source: "note1".into(), target: "kb1".into(), ..Default::default() },
                // dangling: ghost source
                CanvasEdge { id: "dangling".into(), source: "ghost".into(), target: "an".into(), ..Default::default() },
            ],
            viewport: Viewport::default(),
        };
        let pruned = doc.prune_invalid_edges();
        let ids: Vec<&str> = pruned.edges.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["ok"]);
    }
```

- [ ] **Step 2: Run — expect fail**

Run: `cargo test -p knowledge-server --lib prune_invalid_edges`
Expected: FAIL — method not found.

- [ ] **Step 3: Implement `prune_invalid_edges`**

Add inside `impl CanvasDocument` in `document.rs`:

```rust
    /// Return a copy with illegal edges removed: self-loops, duplicates
    /// (same source+target), edges whose target is not a consumer
    /// (search/ai_analyze/ai_image), and dangling edges (missing endpoints).
    /// Cycles are intentionally not pruned (front-end blocks them; runtime reads
    /// only direct predecessors so a cycle is harmless).
    pub fn prune_invalid_edges(&self) -> CanvasDocument {
        let is_consumer =
            |ty: Option<&str>| matches!(ty, Some("search" | "ai_analyze" | "ai_image"));

        let mut seen: std::collections::HashSet<(String, String)> =
            std::collections::HashSet::new();
        let edges = self
            .edges
            .iter()
            .filter(|e| e.source != e.target)
            .filter(|e| self.node(&e.source).is_some() && self.node(&e.target).is_some())
            .filter(|e| is_consumer(self.node(&e.target).map(|n| n.r#type.as_str())))
            .filter(|e| seen.insert((e.source.clone(), e.target.clone())))
            .cloned()
            .collect();

        CanvasDocument { nodes: self.nodes.clone(), edges, viewport: self.viewport.clone() }
    }
```

- [ ] **Step 4: Call it in the save path**

In `routes.rs::save_handler`, prune before persisting. Replace the document argument passed to `store::update_canvas`:

```rust
    let pruned = body.document.prune_invalid_edges();
    let document_text = serde_json::to_string(&pruned).unwrap_or_else(|_| "{}".to_string());
    let rec = store::update_canvas(
        &state.pool,
        &id,
        &principal.user_id,
        &body.title,
        &document_text,
    )
```

(This replaces the previous `&body.document_text()` argument. `SaveCanvasRequest::document_text` may now be unused — if the compiler warns, delete the method and its `save_request_serializes_document_to_text` test; verify with the compiler.)

- [ ] **Step 5: Run test to verify pass**

Run: `cargo test -p knowledge-server --lib`
Expected: PASS — prune test green, save handler compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/src/canvas/document.rs crates/knowledge-server/src/canvas/routes.rs
git commit -m "feat(canvas): prune illegal edges on save"
```

---

### Task 10: Front-end handle matrix (producer vs consumer)

Give each node the correct handles per §2: pure producers (`note`, `kb`) lose their
left input handle; `search` gains one (it becomes a consumer). `NodeShell` already
supports `targetHandle` (default `true`), so this is prop-only.

**Files:**
- Modify: `apps/admin/src/features/canvas/node-types/note.tsx`
- Modify: `apps/admin/src/features/canvas/node-types/kb.tsx`
- Modify: `apps/admin/src/features/canvas/node-types/search.tsx`
- Modify: `apps/admin/src/features/canvas/node-types/search.test.tsx`
- Modify: `apps/admin/src/features/canvas/board-render.test.tsx`

- [ ] **Step 1: Update the search handle test to expect two handles (write it first — it must fail)**

`search` is becoming a consumer, so it must render **both** a source and a target
handle. Replace the last test in `search.test.tsx` (currently "renders only a source
handle (no target — it is a data source)"):

```tsx
  it("renders both a source and a target handle (it is a consumer)", () => {
    const { container } = renderNode({ query: "cats" });
    expect(container.querySelectorAll(".react-flow__handle").length).toBe(2);
  });
```

- [ ] **Step 2: Run it — expect fail**

Run (CWD `apps/admin`): `npx vitest run src/features/canvas/node-types/search.test.tsx`
Expected: FAIL — still 1 handle (search hard-codes `targetHandle={false}`).

- [ ] **Step 3: Remove `targetHandle={false}` from `search.tsx`**

In `apps/admin/src/features/canvas/node-types/search.tsx`, delete the `targetHandle={false}`
line (currently line 40) from the `<NodeShell>` props so it defaults back to `true`:

```tsx
    <NodeShell
      icon={<Search className="size-3.5" />}
      label="SEARCH · WEB"
      nodeId={nodeId}
      index={index}
      selected={selected}
      status={status}
      headerRight={
```

- [ ] **Step 4: Run it — expect pass**

Run (CWD `apps/admin`): `npx vitest run src/features/canvas/node-types/search.test.tsx`
Expected: PASS — 2 handles now.

- [ ] **Step 5: Add `targetHandle={false}` to `note.tsx`**

In `apps/admin/src/features/canvas/node-types/note.tsx`, add the prop to `<NodeShell>`:

```tsx
    <NodeShell
      icon={<StickyNote className="size-3.5" />}
      label="NOTE"
      nodeId={nodeId}
      index={index}
      selected={selected}
      targetHandle={false}
    >
```

- [ ] **Step 6: Add `targetHandle={false}` to `kb.tsx`**

In `apps/admin/src/features/canvas/node-types/kb.tsx`, add the prop to `<NodeShell>`:

```tsx
    <NodeShell
      icon={<Database className="size-3.5" />}
      label="KNOWLEDGE BASE"
      nodeId={nodeId}
      index={index}
      selected={selected}
      status={data.noAccess ? "no-access" : "idle"}
      targetHandle={false}
    >
```

- [ ] **Step 7: Fix the board-render handle test (notes now have one handle)**

`board-render.test.tsx`'s "renders source and target handles" test currently renders two
`note` nodes and expects `>= 4` handles. Notes are now source-only (2 handles total), so
that assertion breaks. Replace the whole `it("renders source and target handles ...")`
block (lines 80-94) with a producer+consumer document so both handle kinds are exercised:

```tsx
  it("renders source and target handles so edges can be drawn by hand", () => {
    // A pure producer (note: source only) plus a consumer (ai_analyze: source +
    // target). Without both handle kinds users could only get edges from /analyze
    // auto-wiring, never by dragging between nodes.
    const doc: CanvasDocument = {
      nodes: [
        { id: "n1", type: "note", x: 0, y: 0, w: 280, h: 160, data: {} },
        { id: "a1", type: "ai_analyze", x: 400, y: 0, w: 360, h: 320, data: {} },
      ],
      edges: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    };
    const { container } = render(
      <CanvasBoard
        document={doc}
        onChange={vi.fn()}
        onRunNode={vi.fn()}
        onFetchUrl={vi.fn()}
        onSearchNode={vi.fn()}
      />,
    );
    // note contributes 1 (source), ai_analyze contributes 2 (source + target) = 3.
    expect(container.querySelectorAll(".react-flow__handle").length).toBeGreaterThanOrEqual(3);
  });
```

- [ ] **Step 8: Run the canvas front-end suite**

Run (CWD `apps/admin`): `npx vitest run src/features/canvas`
Expected: PASS — search/board-render green (`onSearchNode` prop is removed in Task 12).

- [ ] **Step 9: Commit**

```bash
git add apps/admin/src/features/canvas/node-types/note.tsx apps/admin/src/features/canvas/node-types/kb.tsx apps/admin/src/features/canvas/node-types/search.tsx apps/admin/src/features/canvas/node-types/search.test.tsx apps/admin/src/features/canvas/board-render.test.tsx
git commit -m "feat(canvas): producer/consumer handle matrix"
```

---

### Task 11: `isValidConnection` — block illegal edges during drag (§3.2)

A pure predicate the board hands React Flow so an illegal handle never highlights and a
release makes no edge. Rules: endpoints differ, target is a consumer, not a duplicate, and
no cycle (target must not already reach source).

**Files:**
- Modify: `apps/admin/src/features/canvas/canvas-board.tsx`
- Modify: `apps/admin/src/features/canvas/canvas-board.test.tsx`

- [ ] **Step 1: Write the failing unit tests**

Add this `describe` block to `canvas-board.test.tsx` (after the `pruneDanglingEdges`
block), and add `isValidConnection` to the existing import from `./canvas-board`:

```tsx
describe("isValidConnection", () => {
  const nodes = [
    { id: "note1", type: "note" },
    { id: "an1", type: "ai_analyze" },
    { id: "img1", type: "ai_image" },
    { id: "s1", type: "search" },
    { id: "kb1", type: "kb" },
  ];

  it("accepts a producer -> consumer edge", () => {
    expect(isValidConnection(nodes, [], { source: "note1", target: "an1" })).toBe(true);
  });

  it("rejects a self-loop", () => {
    expect(isValidConnection(nodes, [], { source: "an1", target: "an1" })).toBe(false);
  });

  it("rejects a target that is not a consumer", () => {
    expect(isValidConnection(nodes, [], { source: "an1", target: "note1" })).toBe(false);
    expect(isValidConnection(nodes, [], { source: "note1", target: "kb1" })).toBe(false);
  });

  it("rejects a duplicate edge", () => {
    const edges = [{ source: "note1", target: "an1" }];
    expect(isValidConnection(nodes, edges, { source: "note1", target: "an1" })).toBe(false);
  });

  it("rejects an edge that would form a cycle", () => {
    // an1 -> img1 exists; adding img1 -> an1 would close a loop.
    const edges = [{ source: "an1", target: "img1" }];
    expect(isValidConnection(nodes, edges, { source: "img1", target: "an1" })).toBe(false);
  });

  it("rejects a null endpoint", () => {
    expect(isValidConnection(nodes, [], { source: null, target: "an1" })).toBe(false);
  });
});
```

Update the import line:

```tsx
import { CanvasBoard, commitNodeGeometry, isResizeEndChange, isValidConnection, pruneDanglingEdges } from "./canvas-board";
```

- [ ] **Step 2: Run — expect fail**

Run (CWD `apps/admin`): `npx vitest run src/features/canvas/canvas-board.test.tsx`
Expected: FAIL — `isValidConnection` is not exported.

- [ ] **Step 3: Implement `isValidConnection`**

In `canvas-board.tsx`, add near `pruneDanglingEdges` (an exported module-level helper):

```tsx
const CONSUMER_TYPES = ["search", "ai_analyze", "ai_image"];

// A connection is valid iff: the endpoints differ, the target is a consumer
// (search/ai_analyze/ai_image), it does not duplicate an existing edge, and it
// would not create a cycle (walking forward from target must not reach source).
// React Flow calls this during a drag, so an illegal handle never highlights and
// a release over it makes no edge (§3.2 "middle" blocking).
export function isValidConnection(
  nodes: { id: string; type?: string }[],
  edges: { source: string; target: string }[],
  conn: { source: string | null; target: string | null },
): boolean {
  const { source, target } = conn;
  if (!source || !target || source === target) {
    return false;
  }
  const targetNode = nodes.find((n) => n.id === target);
  if (!targetNode || !CONSUMER_TYPES.includes(targetNode.type ?? "")) {
    return false;
  }
  if (edges.some((e) => e.source === source && e.target === target)) {
    return false;
  }
  const adjacency = new Map<string, string[]>();
  for (const e of edges) {
    const list = adjacency.get(e.source);
    if (list) {
      list.push(e.target);
    } else {
      adjacency.set(e.source, [e.target]);
    }
  }
  const stack = [target];
  const seen = new Set<string>();
  while (stack.length > 0) {
    const cur = stack.pop() as string;
    if (cur === source) {
      return false;
    }
    if (seen.has(cur)) {
      continue;
    }
    seen.add(cur);
    for (const next of adjacency.get(cur) ?? []) {
      stack.push(next);
    }
  }
  return true;
}
```

- [ ] **Step 4: Run — expect pass**

Run (CWD `apps/admin`): `npx vitest run src/features/canvas/canvas-board.test.tsx`
Expected: PASS.

- [ ] **Step 5: Wire it into `<ReactFlow>`**

Inside the `CanvasBoard` component, add a memoized callback next to `onConnect`:

```tsx
  const validateConnection = useCallback(
    (conn: Connection | Edge) => isValidConnection(rfNodes, rfEdges, conn),
    [rfNodes, rfEdges],
  );
```

And pass it to the `<ReactFlow>` element:

```tsx
        <ReactFlow
          nodes={rfNodes}
          edges={rfEdges}
          nodeTypes={nodeTypes}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          isValidConnection={validateConnection}
          deleteKeyCode={["Delete", "Backspace"]}
          defaultViewport={document.viewport}
        >
```

`Connection` and `Edge` are already imported as types at the top of the file.

- [ ] **Step 6: Run the canvas front-end suite + types**

Run (CWD `apps/admin`): `npx vitest run src/features/canvas && npx tsc --noEmit`
Expected: PASS, tsc clean.

- [ ] **Step 7: Commit**

```bash
git add apps/admin/src/features/canvas/canvas-board.tsx apps/admin/src/features/canvas/canvas-board.test.tsx
git commit -m "feat(canvas): reject illegal edges via isValidConnection"
```

---

### Task 12: Unify the search Run path (remove the standalone search endpoint)

`search` stops using its own `POST /api/canvas/search` path and joins the shared SSE Run
(`onRunNode`). Remove the `onSearch`/`onSearchNode` plumbing, `runSearchNode`, and the
`searchWeb` client. (Backend removal of `/api/canvas/search` was Task 8.)

**Files:**
- Modify: `apps/admin/src/features/canvas/node-types/search.tsx`
- Modify: `apps/admin/src/features/canvas/node-types/search.test.tsx`
- Modify: `apps/admin/src/features/canvas/canvas-board.tsx`
- Modify: `apps/admin/src/features/canvas/page.tsx`
- Modify: `apps/admin/src/features/canvas/api.ts`
- Modify: `apps/admin/src/features/canvas/page.test.tsx`
- Modify: `apps/admin/src/features/canvas/board-render.test.tsx`
- Modify: `apps/admin/src/features/canvas/canvas-board.test.tsx`

- [ ] **Step 1: Rename `search.tsx`'s `onSearch` prop to `onRun`**

In `node-types/search.tsx`, rename the callback prop so its semantics match analyze/image
(it now drives the shared Run). Update the interface, the destructure, and the button
`onClick`. Also relax the disabled rule: with an upstream input the query box may be empty
(the query is synthesised from upstream), so only disable while loading:

```tsx
interface SearchNodeProps {
  data: SearchNodeData;
  nodeId: string;
  index?: number;
  selected?: boolean;
  onQueryChange: (query: string) => void;
  onRun: () => void;
}

export function SearchNode({ data, nodeId, index, selected, onQueryChange, onRun }: SearchNodeProps) {
```

```tsx
        <Button
          type="button"
          size="xs"
          variant={isError ? "destructive" : "default"}
          onClick={onRun}
          disabled={loading}
        >
```

- [ ] **Step 2: Update `search.test.tsx` for the `onRun` rename**

In `search.test.tsx`, rename the `onSearch` helper param/prop to `onRun`, and update the
click test. The "disables the button with an empty query" test is now wrong (an empty
query is allowed because upstream can supply one) — replace it with a "not disabled when
idle" assertion:

```tsx
function renderNode(data: SearchNodeData, onRun = vi.fn(), onQueryChange = vi.fn()) {
  return render(
    <ReactFlowProvider>
      <SearchNode
        data={data}
        nodeId="abcd-1234"
        onQueryChange={onQueryChange}
        onRun={onRun}
      />
    </ReactFlowProvider>,
  );
}
```

```tsx
  it("calls onRun when the button is clicked", () => {
    const onRun = vi.fn();
    renderNode({ query: "cats" }, onRun);
    fireEvent.click(screen.getByRole("button", { name: /search/i }));
    expect(onRun).toHaveBeenCalledTimes(1);
  });

  it("keeps the button enabled with an empty query (upstream can supply one)", () => {
    renderNode({ query: "" });
    expect(screen.getByRole("button", { name: /search/i })).not.toBeDisabled();
  });
```

- [ ] **Step 3: Point `SearchAdapter` at `cb.onRun` and drop `onSearch`/`onSearchNode` plumbing**

In `canvas-board.tsx`:

Remove `onSearch` from the `NodeCallbacks` interface:

```tsx
interface NodeCallbacks {
  onPatch: (patch: Record<string, unknown>) => void;
  onRun: () => void;
  onFetchUrl: () => void;
}
```

Remove the `onSearch` default in `callbacks()`:

```tsx
function callbacks(data: Record<string, unknown>): NodeCallbacks {
  return (
    (data.__cb as NodeCallbacks | undefined) ?? {
      onPatch: () => {},
      onRun: () => {},
      onFetchUrl: () => {},
    }
  );
}
```

Point `SearchAdapter` at `cb.onRun`:

```tsx
function SearchAdapter({ id, data, selected }: NodeProps) {
  const cb = callbacks(data);
  return (
    <SearchNode
      data={data as unknown as SearchNodeData}
      nodeId={id}
      index={indexOf(data)}
      selected={selected}
      onQueryChange={(query) => cb.onPatch({ query })}
      onRun={cb.onRun}
    />
  );
}
```

Remove `onSearchNode` from `CanvasBoardProps`:

```tsx
interface CanvasBoardProps {
  document: CanvasDocument;
  onChange: (next: CanvasDocument) => void;
  onRunNode: (nodeId: string) => void;
  onFetchUrl: (nodeId: string) => void;
  onSelectionChange?: (nodeIds: string[]) => void;
}
```

Remove it from the destructure:

```tsx
export function CanvasBoard({ document, onChange, onRunNode, onFetchUrl, onSelectionChange }: CanvasBoardProps) {
```

Remove the `onSearch` injection in `__cb` and drop `onSearchNode` from the deps array:

```tsx
          __cb: {
            onPatch: (patch: Record<string, unknown>) => patchNode(n.id, patch),
            onRun: () => onRunNode(n.id),
            onFetchUrl: () => onFetchUrl(n.id),
          } satisfies NodeCallbacks,
```

```tsx
    [document.nodes, patchNode, onRunNode, onFetchUrl, analyzeModel, imageModel, selectedIds],
```

- [ ] **Step 4: Remove `runSearchNode` and its wiring in `page.tsx`**

In `page.tsx`:

Drop `searchWeb` from the import (keep `extractUrl`):

```tsx
import { extractUrl } from "./api";
```

Delete the entire `runSearchNode` `useCallback` (currently lines 287-315).

Remove `onSearchNode={runSearchNode}` from the `<CanvasBoard>` element:

```tsx
              <CanvasBoard key={canvasId} document={doc} onChange={setDoc} onRunNode={runNode} onFetchUrl={fetchUrlNode} onSelectionChange={setSelectedNodeIds} />
```

- [ ] **Step 5: Remove `searchWeb` + `searchResultSchema` from `api.ts`**

Delete both (currently lines 79-96). `run_web_search_markdown` still exists server-side for
the `/search` chat skill; only the canvas HTTP client is removed.

- [ ] **Step 6: Update `board-render.test.tsx` and `canvas-board.test.tsx` — remove `onSearchNode` props**

`onSearchNode` is no longer a `CanvasBoard` prop, so every `onSearchNode={...}` becomes an
excess-prop type error. Remove all occurrences:

- In `board-render.test.tsx`: delete the `onSearchNode={vi.fn()}` line from each of the six
  `<CanvasBoard>` renders (the five multi-line ones and the inline one on the
  "shows the active-connection model" test).
- In `canvas-board.test.tsx`: delete the `onSearchNode={() => {}}` line from both
  `<CanvasBoard>` renders.

- [ ] **Step 7: Rewrite the page's search test to go through the unified Run**

In `page.test.tsx`:

Remove the `searchWeb` mock entirely:
- Delete `const searchWeb = vi.fn();` (line 8).
- Delete `searchWeb.mockReset();` from `beforeEach` (line 32).
- Delete `searchWeb: (query: string) => searchWeb(query),` from the `vi.mock("./api")` factory (line 42).
- Delete `onSearchNode?: (id: string) => void;` from the `boardProps` type (line 70).

Replace the "runs a search node through the search endpoint" test (lines 186-204) with:

```tsx
  it("runs a search node through the unified run endpoint", async () => {
    canvasResult = {
      data: {
        ...c1Data(),
        document: {
          nodes: [
            { id: "s1", type: "search", x: 0, y: 0, w: 280, h: 160, data: { query: "cats" } } as unknown as ReturnType<typeof c1Data>["document"]["nodes"][number],
          ],
          edges: [],
          viewport: { x: 0, y: 0, zoom: 1 },
        },
      },
    };
    render(<CanvasPage />);
    await waitFor(() => expect(boardProps.onRunNode).toBeTypeOf("function"));
    boardProps.onRunNode?.("s1");
    await waitFor(() =>
      expect(runCanvasNode).toHaveBeenCalledWith("c1", "s1", expect.anything()),
    );
  });
```

- [ ] **Step 8: Run the canvas suite + types**

Run (CWD `apps/admin`): `npx vitest run src/features/canvas && npx tsc --noEmit`
Expected: PASS, tsc clean.

- [ ] **Step 9: Commit**

```bash
git add apps/admin/src/features/canvas/node-types/search.tsx apps/admin/src/features/canvas/node-types/search.test.tsx apps/admin/src/features/canvas/canvas-board.tsx apps/admin/src/features/canvas/page.tsx apps/admin/src/features/canvas/api.ts apps/admin/src/features/canvas/page.test.tsx apps/admin/src/features/canvas/board-render.test.tsx apps/admin/src/features/canvas/canvas-board.test.tsx
git commit -m "refactor(canvas): route search through the shared Run path"
```

---

### Task 13: Search Run result — write `data.markdown`, don't append a version

The shared Run's `done` payload carries `markdown` for a search node. The front-end writes
it to `data.markdown` (no versioning — search results replace, they don't accumulate).

**Files:**
- Modify: `apps/admin/src/features/canvas/stream.ts`
- Modify: `apps/admin/src/features/canvas/page.tsx`
- Modify: `apps/admin/src/features/canvas/page.test.tsx`

- [ ] **Step 1: Add `markdown?` to the run `onDone` payload**

In `stream.ts`, extend `NodeRunHandlers.onDone`:

```ts
export interface NodeRunHandlers {
  onDelta: (text: string) => void;
  onDone: (payload: {
    versionId: string;
    createdAt: string;
    content?: string;
    url?: string;
    markdown?: string;
  }) => void;
  onError: (message: string) => void;
}
```

(`versionId`/`createdAt` stay required in the type; the search branch simply ignores them,
matching the analyze/image contract without a schema fork.)

- [ ] **Step 2: Write the failing page test**

In `page.test.tsx`, add a test asserting a search Run writes `data.markdown` and appends no
version. `runCanvasNode` is mocked, so override it to fire `onDone` for this test:

```tsx
  it("writes search results to markdown without appending a version", async () => {
    canvasResult = {
      data: {
        ...c1Data(),
        document: {
          nodes: [
            { id: "s1", type: "search", x: 0, y: 0, w: 280, h: 160, data: { query: "cats" } } as unknown as ReturnType<typeof c1Data>["document"]["nodes"][number],
          ],
          edges: [],
          viewport: { x: 0, y: 0, zoom: 1 },
        },
      },
    };
    runCanvasNode.mockImplementation(
      (_id: string, _nodeId: string, handlers: { onDone: (p: unknown) => void }) => {
        handlers.onDone({ markdown: "# results" });
        return Promise.resolve(undefined);
      },
    );
    render(<CanvasPage />);
    await waitFor(() => expect(boardProps.onRunNode).toBeTypeOf("function"));
    boardProps.onRunNode?.("s1");
    await waitFor(() => {
      const node = boardProps.document?.nodes.find((n) => n.id === "s1");
      expect(node?.data?.markdown).toBe("# results");
      expect(node?.data?.versions).toBeUndefined();
    });
  });
```

- [ ] **Step 3: Run — expect fail**

Run (CWD `apps/admin`): `npx vitest run src/features/canvas/page.test.tsx`
Expected: FAIL — `onDone` currently appends a version for every non-image node, so
`data.versions` is defined and `data.markdown` is unset for the search node.

- [ ] **Step 4: Add the search branch to `runNode.onDone`**

In `page.tsx`, inside the `onDone` `nodes.map` callback, add a `search` branch before the
version logic:

```tsx
          onDone: (payload) => {
            setDoc((prev) => {
              if (!prev) {
                return prev;
              }
              return {
                ...prev,
                nodes: prev.nodes.map((n) => {
                  if (n.id !== nodeId) {
                    return n;
                  }
                  // Search results replace the node's markdown and are not
                  // versioned (unlike analyze/image, which accumulate versions).
                  if (n.type === "search") {
                    return {
                      ...n,
                      data: { ...n.data, status: "idle", error: null, markdown: payload.markdown },
                    };
                  }
                  const versions = Array.isArray(n.data.versions)
                    ? (n.data.versions as unknown[])
                    : [];
                  const version =
                    n.type === "ai_image"
                      ? { id: payload.versionId, url: payload.url, createdAt: payload.createdAt }
                      : {
                          id: payload.versionId,
                          content: payload.content,
                          createdAt: payload.createdAt,
                        };
                  return {
                    ...n,
                    data: {
                      ...n.data,
                      status: "idle",
                      error: null,
                      versions: [...versions, version].slice(-10),
                      activeVersionId: payload.versionId,
                    },
                  };
                }),
              };
            });
          },
```

- [ ] **Step 5: Run — expect pass**

Run (CWD `apps/admin`): `npx vitest run src/features/canvas/page.test.tsx`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/features/canvas/stream.ts apps/admin/src/features/canvas/page.tsx apps/admin/src/features/canvas/page.test.tsx
git commit -m "feat(canvas): search Run writes markdown, no version"
```

---

### Task 14: Stop persisting `data.sourceNodeIds` on skill nodes (§7)

`/analyze` sends `sourceNodeIds` in the SSE node payload so the board can wire edges. It is
transient — the runtime never reads it — but `addSkillNode` currently spreads it into the
persisted `node.data`, leaving a drifting dead field. Read it to build edges, then exclude
it from the stored data.

**Files:**
- Modify: `apps/admin/src/features/canvas/page.tsx`
- Modify: `apps/admin/src/features/canvas/page.test.tsx`

- [ ] **Step 1: Write the failing test**

In `page.test.tsx`, add a test asserting the created node's `data` carries the real fields
but not `sourceNodeIds`:

```tsx
  it("does not persist sourceNodeIds onto the created node's data", async () => {
    render(<CanvasPage />);
    await waitFor(() => expect(chatProps.onSkillNode).toBeTypeOf("function"));
    chatProps.onSkillNode?.({
      node: { type: "ai_analyze", data: { prompt: "sum", sourceNodeIds: ["u1"] } },
      x: 0,
      y: 0,
    });
    await waitFor(() => expect((boardProps.document?.nodes ?? []).length).toBe(2));
    const added = (boardProps.document?.nodes ?? []).find((n) => n.id !== "u1");
    expect(added?.data).not.toHaveProperty("sourceNodeIds");
    expect(added?.data?.prompt).toBe("sum");
    // The edge is still wired from the referenced source.
    const edges = boardProps.document?.edges ?? [];
    expect(edges.some((e) => e.source === "u1" && e.target === added?.id)).toBe(true);
  });
```

- [ ] **Step 2: Run — expect fail**

Run (CWD `apps/admin`): `npx vitest run src/features/canvas/page.test.tsx`
Expected: FAIL — `data` still contains `sourceNodeIds` (spread from `source.data`).

- [ ] **Step 3: Exclude `sourceNodeIds` from persisted node data**

In `page.tsx`'s `addSkillNode`, build the node's `data` from a copy of `source.data` with
`sourceNodeIds` removed (the edge-building code below still reads `source.data?.sourceNodeIds`):

```tsx
      const size = defaultSize(source.type);
      // Chain placement: every new node sits to the right of the most recently
      // created node (highest creation number), top-aligned. The first node on
      // an empty canvas falls back to the payload/view origin.
      const latest = latestNode(prev.nodes);
      const position = latest
        ? { x: latest.x + latest.w + CHAIN_GAP, y: latest.y }
        : (suggestedPosition(payload) ?? placementOrigin(prev));
      // sourceNodeIds is a transient wiring hint from /analyze; it drives the
      // edges below but must not be persisted onto the node (the runtime never
      // reads it and it would drift as the graph changes).
      const persistedData = { ...(source.data ?? {}) };
      delete persistedData.sourceNodeIds;
      const node: CanvasNode = {
        id,
        type: source.type,
        x: position.x,
        y: position.y,
        w: size.w,
        h: size.h,
        // Stable creation number, assigned once and never renumbered (gaps are
        // left after deletions). Read back for display via data.index.
        data: { ...persistedData, index: nextNodeIndex(prev.nodes) },
      };
```

- [ ] **Step 4: Run — expect pass**

Run (CWD `apps/admin`): `npx vitest run src/features/canvas/page.test.tsx`
Expected: PASS — both the new test and the existing "creates reference edges" test (which
reads `source.data.sourceNodeIds`, still intact) stay green.

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/canvas/page.tsx apps/admin/src/features/canvas/page.test.tsx
git commit -m "fix(canvas): stop persisting transient sourceNodeIds"
```

---

### Task 15: Full verification

**Files:** none (verification only).

- [ ] **Step 1: Backend suite + check**

Run from `E:\Projects\Js\knowledge`:

```bash
cargo test -p knowledge-server --lib
cargo check -p knowledge-server
```

Expected: all tests pass (including `ordered_incoming_sources`, `format_text_reference_block`,
`build_search_query_prompt`, `build_search_done_payload`, `prune_invalid_edges`); no warnings
about removed items (`search_handler`, `SearchRequest`, `collect_reference_blocks`,
`referenced_kb_project_ids`, `SaveCanvasRequest::document_text` if it became unused).

- [ ] **Step 2: Front-end suite + types**

Run from `apps/admin`:

```bash
npx vitest run
npx tsc --noEmit
```

Expected: all tests pass; tsc clean. No remaining references to `searchWeb`, `runSearchNode`,
`onSearchNode`, or the search node's old `onSearch` prop.

- [ ] **Step 3: End-to-end smoke (docker)**

Run from `E:\Projects\Js\knowledge`:

```bash
docker compose build backend admin && docker compose up -d
```

Then in the admin UI, verify the design's combinations (§11):
- `url → ai_analyze`: fetch a page, Run analyze — the analysis reflects the page (no regression).
- `note + kb → ai_analyze`: both feed one analysis.
- `note → search`: type a hint in the note, Run the search node — a synthesised query runs and
  markdown lands in the search node (query box hint preserved, not overwritten).
- `search → ai_analyze`: search results feed a downstream analysis (previously silently dropped).
- `ai_analyze → ai_image`: the analysis is appended as a reference to the image prompt.
- Handles: `note`/`kb` show only a right (source) handle; `search` shows both.
- Illegal drags are refused: self-loop, duplicate, a target that is `note`/`url`/`kb`, and a
  cycle-closing edge all fail to connect.
- Save then reload: a hand-authored illegal edge (e.g. targeting a `kb`) is pruned server-side.

- [ ] **Step 4: Finish the branch**

Use the `superpowers:finishing-a-development-branch` skill to complete the work.

---
