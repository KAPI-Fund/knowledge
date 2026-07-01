# Knowledge Canvas Review Remediation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the seven verified defects (P0 #1-#4, P1 #5-#7) from the code review so the Knowledge Canvas front/back protocol actually closes: slash-skill nodes land on the board, URL fetch uses the extract endpoint, AI images generate and regenerate, selected-node analysis wires reference edges, autosave does not fire on open, viewport round-trips, and there is a reachable UI to add note/url/kb nodes.

**Architecture:** The canvas page owns the live `CanvasDocument` and drives autosave; the board is a controlled xyflow view; the chat panel issues slash skills over SSE. Fixes span the Rust `canvas` routes (SSE event contract, request casing, image re-run branch) and the React `canvas` feature (autosave baseline, url-fetch callback, selection reporting, reference-edge creation, viewport persistence, add-node toolbar).

**Tech Stack:** Rust (Axum 0.8, sqlx, async-stream SSE), React 19 + Vite, TanStack Query 5, `@xyflow/react` v12, shadcn/ui, Vitest.

**Decisions locked with the user (2026-07-01):**
- Scope = all findings; **#8 (ASCII) is a documented won't-fix** — Chinese UI copy stays, consistent with every sibling admin screen and the plan's own prescribed literals. ASCII-only applies to code/comments/identifiers, not user-facing copy.
- #3 image regenerate = **build a real backend image re-run branch** on the existing node-run endpoint, appending a new version.

**Won't-fix (finding #8):** No code change. Rationale recorded here so it is not re-flagged.

---

## Finding -> Task map

| Finding | Severity | Task(s) |
|---|---|---|
| #4 request casing (`selectedNodeIds` vs `selected_node_ids`) | P0 | B1 |
| #1 skill nodes emitted on `done`, frontend listens on `node` | P0 | B2, F5 |
| #3 image node data shape + no re-run endpoint | P0 | B3, F2 |
| #4 selection not wired + `/analyze` creates no reference edges | P0 | F4 |
| #2 URL Fetch calls analyze instead of extract-url | P0 | F3 |
| #5 spurious autosave on open | P1 | F1 |
| #6 viewport not round-tripped + skill nodes stack at (0,0) | P1 | F6 |
| #7 no reachable UI to add note/url/kb | P1 | F7 |
| #8 ASCII drift | P2 | won't-fix (documented) |

## File Structure

**Backend (`crates/knowledge-server/src/canvas/`):**
- `routes.rs` — Modify: `CanvasChatRequest` casing (B1); chat SSE emits `node` event (B2); `run_node_handler` dispatches analyze vs image + new `build_image_done_payload` (B3).

**Frontend (`apps/admin/src/features/canvas/`):**
- `use-autosave.ts` — Rewrite: JSON-baseline dirty check + `reset(value)` (F1).
- `stream.ts` — Modify: widen `NodeRunHandlers.onDone` payload to carry optional `content`/`url` (F2).
- `page.tsx` — Modify: `runNode` done-handler builds analyze or image version; `fetchUrlNode` (F3); selection state + reference-edge creation in `addSkillNode` + placement (F4/F6); call `reset` on load (F1); render toolbar (F7).
- `canvas-board.tsx` — Modify: inject `onFetchUrl` (F3); `onSelectionChange` (F4); `defaultViewport` + `onMoveEnd` persistence (F6).
- `chat-panel.tsx` — Modify: skill submit uses `onNode` + confirmation, no dangling "…" bubble (F5); import shared KB picker (F7).
- `kb-picker.tsx` — Create: `KbProjectPicker` moved out of chat-panel for reuse (F7).
- `canvas-toolbar.tsx` — Create: add-node toolbar (Note/URL/KB) (F7).

Test files sit next to each source file (`*.test.ts[x]`). Rust tests live in the `#[cfg(test)]` module of `routes.rs`.

---

## Task B1: Accept camelCase in the canvas chat request (#4)

**Files:**
- Modify: `crates/knowledge-server/src/canvas/routes.rs:374-383` (`CanvasChatRequest`)
- Test: `crates/knowledge-server/src/canvas/routes.rs` tests module

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `routes.rs`:

```rust
    #[test]
    fn chat_request_deserializes_camel_case_selected_ids() {
        let req: CanvasChatRequest = serde_json::from_str(
            r#"{"message":"hi","selectedNodeIds":["a","b"],"x":10.0,"y":20.0}"#,
        )
        .unwrap();
        assert_eq!(req.message, "hi");
        assert_eq!(req.selected_node_ids, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(req.x, Some(10.0));
        assert_eq!(req.y, Some(20.0));
    }
```

Note: `CanvasChatRequest` is currently private (`struct CanvasChatRequest`). The test is in the same module tree via `use super::*;`, so no visibility change is needed.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p knowledge-server chat_request_deserializes_camel_case_selected_ids`
Expected: FAIL — `selected_node_ids` is empty because the payload key is `selectedNodeIds` but serde expects `selected_node_ids`.

- [ ] **Step 3: Add camelCase rename**

Change the struct at `routes.rs:374`:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CanvasChatRequest {
    message: String,
    #[serde(default)]
    selected_node_ids: Vec<String>,
    #[serde(default)]
    x: Option<f64>,
    #[serde(default)]
    y: Option<f64>,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p knowledge-server chat_request_deserializes_camel_case_selected_ids`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/canvas/routes.rs
git commit -m "fix(canvas): accept camelCase selectedNodeIds in chat request"
```

---

## Task B2: Emit skill results on a `node` SSE event (#1)

**Files:**
- Modify: `crates/knowledge-server/src/canvas/routes.rs:440-481` (chat SSE skill branches)
- Test: `crates/knowledge-server/src/canvas/routes.rs` tests module (rename existing assertion intent)

**Context:** The frontend `stream.ts` dispatches skill nodes only on a `node` event (`stream.ts:116`) and treats `done` as "stop streaming." The backend currently sends skill nodes inside `done`, so nodes never land. Fix: for `/search`, `/image`, `/analyze`, emit a `node` event carrying `{node, x, y}` and then a terminal `done` with `{}`. Plain chat keeps its `done` with `{content}`.

- [ ] **Step 1: Add a helper + failing test**

The payload shape is unchanged; only the event name changes (which lives inside `async_stream`). Add a tiny pure helper so the event name is asserted in a unit test. Add near `build_skill_node_done_payload` (`routes.rs:209`):

```rust
/// The SSE event name that carries a freshly-built skill node to the client.
pub fn skill_node_event_name() -> &'static str {
    "node"
}
```

Add to the tests module:

```rust
    #[test]
    fn skill_node_uses_node_event_not_done() {
        assert_eq!(skill_node_event_name(), "node");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p knowledge-server skill_node_uses_node_event_not_done`
Expected: FAIL — `skill_node_event_name` is not yet defined (compile error) OR, once defined, this step is only meaningful after Step 1 is added; run it after adding the helper to confirm PASS, then proceed. (The real behavioral change is in Step 3.)

- [ ] **Step 3: Switch the three skill branches to a `node` event + terminal `done`**

In `chat_handler`'s `async_stream!` block, replace each of the three skill success emissions. For `ChatCommand::Search` (`routes.rs:447-456`):

```rust
                match run_search_skill(&stream_state, &query).await {
                    Ok(node) => {
                        yield Ok(
                            Event::default().event(skill_node_event_name()).data(
                                build_skill_node_done_payload(node, x, y).to_string(),
                            ),
                        );
                        yield Ok(Event::default().event("done").data("{}".to_string()));
                    }
                    Err(message) => yield Ok(sse_error(&message)),
                }
```

For `ChatCommand::Image` (`routes.rs:463-472`):

```rust
                match run_image_skill(&stream_state, &user_id, &prompt).await {
                    Ok(node) => {
                        yield Ok(
                            Event::default().event(skill_node_event_name()).data(
                                build_skill_node_done_payload(node, x, y).to_string(),
                            ),
                        );
                        yield Ok(Event::default().event("done").data("{}".to_string()));
                    }
                    Err(message) => yield Ok(sse_error(&message)),
                }
```

For `ChatCommand::Analyze` (`routes.rs:474-481`):

```rust
            ChatCommand::Analyze(prompt) => {
                let node = build_analyze_node(&prompt, &selected_ids);
                yield Ok(
                    Event::default()
                        .event(skill_node_event_name())
                        .data(build_skill_node_done_payload(node, x, y).to_string()),
                );
                yield Ok(Event::default().event("done").data("{}".to_string()));
            }
```

Leave the `ChatCommand::Plain` branch untouched (it still ends with `done` + `{content}`).

- [ ] **Step 4: Run tests + build**

Run: `cargo test -p knowledge-server canvas:: && cargo build -p knowledge-server`
Expected: PASS + clean build. (Streaming behavior itself is verified by build + the frontend tests in F5; the pure helpers cover the payload shape and event name.)

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/canvas/routes.rs
git commit -m "fix(canvas): emit skill results on node SSE event with terminal done"
```

---

## Task B3: Add an image re-run branch to the node-run endpoint (#3)

**Files:**
- Modify: `crates/knowledge-server/src/canvas/routes.rs:266-368` (`run_node_handler`) + a new payload builder near `routes.rs:201-211`
- Test: `crates/knowledge-server/src/canvas/routes.rs` tests module

**Context:** `run_node_handler` only ever builds an analyze prompt. The `ai_image` node's regenerate button posts to the same endpoint, so it must branch on node type. For an `ai_image` node it should read `data.prompt`, generate an image, persist it as an asset, and emit `done` with `{versionId, url, createdAt}` (mirroring the analyze `done` but with `url` instead of `content`). The frontend (F2) reads either field.

- [ ] **Step 1: Write the failing test for the image done payload builder**

Add to the tests module:

```rust
    #[test]
    fn image_done_payload_carries_url_and_version_fields() {
        let payload = build_image_done_payload("v-img", "/api/assets/abc", "2026-07-01T00:00:00Z");
        assert_eq!(payload["versionId"], "v-img");
        assert_eq!(payload["url"], "/api/assets/abc");
        assert_eq!(payload["createdAt"], "2026-07-01T00:00:00Z");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p knowledge-server image_done_payload_carries_url_and_version_fields`
Expected: FAIL — `build_image_done_payload` is not defined.

- [ ] **Step 3: Add the builder**

Add next to `build_analyze_done_payload` (`routes.rs:201`):

```rust
pub fn build_image_done_payload(version_id: &str, url: &str, created_at: &str) -> serde_json::Value {
    json!({ "versionId": version_id, "url": url, "createdAt": created_at })
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p knowledge-server image_done_payload_carries_url_and_version_fields`
Expected: PASS.

- [ ] **Step 5: Branch `run_node_handler` on node type**

The current handler (from `routes.rs:281`) builds the analyze prompt and streams. Wrap the analyze-specific work so image nodes take a different path. Replace the body from the `let node_prompt = ...` line (`routes.rs:281`) through the end of the analyze `event_stream` construction so that:

- both branches read `node.data.prompt`,
- `ai_image` nodes generate + persist + emit a single `done` with `build_image_done_payload`,
- every other type keeps the existing analyze behavior.

Concretely, keep everything up to and including obtaining `node` (`routes.rs:278-279`). Then replace `routes.rs:281-367` with:

```rust
    let node_prompt =
        node.data.get("prompt").and_then(|v| v.as_str()).unwrap_or("").to_string();

    if node.r#type == "ai_image" {
        if node_prompt.trim().is_empty() {
            return Err(ApiError::bad_request("image node has no prompt"));
        }
        let settings = load_query_settings(&state).await?;
        let provider = build_provider(&settings)?;
        let user_id = principal.user_id.clone();
        let prompt = node_prompt.clone();
        let stream_state = state.clone();
        let event_stream = async_stream::stream! {
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
                            build_image_done_payload(&version_id, &url, &created_at).to_string(),
                        ),
                    );
                }
                Err(message) => yield Ok(sse_error(&message)),
            }
        };
        // `provider` is validated above so misconfiguration fails before streaming.
        let _ = &provider;
        return Ok(Sse::new(event_stream).keep_alive(KeepAlive::default()));
    }

    // Note/URL/prior-analysis references.
    let mut blocks = crate::canvas::service::collect_reference_blocks(&doc, &node_id);

    // KB nodes -> RAG, with a per-project permission check; excluded if no access.
    for project_id in crate::canvas::service::referenced_kb_project_ids(&doc, &node_id) {
        let role = crate::tenancy::access::project_access_role(
            &state.pool,
            &project_id,
            &principal.user_id,
        )
        .await
        .map_err(ApiError::from)?;
        match role {
            Some(_role) => {
                let root =
                    crate::projects::service::project_root_for_id(&state, &project_id).await?;
                let assembled = crate::chat::context::assemble_chat_context(
                    &state,
                    &project_id,
                    &root,
                    &node_prompt,
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
    }

    let prompt = crate::canvas::service::build_analyze_prompt(&node_prompt, &blocks);

    let settings = load_query_settings(&state).await?;
    let provider = build_provider(&settings)?;
    let system_prompt = format!(
        "You are an analysis assistant reasoning over a knowledge canvas. Respond in {}. Use only the provided sources; if they are insufficient, say so plainly.",
        settings.language
    );
    let messages = vec![ProviderChatMessage { role: "user".to_string(), content: prompt }];

    let event_stream = async_stream::stream! {
        let mut full_text = String::new();
        match provider
            .stream_chat(ProviderChatStreamRequest { system_prompt, messages })
            .await
        {
            Ok(mut deltas) => {
                while let Some(delta) = deltas.next().await {
                    match delta {
                        Ok(text) => {
                            full_text.push_str(&text);
                            yield Ok(
                                Event::default()
                                    .event("delta")
                                    .data(json!({ "text": text }).to_string()),
                            );
                        }
                        Err(error) => {
                            yield Ok(sse_error(error.message()));
                            return;
                        }
                    }
                }
            }
            Err(error) => {
                yield Ok(sse_error(error.message()));
                return;
            }
        }

        let version_id = uuid::Uuid::new_v4().to_string();
        let created_at = now_rfc3339();
        yield Ok(
            Event::default().event("done").data(
                build_analyze_done_payload(&version_id, &full_text, &created_at).to_string(),
            ),
        );
    };

    Ok(Sse::new(event_stream).keep_alive(KeepAlive::default()))
```

Note: the `let _ = &provider;` line only exists to keep the early config validation meaningful for the image branch; if clippy objects, instead move `build_provider` validation before the `async_stream!` without binding, i.e. call `build_provider(&settings)?;` and drop the `_` line. Prefer the clean form: replace the two image-branch lines

```rust
        let provider = build_provider(&settings)?;
```
and
```rust
        let _ = &provider;
```
with a single
```rust
        build_provider(&settings)?;
```
placed before the `async_stream!` (validation only; the image skill builds its own provider internally via `run_image_skill`).

- [ ] **Step 6: Run tests + build**

Run: `cargo test -p knowledge-server canvas:: && cargo build -p knowledge-server`
Expected: PASS + clean build.

- [ ] **Step 7: Commit**

```bash
git add crates/knowledge-server/src/canvas/routes.rs
git commit -m "feat(canvas): add image re-run branch to node-run endpoint"
```

---

## Task F1: Autosave baseline so opening a canvas does not save (#5)

**Files:**
- Rewrite: `apps/admin/src/features/canvas/use-autosave.ts`
- Test: `apps/admin/src/features/canvas/use-autosave.test.ts` (replace)

**Context:** The page feeds `value: doc ?? emptyDoc()` and `emptyDoc()` is a fresh object every render; the old hook fired on any post-first-render value change, so the async load (emptyDoc -> loaded doc) triggered a save. Fix: compare a JSON serialization of the value against a `lastSaved` baseline; expose `reset(value)` so the page can seed the baseline when it loads a document. The hook effect must be registered before the page's load effect so it runs first (the page calls `reset` in the load effect, which runs after).

- [ ] **Step 1: Replace the tests**

Overwrite `use-autosave.test.ts`:

```ts
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAutosave } from "./use-autosave";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("useAutosave", () => {
  it("does not save the initial value", () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    renderHook(() => useAutosave({ value: { a: 1 }, delayMs: 100, onSave }));
    act(() => vi.advanceTimersByTime(200));
    expect(onSave).not.toHaveBeenCalled();
  });

  it("saves after the value changes and debounce elapses", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const { rerender } = renderHook(({ value }) => useAutosave({ value, delayMs: 100, onSave }), {
      initialProps: { value: { a: 1 } },
    });
    rerender({ value: { a: 2 } });
    await act(async () => {
      vi.advanceTimersByTime(100);
    });
    expect(onSave).toHaveBeenCalledWith({ a: 2 });
  });

  it("does not save when reset seeds a new baseline equal to the next value", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const { result, rerender } = renderHook(
      ({ value }) => useAutosave({ value, delayMs: 100, onSave }),
      { initialProps: { value: { a: 1 } } },
    );
    // Simulate a load: reset baseline to the loaded value, then rerender with it.
    act(() => result.current.reset({ a: 9 }));
    rerender({ value: { a: 9 } });
    await act(async () => {
      vi.advanceTimersByTime(200);
    });
    expect(onSave).not.toHaveBeenCalled();
  });

  it("does not save an unchanged value (same JSON)", () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const { rerender } = renderHook(({ value }) => useAutosave({ value, delayMs: 100, onSave }), {
      initialProps: { value: { a: 1 } },
    });
    rerender({ value: { a: 1 } });
    act(() => vi.advanceTimersByTime(200));
    expect(onSave).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- src/features/canvas/use-autosave`
Expected: FAIL — the old hook returns no `reset` and saves the loaded value.

- [ ] **Step 3: Rewrite the hook**

Overwrite `use-autosave.ts`:

```ts
import { useCallback, useEffect, useRef, useState } from "react";

export type SaveStatus = "idle" | "pending" | "saving" | "saved" | "error";

interface UseAutosaveOptions<T> {
  value: T;
  delayMs: number;
  onSave: (value: T) => Promise<unknown>;
}

export function useAutosave<T>({ value, delayMs, onSave }: UseAutosaveOptions<T>) {
  const [status, setStatus] = useState<SaveStatus>("idle");
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastSaved = useRef<string | null>(null);
  const latest = useRef(value);
  latest.current = value;

  const reset = useCallback((next: T) => {
    lastSaved.current = JSON.stringify(next);
  }, []);

  useEffect(() => {
    const serialized = JSON.stringify(value);
    if (lastSaved.current === null) {
      lastSaved.current = serialized;
      return;
    }
    if (serialized === lastSaved.current) {
      return;
    }
    setStatus("pending");
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      const snapshot = JSON.stringify(latest.current);
      setStatus("saving");
      onSave(latest.current)
        .then(() => {
          lastSaved.current = snapshot;
          setStatus("saved");
        })
        .catch(() => setStatus("error"));
    }, delayMs);
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, [value, delayMs, onSave]);

  return { status, reset };
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- src/features/canvas/use-autosave`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/canvas/use-autosave.ts apps/admin/src/features/canvas/use-autosave.test.ts
git commit -m "fix(canvas): autosave ignores load via JSON baseline and reset"
```

---

## Task F2: Frontend handles the image `done` payload (#3)

**Files:**
- Modify: `apps/admin/src/features/canvas/stream.ts:33-37` (`NodeRunHandlers`)
- Modify: `apps/admin/src/features/canvas/page.tsx:56-103` (`runNode`)
- Test: covered via `page.test.tsx` (F-level) — no new stream test required; `stream.test.ts` still passes since the wire shape is unchanged (JSON.parse of `done`).

**Context:** After B3, `done` for an analyze node is `{versionId, content, createdAt}` and for an image node is `{versionId, url, createdAt}`. `runNode` must build the right version object. The `ai_analyze` version uses `content`; the `ai_image` version uses `url`.

- [ ] **Step 1: Widen the done payload type**

In `stream.ts`, change `NodeRunHandlers` (`stream.ts:33`):

```ts
export interface NodeRunHandlers {
  onDelta: (text: string) => void;
  onDone: (payload: {
    versionId: string;
    createdAt: string;
    content?: string;
    url?: string;
  }) => void;
  onError: (message: string) => void;
}
```

- [ ] **Step 2: Build analyze-or-image versions in `runNode`**

In `page.tsx`, replace the `onDone` callback inside `runNode` (`page.tsx:63-98`) so the appended version carries `content` or `url` depending on the node type:

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
                    versions: [...versions, version],
                    activeVersionId: payload.versionId,
                  },
                };
              }),
            };
          });
        },
```

- [ ] **Step 3: Typecheck**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 4: Run existing stream + page tests**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- src/features/canvas/stream src/features/canvas/page`
Expected: PASS (no regressions).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/canvas/stream.ts apps/admin/src/features/canvas/page.tsx
git commit -m "feat(canvas): build image versions from node-run done payload"
```

---

## Task F3: URL Fetch calls the extract-url endpoint (#2)

**Files:**
- Modify: `apps/admin/src/features/canvas/canvas-board.tsx:28-56, 111-126` (`NodeCallbacks`, `UrlAdapter`, `rfNodes`)
- Modify: `apps/admin/src/features/canvas/page.tsx` (add `fetchUrlNode`, pass to board)
- Test: `apps/admin/src/features/canvas/page.test.tsx` — add a test that Fetch calls `extractUrl` and patches the node

**Context:** The URL node's Fetch is currently wired to the generic `onRun` (analyze). It must call `extractUrl(url)` and patch `{title, markdown, status, error}`. Add a dedicated `onFetchUrl` callback threaded like `onRun`.

- [ ] **Step 1: Write the failing page test**

The existing `page.test.tsx` mocks `./canvas-board`, `./chat-panel`, `./history-sidebar`, `./queries`. To test `fetchUrlNode`, capture the `onFetchUrl` handler the page passes to a real-ish board mock, and mock `./stream`'s `extractUrl`. Replace `page.test.tsx` with:

```tsx
import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const extractUrl = vi.fn();

vi.mock("./api", () => ({ extractUrl: (url: string) => extractUrl(url) }));
vi.mock("./history-sidebar", () => ({ HistorySidebar: () => <div>history</div> }));
vi.mock("./chat-panel", () => ({ ChatPanel: () => <div>chat</div> }));
vi.mock("./canvas-toolbar", () => ({ CanvasToolbar: () => <div>toolbar</div> }));

let boardProps: {
  onFetchUrl?: (id: string) => void;
} = {};
vi.mock("./canvas-board", () => ({
  CanvasBoard: (props: { onFetchUrl?: (id: string) => void }) => {
    boardProps = props;
    return <div>board</div>;
  },
}));

vi.mock("./queries", () => ({
  useCanvasList: () => ({ data: [], isLoading: false }),
  useCanvas: () => ({
    data: {
      id: "c1",
      title: "B",
      document: {
        nodes: [{ id: "u1", type: "url", x: 0, y: 0, w: 280, h: 160, data: { url: "https://x.test" } }],
        edges: [],
        viewport: { x: 0, y: 0, zoom: 1 },
      },
    },
  }),
  useCreateCanvas: () => ({ mutate: vi.fn() }),
  useSaveCanvas: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
  useDeleteCanvas: () => ({ mutate: vi.fn() }),
}));

import { CanvasPage } from "./page";

describe("CanvasPage", () => {
  it("renders the three panes", () => {
    render(<CanvasPage />);
    expect(screen.getByText("history")).toBeInTheDocument();
    expect(screen.getByText("board")).toBeInTheDocument();
    expect(screen.getByText("chat")).toBeInTheDocument();
  });

  it("fetches a url node through the extract endpoint", async () => {
    extractUrl.mockResolvedValue({ status: "ok", title: "Hello", markdown: "# hi", error: null });
    render(<CanvasPage />);
    await waitFor(() => expect(boardProps.onFetchUrl).toBeTypeOf("function"));
    boardProps.onFetchUrl?.("u1");
    await waitFor(() => expect(extractUrl).toHaveBeenCalledWith("https://x.test"));
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- src/features/canvas/page`
Expected: FAIL — the board mock never receives `onFetchUrl` (prop does not exist yet) and `./canvas-toolbar` module is not found.

Note: `./canvas-toolbar` is created in F7. If executing tasks strictly in order, temporarily the mock of a not-yet-created module still works in Vitest (the factory replaces the import), so this passes module resolution. If it does not, create an empty `canvas-toolbar.tsx` exporting `CanvasToolbar` first (F7 fills it in).

- [ ] **Step 3: Thread `onFetchUrl` through the board**

In `canvas-board.tsx`, extend `NodeCallbacks` (`:28`):

```tsx
interface NodeCallbacks {
  onPatch: (patch: Record<string, unknown>) => void;
  onRun: () => void;
  onFetchUrl: () => void;
}
```

Update the fallback in `callbacks` (`:33`):

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

Point `UrlAdapter`'s `onFetch` at `onFetchUrl` (`:47-56`):

```tsx
function UrlAdapter({ data }: NodeProps) {
  const cb = callbacks(data);
  return (
    <UrlNode
      data={data as unknown as UrlNodeData}
      onUrlChange={(url) => cb.onPatch({ url })}
      onFetch={cb.onFetchUrl}
    />
  );
}
```

Add `onFetchNode` to the board props and inject into `__cb` (`:92-126`):

```tsx
interface CanvasBoardProps {
  document: CanvasDocument;
  onChange: (next: CanvasDocument) => void;
  onRunNode: (nodeId: string) => void;
  onFetchUrl: (nodeId: string) => void;
}

export function CanvasBoard({ document, onChange, onRunNode, onFetchUrl }: CanvasBoardProps) {
```

In `rfNodes`' `__cb` object (`:119-122`), add:

```tsx
          __cb: {
            onPatch: (patch: Record<string, unknown>) => patchNode(n.id, patch),
            onRun: () => onRunNode(n.id),
            onFetchUrl: () => onFetchUrl(n.id),
          } satisfies NodeCallbacks,
```

and add `onFetchUrl` to the `useMemo` dependency array (`:125`): `[document.nodes, patchNode, onRunNode, onFetchUrl]`.

- [ ] **Step 4: Add `fetchUrlNode` to the page and pass it down**

In `page.tsx`, import `extractUrl`:

```tsx
import { extractUrl } from "./api";
```

Add the callback near `runNode`:

```tsx
  const fetchUrlNode = useCallback(
    (nodeId: string) => {
      const node = doc?.nodes.find((n) => n.id === nodeId);
      const url = typeof node?.data.url === "string" ? node.data.url : "";
      if (!url) {
        return;
      }
      patchNodeData(nodeId, { status: "loading", error: null });
      void extractUrl(url)
        .then((result) => {
          if (result.status === "ok") {
            patchNodeData(nodeId, {
              status: "idle",
              error: null,
              title: result.title,
              markdown: result.markdown,
            });
          } else {
            patchNodeData(nodeId, { status: "error", error: result.error ?? "fetch failed" });
          }
        })
        .catch((error: unknown) => {
          patchNodeData(nodeId, {
            status: "error",
            error: error instanceof Error ? error.message : "fetch failed",
          });
        });
    },
    [doc, patchNodeData],
  );
```

Pass it to the board (`page.tsx:126`):

```tsx
            <CanvasBoard document={doc} onChange={setDoc} onRunNode={runNode} onFetchUrl={fetchUrlNode} />
```

- [ ] **Step 5: Typecheck + run tests**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npx tsc --noEmit && npm test -- src/features/canvas/page`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/features/canvas/canvas-board.tsx apps/admin/src/features/canvas/page.tsx apps/admin/src/features/canvas/page.test.tsx
git commit -m "fix(canvas): url fetch calls extract-url endpoint and patches node"
```

---

## Task F4: Wire selection + create reference edges for `/analyze` (#4)

**Files:**
- Modify: `apps/admin/src/features/canvas/canvas-board.tsx` (report selection)
- Modify: `apps/admin/src/features/canvas/page.tsx` (`selectedNodeIds` state; edges in `addSkillNode`)
- Test: `apps/admin/src/features/canvas/page.test.tsx` — extend with an `addSkillNode` edge assertion via a chat-panel mock that captures `onSkillNode`

**Context:** `page.tsx:134` passes `selectedNodeIds={[]}`. The board must report selected node ids up; the page holds them and passes them to `ChatPanel`. Separately, `/analyze` returns a node with `data.sourceNodeIds`; `addSkillNode` must create reference edges `source -> newNode` for each id so `run_node_handler`'s incoming-edge collection sees them.

- [ ] **Step 1: Extend the page test to assert reference edges**

Update the `./chat-panel` and `./canvas-board` mocks in `page.test.tsx` to capture callbacks, and add a test. Replace the `./chat-panel` mock and add capture:

```tsx
let chatProps: {
  selectedNodeIds?: string[];
  onSkillNode?: (payload: { node: unknown; x: number; y: number }) => void;
} = {};
vi.mock("./chat-panel", () => ({
  ChatPanel: (props: {
    selectedNodeIds?: string[];
    onSkillNode?: (p: { node: unknown; x: number; y: number }) => void;
  }) => {
    chatProps = props;
    return <div>chat</div>;
  },
}));
```

Extend the board mock to also capture `onSelectionChange` and `document`:

```tsx
let boardProps: {
  onFetchUrl?: (id: string) => void;
  onSelectionChange?: (ids: string[]) => void;
  document?: { nodes: unknown[]; edges: { source: string; target: string }[] };
  onChange?: (doc: unknown) => void;
} = {};
vi.mock("./canvas-board", () => ({
  CanvasBoard: (props: typeof boardProps) => {
    boardProps = props;
    return <div>board</div>;
  },
}));
```

Add the test:

```tsx
  it("creates reference edges when an analyze skill node references selected nodes", async () => {
    render(<CanvasPage />);
    await waitFor(() => expect(chatProps.onSkillNode).toBeTypeOf("function"));
    chatProps.onSkillNode?.({
      node: {
        type: "ai_analyze",
        data: { prompt: "sum", sourceNodeIds: ["u1"] },
      },
      x: 0,
      y: 0,
    });
    await waitFor(() => {
      const edges = boardProps.document?.edges ?? [];
      expect(edges.some((e) => e.source === "u1")).toBe(true);
    });
  });
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- src/features/canvas/page`
Expected: FAIL — `addSkillNode` currently appends the node with no edges.

- [ ] **Step 3: Report selection from the board**

In `canvas-board.tsx`, add to props and wire xyflow's `onSelectionChange`:

```tsx
interface CanvasBoardProps {
  document: CanvasDocument;
  onChange: (next: CanvasDocument) => void;
  onRunNode: (nodeId: string) => void;
  onFetchUrl: (nodeId: string) => void;
  onSelectionChange?: (nodeIds: string[]) => void;
}
```

Add a memoized handler and pass to `<ReactFlow>`:

```tsx
  const handleSelectionChange = useCallback(
    ({ nodes }: { nodes: Node[] }) => {
      onSelectionChange?.(nodes.map((n) => n.id));
    },
    [onSelectionChange],
  );
```

On the `<ReactFlow>` element add: `onSelectionChange={handleSelectionChange}`.

- [ ] **Step 4: Hold selection + build edges in the page**

In `page.tsx`, add state:

```tsx
  const [selectedNodeIds, setSelectedNodeIds] = useState<string[]>([]);
```

Rewrite `addSkillNode` (`page.tsx:105-117`) to also create reference edges from `sourceNodeIds`:

```tsx
  const addSkillNode = useCallback((payload: SkillNodePayload) => {
    const source = payload.node as {
      type: CanvasNode["type"];
      data?: Record<string, unknown>;
    };
    const id = crypto.randomUUID();
    setDoc((prev) => {
      if (!prev) {
        return prev;
      }
      const position = nextNodePosition(prev);
      const node: CanvasNode = {
        id,
        type: source.type,
        x: position.x,
        y: position.y,
        w: 280,
        h: 160,
        data: source.data ?? {},
      };
      const sourceIds = Array.isArray(source.data?.sourceNodeIds)
        ? (source.data?.sourceNodeIds as unknown[]).filter(
            (s): s is string => typeof s === "string",
          )
        : [];
      const newEdges = sourceIds.map((src) => ({
        id: crypto.randomUUID(),
        source: src,
        target: id,
      }));
      return { ...prev, nodes: [...prev.nodes, node], edges: [...prev.edges, ...newEdges] };
    });
  }, []);
```

Pass selection to the board and chat, and the selection setter (`page.tsx:126, 134`):

```tsx
            <CanvasBoard
              document={doc}
              onChange={setDoc}
              onRunNode={runNode}
              onFetchUrl={fetchUrlNode}
              onSelectionChange={setSelectedNodeIds}
            />
```

```tsx
      <ChatPanel canvasId={canvasId ?? ""} selectedNodeIds={selectedNodeIds} onSkillNode={addSkillNode} />
```

`nextNodePosition` is defined in F6 Step 3. If executing F4 before F6, add a temporary stub at the bottom of `page.tsx`:

```tsx
function nextNodePosition(doc: CanvasDocument): { x: number; y: number } {
  const n = doc.nodes.length;
  return { x: 80 + (n % 6) * 48, y: 80 + (n % 6) * 48 };
}
```

(F6 keeps this exact function, so no rework is needed.)

- [ ] **Step 5: Typecheck + run tests**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npx tsc --noEmit && npm test -- src/features/canvas/page`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/features/canvas/canvas-board.tsx apps/admin/src/features/canvas/page.tsx apps/admin/src/features/canvas/page.test.tsx
git commit -m "feat(canvas): wire node selection and reference edges for analyze"
```

---

## Task F5: Chat panel handles skill nodes and avoids a dangling bubble (#1)

**Files:**
- Modify: `apps/admin/src/features/canvas/chat-panel.tsx:59-88` (`submit`)
- Test: `apps/admin/src/features/canvas/chat-panel.test.tsx` — add a test that a skill submit calls `onSkillNode`

**Context:** After B2, skill results arrive on the `node` event (already routed to `onNode` in `stream.ts`). The chat panel must, for a skill command, call `onSkillNode` and show a short confirmation instead of leaving an empty assistant bubble that only fills from `delta` events (skills emit none).

- [ ] **Step 1: Add the failing test**

Append to `chat-panel.test.tsx`. Mock `./stream` so `streamCanvasChat` invokes `onNode` then `onDone`:

```tsx
import { vi } from "vitest";

vi.mock("./stream", () => ({
  streamCanvasChat: vi.fn(
    async (
      _canvasId: string,
      _message: string,
      _ids: string[],
      handlers: {
        onNode?: (p: { node: unknown; x: number; y: number }) => void;
        onDone: (p: unknown) => void;
      },
    ) => {
      handlers.onNode?.({ node: { type: "note", data: {} }, x: 0, y: 0 });
      handlers.onDone({});
    },
  ),
}));

describe("ChatPanel skill submit", () => {
  it("calls onSkillNode when a search skill returns a node", async () => {
    const onSkillNode = vi.fn();
    render(<ChatPanel canvasId="c1" selectedNodeIds={[]} onSkillNode={onSkillNode} />);
    const input = screen.getByPlaceholderText("/ 或提问");
    fireEvent.change(input, { target: { value: "/search cats" } });
    fireEvent.submit(input.closest("form")!);
    await waitFor(() => expect(onSkillNode).toHaveBeenCalledTimes(1));
  });
});
```

Add `waitFor` to the testing-library import at the top of the file.

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- src/features/canvas/chat-panel`
Expected: FAIL — current `submit` wires `onNode: (payload) => onSkillNode(payload)` already, so this may PASS for the call count; but the assistant bubble remains "…". To make the test meaningful and drive the confirmation behavior, also assert the confirmation:

```tsx
    await waitFor(() => expect(screen.getByText("已在画布中添加节点。")).toBeInTheDocument());
```

With that added assertion, Step 2 FAILS because no confirmation text is rendered yet.

- [ ] **Step 3: Update `submit` to confirm skill nodes**

In `chat-panel.tsx`, replace the streaming call inside `submit` (`:75-84`) so a `node` result also writes a confirmation into the assistant bubble:

```tsx
    try {
      await streamCanvasChat(canvasId, text, selectedNodeIds, {
        onDelta: (delta) => appendAssistantDelta(assistantId, delta),
        onNode: (payload) => {
          onSkillNode(payload);
          setMessages((prev) =>
            prev.map((message) =>
              message.id === assistantId
                ? { ...message, content: "已在画布中添加节点。" }
                : message,
            ),
          );
        },
        onDone: () => setStreaming(false),
        onError: (message) => {
          appendAssistantDelta(assistantId, `\n\n> 出错了：${message}`);
          setStreaming(false);
        },
      });
    } finally {
      setStreaming(false);
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- src/features/canvas/chat-panel`
Expected: PASS (slash menu test + skill submit test).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/canvas/chat-panel.tsx apps/admin/src/features/canvas/chat-panel.test.tsx
git commit -m "fix(canvas): chat panel adds skill node and confirms in thread"
```

---

## Task F6: Round-trip viewport + place skill nodes sensibly (#6)

**Files:**
- Modify: `apps/admin/src/features/canvas/canvas-board.tsx` (`defaultViewport`, `onMoveEnd`, drop unconditional `fitView`)
- Modify: `apps/admin/src/features/canvas/page.tsx` (keep `nextNodePosition`; call `reset` on load)
- Test: `apps/admin/src/features/canvas/canvas-board.test.tsx` — assert viewport persists on move

**Context:** `document.viewport` is never read or written. Restore it via `defaultViewport` and persist it on `onMoveEnd`. Skill nodes must not stack at (0,0): the page assigns positions via `nextNodePosition` (introduced in F4). Also seed the autosave baseline on load via the `reset` from F1 so viewport restoration + load do not trigger a save.

- [ ] **Step 1: Extend the board test**

The existing `canvas-board.test.tsx` mocks `@xyflow/react`. Extend the mock so `ReactFlow` exposes `onMoveEnd` via a button, and assert `onChange` receives the new viewport. Replace `canvas-board.test.tsx`:

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@xyflow/react/dist/style.css", () => ({}));
vi.mock("@xyflow/react", () => ({
  ReactFlow: (props: {
    onMoveEnd?: (e: unknown, vp: { x: number; y: number; zoom: number }) => void;
  }) => (
    <div>
      <button type="button" onClick={() => props.onMoveEnd?.(null, { x: 5, y: 6, zoom: 2 })}>
        move
      </button>
      flow
    </div>
  ),
  ReactFlowProvider: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Background: () => null,
  Controls: () => null,
  addEdge: (c: unknown, edges: unknown[]) => edges,
  applyEdgeChanges: (_c: unknown, edges: unknown[]) => edges,
  applyNodeChanges: (_c: unknown, nodes: unknown[]) => nodes,
}));

import { CanvasBoard } from "./canvas-board";

const emptyDoc = { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } };

describe("CanvasBoard", () => {
  it("renders the flow", () => {
    render(
      <CanvasBoard
        document={emptyDoc}
        onChange={() => {}}
        onRunNode={() => {}}
        onFetchUrl={() => {}}
      />,
    );
    expect(screen.getByText("flow")).toBeInTheDocument();
  });

  it("persists viewport on move end", () => {
    const onChange = vi.fn();
    render(
      <CanvasBoard
        document={emptyDoc}
        onChange={onChange}
        onRunNode={() => {}}
        onFetchUrl={() => {}}
      />,
    );
    fireEvent.click(screen.getByText("move"));
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({ viewport: { x: 5, y: 6, zoom: 2 } }),
    );
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- src/features/canvas/canvas-board`
Expected: FAIL — board has no `onMoveEnd`; and the mock's `move` button calls an undefined prop.

- [ ] **Step 3: Persist + restore viewport in the board**

In `canvas-board.tsx`, import `Viewport` type usage is not required; add a move handler and wire props on `<ReactFlow>`. Add:

```tsx
  const onMoveEnd = useCallback(
    (_event: unknown, viewport: { x: number; y: number; zoom: number }) => {
      onChange({ ...document, viewport: { x: viewport.x, y: viewport.y, zoom: viewport.zoom } });
    },
    [document, onChange],
  );
```

On the `<ReactFlow>` element, replace `fitView` with viewport control:

```tsx
        <ReactFlow
          nodes={rfNodes}
          edges={rfEdges}
          nodeTypes={nodeTypes}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          onSelectionChange={handleSelectionChange}
          defaultViewport={document.viewport}
          onMoveEnd={onMoveEnd}
        >
```

- [ ] **Step 4: Keep `nextNodePosition` + seed autosave baseline on load**

Confirm `nextNodePosition` exists at the bottom of `page.tsx` (added in F4). Then update the load effect and autosave call in `page.tsx` to use `reset` (from F1):

```tsx
  const { status, reset } = useAutosave({ value: doc ?? emptyDoc(), delayMs: 800, onSave });

  useEffect(() => {
    if (canvas.data && loadedId.current !== canvas.data.id) {
      loadedId.current = canvas.data.id;
      setDoc(canvas.data.document);
      setTitle(canvas.data.title);
      reset(canvas.data.document);
    }
  }, [canvas.data, reset]);
```

- [ ] **Step 5: Typecheck + run board + page + autosave tests**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npx tsc --noEmit && npm test -- src/features/canvas/canvas-board src/features/canvas/page src/features/canvas/use-autosave`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/features/canvas/canvas-board.tsx apps/admin/src/features/canvas/page.tsx apps/admin/src/features/canvas/canvas-board.test.tsx
git commit -m "feat(canvas): round-trip viewport and place skill nodes without overlap"
```

---

## Task F7: Reachable toolbar to add Note / URL / KB nodes (#7)

**Files:**
- Create: `apps/admin/src/features/canvas/kb-picker.tsx` (moved `KbProjectPicker`)
- Modify: `apps/admin/src/features/canvas/chat-panel.tsx` (import `KbProjectPicker` from `./kb-picker`)
- Create: `apps/admin/src/features/canvas/canvas-toolbar.tsx`
- Modify: `apps/admin/src/features/canvas/page.tsx` (render `CanvasToolbar`)
- Test: `apps/admin/src/features/canvas/canvas-toolbar.test.tsx`

**Context:** There is no UI to add plain note/url/kb nodes; only slash skills exist. Add a toolbar with three buttons. Note/URL add empty nodes via the page's `addSkillNode` (which already handles placement). KB opens a project picker dialog reusing the existing picker, which is currently private inside `chat-panel.tsx` — extract it so both callers share one component.

- [ ] **Step 1: Extract `KbProjectPicker` into its own file**

Create `kb-picker.tsx` with the exact component currently in `chat-panel.tsx:166-198`:

```tsx
import { Button } from "@/components/ui/button";

import { useProjectsQuery } from "../projects/queries";

interface KbProjectPickerProps {
  onPick: (project: { id: string; name: string }) => void;
}

export function KbProjectPicker({ onPick }: KbProjectPickerProps) {
  const projects = useProjectsQuery();
  const items = projects.data ?? [];

  if (projects.isLoading) {
    return <p className="text-sm text-muted-foreground">加载项目中…</p>;
  }
  if (items.length === 0) {
    return <p className="text-sm text-muted-foreground">没有可用的知识库。</p>;
  }
  return (
    <div className="space-y-2">
      <ul className="max-h-64 space-y-1 overflow-auto">
        {items.map((project) => (
          <li key={project.id}>
            <Button
              type="button"
              variant="ghost"
              className="w-full justify-start"
              onClick={() => onPick(project)}
            >
              {project.name}
            </Button>
          </li>
        ))}
      </ul>
    </div>
  );
}
```

In `chat-panel.tsx`: delete the inline `KbProjectPickerProps` interface + `KbProjectPicker` function (`:166-198`) and the now-unused `useProjectsQuery` and `Button` imports if they are not otherwise used (verify with tsc). Add `import { KbProjectPicker } from "./kb-picker";`.

- [ ] **Step 2: Write the failing toolbar test**

Create `canvas-toolbar.test.tsx`:

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("./kb-picker", () => ({ KbProjectPicker: () => <div>kb-picker</div> }));

import { CanvasToolbar } from "./canvas-toolbar";

describe("CanvasToolbar", () => {
  it("adds a note node", () => {
    const onAdd = vi.fn();
    render(<CanvasToolbar onAdd={onAdd} />);
    fireEvent.click(screen.getByRole("button", { name: "笔记" }));
    expect(onAdd).toHaveBeenCalledWith({ node: { type: "note", data: {} }, x: 0, y: 0 });
  });

  it("opens the kb picker dialog", () => {
    render(<CanvasToolbar onAdd={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "知识库" }));
    expect(screen.getByText("kb-picker")).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- src/features/canvas/canvas-toolbar`
Expected: FAIL — `canvas-toolbar.tsx` does not exist.

- [ ] **Step 4: Implement the toolbar**

Create `canvas-toolbar.tsx`. It reuses the `SkillNodePayload` shape so the page's `addSkillNode` handles placement + edges uniformly:

```tsx
import { Database, FileText, Globe } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";

import type { SkillNodePayload } from "./chat-panel";
import { KbProjectPicker } from "./kb-picker";

interface CanvasToolbarProps {
  onAdd: (payload: SkillNodePayload) => void;
}

export function CanvasToolbar({ onAdd }: CanvasToolbarProps) {
  const [kbOpen, setKbOpen] = useState(false);
  return (
    <div className="flex items-center gap-1">
      <Button
        type="button"
        size="sm"
        variant="outline"
        onClick={() => onAdd({ node: { type: "note", data: {} }, x: 0, y: 0 })}
      >
        <FileText className="size-3" />
        笔记
      </Button>
      <Button
        type="button"
        size="sm"
        variant="outline"
        onClick={() => onAdd({ node: { type: "url", data: {} }, x: 0, y: 0 })}
      >
        <Globe className="size-3" />
        网页
      </Button>
      <Button type="button" size="sm" variant="outline" onClick={() => setKbOpen(true)}>
        <Database className="size-3" />
        知识库
      </Button>
      {kbOpen ? (
        <Dialog open onOpenChange={setKbOpen}>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>选择知识库</DialogTitle>
            </DialogHeader>
            <KbProjectPicker
              onPick={(project) => {
                onAdd({
                  node: { type: "kb", data: { projectId: project.id, projectName: project.name } },
                  x: 0,
                  y: 0,
                });
                setKbOpen(false);
              }}
            />
          </DialogContent>
        </Dialog>
      ) : null}
    </div>
  );
}
```

- [ ] **Step 5: Render the toolbar in the page header**

In `page.tsx`, import and render `CanvasToolbar` in the header row so it is reachable whenever a canvas is open. Add `import { CanvasToolbar } from "./canvas-toolbar";`. Update `CanvasHeader` usage (`page.tsx:123`) to include the toolbar, passing `addSkillNode`:

```tsx
        <CanvasHeader
          title={title}
          status={status}
          onRetry={() => doc && void onSave(doc)}
          actions={doc ? <CanvasToolbar onAdd={addSkillNode} /> : null}
        />
```

Extend `CanvasHeaderProps` and render `actions` (`page.tsx:147-170`):

```tsx
interface CanvasHeaderProps {
  title: string;
  status: SaveStatus;
  onRetry: () => void;
  actions?: React.ReactNode;
}

function CanvasHeader({ title, status, onRetry, actions }: CanvasHeaderProps) {
  const label = statusLabels[status];
  return (
    <header className="flex items-center justify-between gap-2 border-b px-4 py-2">
      <span className="truncate text-sm font-semibold">{title || "未命名画布"}</span>
      <div className="flex items-center gap-3">
        {actions}
        {label.text ? (
          <button
            type="button"
            onClick={status === "error" ? onRetry : undefined}
            disabled={status !== "error"}
            className={cn("text-xs", label.className, status !== "error" && "cursor-default")}
          >
            {label.text}
          </button>
        ) : null}
      </div>
    </header>
  );
}
```

Add `import type { ReactNode } from "react";` (or use `React.ReactNode` if `React` is imported). Since the file imports named hooks from `react`, add `ReactNode` to that import and use `actions?: ReactNode`.

- [ ] **Step 6: Typecheck + run the affected tests**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npx tsc --noEmit && npm test -- src/features/canvas`
Expected: PASS (all canvas tests, including chat-panel after the picker extraction).

- [ ] **Step 7: Commit**

```bash
git add apps/admin/src/features/canvas/kb-picker.tsx apps/admin/src/features/canvas/canvas-toolbar.tsx apps/admin/src/features/canvas/canvas-toolbar.test.tsx apps/admin/src/features/canvas/chat-panel.tsx apps/admin/src/features/canvas/page.tsx
git commit -m "feat(canvas): add reachable toolbar to create note/url/kb nodes"
```

---

## Final Verification

- [ ] **Frontend:** `cd /e/Projects/Js/knowledge/apps/admin && npx tsc --noEmit && npm test -- src/features/canvas` — clean typecheck, all canvas tests green.
- [ ] **Backend:** `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server && cargo build -p knowledge-server` — all green.
- [ ] **Manual smoke (if a dev server is available):** create a canvas; add a Note and a URL via the toolbar; Fetch the URL (expect title+markdown); connect Note+URL into an `/analyze` node and run it (expect analysis using both as sources); run `/search` and `/image` (expect nodes to appear); pan/zoom then reload (expect viewport restored); open an existing canvas and confirm no save fires until an edit.

## Won't-fix note (finding #8)

Chinese UI copy in the canvas feature is intentional and consistent with every sibling admin screen (settings, deep-research, api-tokens, graph) and with this plan's own prescribed literals. The "ASCII-only in all files" guidance governs code, comments, and identifiers — not user-facing display strings. No change.

## Self-Review

- **Spec coverage:** every finding #1-#7 maps to at least one task (see table); #8 is an explicit won't-fix.
- **Type consistency:** `onFetchUrl` (board prop) / `onFetchUrl` (`NodeCallbacks`) / `fetchUrlNode` (page) are consistent; `onSelectionChange` used on board + page; `reset` returned by `useAutosave` and consumed in the page load effect; `nextNodePosition` defined once (F4) and reused (F6); `NodeRunHandlers.onDone` widened once (F2) and consumed in `runNode`; `build_image_done_payload` defined (B3) and used in the image branch; `skill_node_event_name` defined (B2) and used in all three skill branches.
- **Placeholder scan:** all steps contain concrete code; the only cross-task dependency (`nextNodePosition`, `./canvas-toolbar` mock) is called out with exact stubs so tasks can run in order.
