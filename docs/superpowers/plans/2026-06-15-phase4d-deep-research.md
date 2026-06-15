# Phase 4d — Deep Research Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port upstream's deep-research feature (`upstream_llm_wiki/src/lib/deep-research.ts`) to the server as a queued task type `project.deep_research`. Given a `topic` and one or more `searchQueries`, the executor fan-outs queries to the configured web-search provider (Phase 4c), feeds the deduplicated source list to the LLM synthesizer (using the existing OpenAI-compatible provider), and saves the resulting wiki page at `wiki/queries/research-<slug>.md`. An admin Deep Research tab kicks off the task and surfaces its outcome.

**Architecture:** Three layers compose existing primitives. (1) `knowledge-core/src/project/research.rs` adds `render_research_page(input) -> String` — the page renderer ported verbatim from upstream lines 285-302, plus a `<think>` block stripper ported from lines 279-284. (2) `knowledge-server/src/deep_research/` contains the source collector (`collect_research_sources` — port of upstream lines 61-119), the executor `run_deep_research_executor` (orchestrates collect → synthesize → save), and the POST route `/api/projects/{project_id}/deep-research`. (3) `apps/admin/src/features/deep-research/` adds a project tab with a topic + queries form and a list of recent `project.deep_research` tasks.

**Tech Stack:** Rust (axum 0.8, sqlx, reqwest via Phase 4c web_search, the existing task queue, `time`, `uuid`). React 19 + TanStack Query + Zod, vitest. Playwright for the e2e (uses the existing SearXNG mock from Phase 4c + the existing chat mock from Phase 3b).

**Upstream references (memory rule: port, don't invent):**
- `upstream_llm_wiki/src/lib/deep-research.ts:61-119` — `collectResearchSources`: dedupe by lowercased URL or `source:title:snippet`, cap at `MAX_RESEARCH_SOURCES = 20`, run all queries against the resolved web search provider in parallel via `Promise.allSettled`, collect errors. Port verbatim.
- `upstream_llm_wiki/src/lib/deep-research.ts:215-243` — the synthesis system prompt and user message. Port verbatim (excluding the `wikiIndex` cross-referencing block — we keep it; load `wiki/index.md` server-side just like upstream does).
- `upstream_llm_wiki/src/lib/deep-research.ts:279-302` — page renderer + `<think>` block stripping regex. Port verbatim, including the unclosed-think-block fallback.
- `upstream_llm_wiki/src/lib/deep-research.ts:285-302` — frontmatter shape: `type: query`, `title: "Research: <topic>"`, `created: <date>`, `origin: deep-research`, `tags: [research]`. Port verbatim.

**Documented divergences from upstream:**
1. **No streaming.** Upstream streams tokens to a research panel in the desktop UI. The server runs the synthesizer via `provider.complete_text(...)` (single non-streaming call) inside the task queue. Result is persisted on task completion. Reason: the existing task queue model doesn't have a streaming progress channel, and adding one only for one task type isn't worth the complexity. The UI polls task status like every other long-running task.
2. **No auto-ingest after save.** Upstream calls `autoIngest` after the page is written so it gets graph entries / cross-references. We skip that — the user can trigger a separate ingest task on the saved file. Reason: keeps Phase 4d self-contained. (Future enhancement: have the executor optionally enqueue an `project.ingest_source` task on the saved file.)
3. **No `is-active-project` checks.** Upstream gates every update with `isActiveProjectPath` because the desktop UI can switch between projects mid-research. The server runs all queued tasks regardless of which project the admin happens to be viewing.
4. **No `optimizeResearchTopic`.** That LLM call (upstream `optimize-research-topic.ts`) generates queries from a knowledge-gap. Our route accepts `topic + searchQueries[]` directly; if `searchQueries` is omitted or empty, the executor falls back to `[topic]` (matches upstream's queryless fallback at line 178). Adding topic optimization is a future enhancement.
5. **No `anyTxt` local-search source.** That's a separate Windows-only desktop integration. Web only — same gate as Phase 4c.
6. **`wiki/index.md` is updated.** Same way `save_query_page` already does for ordinary query results (`crates/knowledge-core/src/project/queries.rs:91-111`) — append to a `## Queries` section so the saved research page is at least visible in the index. Upstream relies on auto-ingest to do this; we do it directly.
7. **The `wiki_index` context passed to the synthesizer is bounded.** Upstream sends the entire `wiki/index.md` verbatim. We truncate to the first 8 KiB to bound the prompt size for large wikis. The truncation is logged once per task at `tracing::debug` so it's visible during testing.

**Indentation conventions:** `crates/**` source = 2-space EXCEPT `crates/knowledge-server/src/projects/routes.rs` and `crates/knowledge-server/src/auth/routes.rs` = mixed (follow the file). `tests/rust-integration/**` = 4-space. All TS/TSX = 2-space.

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `crates/knowledge-core/src/project/research.rs` | Create | `RenderResearchPageInput` + `render_research_page` + `clean_synthesis_thinking` + 3 unit tests |
| `crates/knowledge-core/src/project/mod.rs` | Modify | Register `pub mod research;` |
| `crates/knowledge-server/src/deep_research/mod.rs` | Create | Re-exports + `ResearchSource` type |
| `crates/knowledge-server/src/deep_research/sources.rs` | Create | `collect_research_sources` (web_search fan-out, dedup, cap) + 2 unit tests |
| `crates/knowledge-server/src/deep_research/routes.rs` | Create | `POST /api/projects/{project_id}/deep-research` handler |
| `crates/knowledge-server/src/lib.rs` | Modify | Register `pub mod deep_research;` |
| `crates/knowledge-server/src/http/router.rs` | Modify | Merge `deep_research::routes::router()` |
| `crates/knowledge-server/src/tasks/executors.rs` | Modify | Add `project.deep_research` dispatch arm + `run_deep_research_executor` |
| `tests/rust-integration/tests/support/mock_openai.rs` | Modify | Add `MockScenario::DeepResearchSuccess` |
| `tests/rust-integration/tests/deep_research_api.rs` | Create | End-to-end: config provider + SearXNG mock → enqueue → wait → assert savedPath + file content |
| `apps/admin/src/features/shared/api.ts` | Modify | `createDeepResearchTask` + zod schema |
| `apps/admin/src/features/deep-research/queries.ts` | Create | Hook for the mutation |
| `apps/admin/src/features/deep-research/page.tsx` | Create | Form + list of recent deep_research tasks |
| `apps/admin/src/features/deep-research/page.test.tsx` | Create | vitest for form submission and list rendering |
| `apps/admin/src/app/router.tsx` | Modify | Add `<Route path="deep-research" element={<DeepResearchPage />} />` inside the project workspace |
| `apps/admin/src/lib/route-meta.ts` | Modify | Add `{ suffix: "/deep-research", label: "Deep Research" }` |
| `tests/web/tests/deep-research.spec.ts` | Create | e2e: configure SearXNG → create project → submit research → expect file in Files tab |

---

### Task 1: Page renderer + thinking-tag stripping

**Files:**
- Create: `crates/knowledge-core/src/project/research.rs`
- Modify: `crates/knowledge-core/src/project/mod.rs`

Upstream reference: `upstream_llm_wiki/src/lib/deep-research.ts:279-302` (frontmatter shape + `<think>` regex). The renderer is pure (no I/O) — ideal for unit tests.

- [ ] **Step 1: Register the module**

In `crates/knowledge-core/src/project/mod.rs`, add (alphabetical):

```rust
pub mod research;
```

- [ ] **Step 2: Write the failing tests**

Create `crates/knowledge-core/src/project/research.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn clean_synthesis_thinking_strips_closed_blocks() {
    let raw = "<think>internal monologue</think>\n\nActual answer.";
    assert_eq!(clean_synthesis_thinking(raw), "Actual answer.");
  }

  #[test]
  fn clean_synthesis_thinking_strips_thinking_blocks_case_insensitive() {
    let raw = "<Thinking>plan</Thinking>\n\nThe answer.";
    assert_eq!(clean_synthesis_thinking(raw), "The answer.");
  }

  #[test]
  fn clean_synthesis_thinking_strips_unclosed_block_at_end() {
    let raw = "Real content.\n\n<think>tail thoughts that never closed";
    assert_eq!(clean_synthesis_thinking(raw), "Real content.\n\n");
  }

  #[test]
  fn render_research_page_emits_upstream_frontmatter_and_references() {
    let input = RenderResearchPageInput {
      topic: "Knowledge Graphs".to_string(),
      slug: "knowledge-graphs".to_string(),
      date: "2026-06-15".to_string(),
      synthesis: "Synthesized text".to_string(),
      references: vec![
        RenderResearchReference {
          title: "Intro".to_string(),
          url: "https://example.com/intro".to_string(),
          source: "example.com".to_string(),
        },
        RenderResearchReference {
          title: "Deep dive".to_string(),
          url: "https://other.example.com/d".to_string(),
          source: "other.example.com".to_string(),
        },
      ],
    };
    let rendered = render_research_page(&input);
    assert!(rendered.starts_with("---\n"));
    assert!(rendered.contains("type: query"));
    assert!(rendered.contains("title: \"Research: Knowledge Graphs\""));
    assert!(rendered.contains("created: 2026-06-15"));
    assert!(rendered.contains("origin: deep-research"));
    assert!(rendered.contains("tags: [research]"));
    assert!(rendered.contains("# Research: Knowledge Graphs"));
    assert!(rendered.contains("Synthesized text"));
    assert!(rendered.contains("## References"));
    assert!(rendered.contains("1. [Intro](https://example.com/intro) — example.com"));
    assert!(rendered.contains("2. [Deep dive](https://other.example.com/d) — other.example.com"));
  }

  #[test]
  fn render_research_page_escapes_double_quotes_in_topic() {
    let input = RenderResearchPageInput {
      topic: r#"What is "RAG"?"#.to_string(),
      slug: "rag".to_string(),
      date: "2026-06-15".to_string(),
      synthesis: "Body.".to_string(),
      references: vec![],
    };
    let rendered = render_research_page(&input);
    assert!(rendered.contains(r#"title: "Research: What is \"RAG\"?""#));
  }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p knowledge-core research`
Expected: FAIL — `cannot find function clean_synthesis_thinking`.

- [ ] **Step 4: Implement the renderer**

Prepend to `crates/knowledge-core/src/project/research.rs`:

```rust
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct RenderResearchReference {
  pub title: String,
  pub url: String,
  pub source: String,
}

#[derive(Debug, Clone)]
pub struct RenderResearchPageInput {
  pub topic: String,
  pub slug: String,
  pub date: String,
  pub synthesis: String,
  pub references: Vec<RenderResearchReference>,
}

/// Ported from deep-research.ts:279-283. Strips `<think>...</think>` and
/// `<thinking>...</thinking>` blocks, case-insensitive, including an
/// unclosed block at the end (model truncated mid-thought).
pub fn clean_synthesis_thinking(raw: &str) -> String {
  static CLOSED: OnceLock<Regex> = OnceLock::new();
  static OPEN_END: OnceLock<Regex> = OnceLock::new();

  let closed = CLOSED.get_or_init(|| {
    Regex::new(r"(?is)<think(?:ing)?>\s*.*?</think(?:ing)?>\s*").unwrap()
  });
  let open_end = OPEN_END.get_or_init(|| {
    Regex::new(r"(?is)<think(?:ing)?>\s*.*$").unwrap()
  });

  let after_closed = closed.replace_all(raw, "").to_string();
  let after_open = open_end.replace_all(&after_closed, "").to_string();
  after_open.trim_start().to_string()
}

/// Ported from deep-research.ts:285-302. The frontmatter shape, body, and
/// references list match upstream verbatim.
pub fn render_research_page(input: &RenderResearchPageInput) -> String {
  let title_escaped = input.topic.replace('"', "\\\"");
  let references = if input.references.is_empty() {
    String::new()
  } else {
    input
      .references
      .iter()
      .enumerate()
      .map(|(index, reference)| {
        format!(
          "{}. [{}]({}) — {}",
          index + 1,
          reference.title,
          reference.url,
          reference.source
        )
      })
      .collect::<Vec<_>>()
      .join("\n")
  };
  let cleaned = clean_synthesis_thinking(&input.synthesis);

  format!(
    "---\ntype: query\ntitle: \"Research: {title}\"\ncreated: {date}\norigin: deep-research\ntags: [research]\n---\n\n# Research: {topic}\n\n{body}\n\n## References\n\n{refs}\n",
    title = title_escaped,
    date = input.date,
    topic = input.topic,
    body = cleaned,
    refs = references
  )
}
```

Add `regex` to `crates/knowledge-core/Cargo.toml` if not present. Check first: `grep "^regex" crates/knowledge-core/Cargo.toml`. If missing, add `regex = "1"` to `[dependencies]`. (Workspace already uses regex elsewhere — likely shared via workspace dep.)

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p knowledge-core research`
Expected: 5 tests PASS.

- [ ] **Step 6: Clippy + commit**

Run: `cargo clippy -p knowledge-core --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/knowledge-core/src/project/research.rs crates/knowledge-core/src/project/mod.rs crates/knowledge-core/Cargo.toml
git commit -m "feat: add research page renderer and thinking-tag stripper"
```

---

### Task 2: Source collection — fan-out web_search + dedup + cap

**Files:**
- Create: `crates/knowledge-server/src/deep_research/mod.rs`
- Create: `crates/knowledge-server/src/deep_research/sources.rs`
- Modify: `crates/knowledge-server/src/lib.rs`

Upstream reference: `upstream_llm_wiki/src/lib/deep-research.ts:61-119` (`collectResearchSources`). Run all queries in parallel, merge, dedupe (lowercased `url` or `source:title:snippet` fallback), cap at 20, collect per-query errors.

- [ ] **Step 1: Register the module**

In `crates/knowledge-server/src/lib.rs`, add (alphabetical):

```rust
pub mod deep_research;
```

- [ ] **Step 2: Scaffold `mod.rs`**

Create `crates/knowledge-server/src/deep_research/mod.rs`:

```rust
pub mod routes;
pub mod sources;

pub use sources::{collect_research_sources, ResearchSources, MAX_RESEARCH_SOURCES};
```

- [ ] **Step 3: Stub `routes.rs`** (so the module compiles before Task 4 adds the handler)

Create `crates/knowledge-server/src/deep_research/routes.rs`:

```rust
use axum::Router;

use crate::app::state::AppState;

pub fn router() -> Router<AppState> {
  Router::new()
}
```

- [ ] **Step 4: Write the failing tests**

Create `crates/knowledge-server/src/deep_research/sources.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
  use super::*;
  use crate::web_search::WebSearchResult;

  fn r(title: &str, url: &str, snippet: &str) -> WebSearchResult {
    WebSearchResult {
      title: title.to_string(),
      url: url.to_string(),
      snippet: snippet.to_string(),
      source: "example.com".to_string(),
    }
  }

  #[test]
  fn dedupe_dedups_by_lowercased_url() {
    let mut accumulator = SourcesAccumulator::default();
    accumulator.merge(vec![r("A", "https://example.com/A", "x")]);
    accumulator.merge(vec![r("A2", "https://EXAMPLE.com/a", "y")]);
    let merged = accumulator.into_results();
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].title, "A");
  }

  #[test]
  fn dedupe_falls_back_to_source_title_snippet_when_no_url() {
    let mut accumulator = SourcesAccumulator::default();
    accumulator.merge(vec![WebSearchResult {
      title: "T".to_string(),
      url: String::new(),
      snippet: "s".to_string(),
      source: "engine-a".to_string(),
    }]);
    accumulator.merge(vec![WebSearchResult {
      title: "T".to_string(),
      url: String::new(),
      snippet: "s".to_string(),
      source: "engine-a".to_string(),
    }]);
    assert_eq!(accumulator.into_results().len(), 1);
  }

  #[test]
  fn dedupe_caps_at_max_research_sources() {
    let mut accumulator = SourcesAccumulator::default();
    let many = (0..50)
      .map(|i| r(&format!("T{i}"), &format!("https://example.com/{i}"), "s"))
      .collect::<Vec<_>>();
    accumulator.merge(many);
    assert_eq!(accumulator.into_results().len(), MAX_RESEARCH_SOURCES);
  }
}
```

- [ ] **Step 5: Run tests to verify they fail**

Run: `cargo test -p knowledge-server deep_research::sources`
Expected: FAIL — `cannot find type SourcesAccumulator`.

- [ ] **Step 6: Implement the collector**

Prepend to `crates/knowledge-server/src/deep_research/sources.rs`:

```rust
use std::collections::HashSet;

use crate::http::error::ApiError;
use crate::web_search::config::WebSearchConfig;
use crate::web_search::provider::web_search;
use crate::web_search::WebSearchResult;

pub const MAX_RESEARCH_SOURCES: usize = 20;
const MAX_PER_QUERY: usize = 5;

#[derive(Debug, Default, Clone)]
pub struct ResearchSources {
  pub results: Vec<WebSearchResult>,
  pub errors: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct SourcesAccumulator {
  results: Vec<WebSearchResult>,
  seen: HashSet<String>,
}

impl SourcesAccumulator {
  pub fn merge(&mut self, batch: Vec<WebSearchResult>) {
    for item in batch {
      if self.results.len() >= MAX_RESEARCH_SOURCES {
        return;
      }
      let key = if item.url.is_empty() {
        format!("{}:{}:{}", item.source, item.title, item.snippet).to_lowercase()
      } else {
        item.url.to_lowercase()
      };
      if self.seen.insert(key) {
        self.results.push(item);
      }
    }
  }

  pub fn into_results(self) -> Vec<WebSearchResult> {
    self.results
  }
}

/// Run all queries in parallel against the configured web-search provider.
/// Dedupe by lowercased URL, fall back to `source:title:snippet` when the
/// URL is empty, cap the merged total at `MAX_RESEARCH_SOURCES`. Each
/// query's error is collected but does not abort the others — matches
/// upstream `Promise.allSettled` semantics at deep-research.ts:107.
pub async fn collect_research_sources(
  queries: &[String],
  config: &WebSearchConfig,
) -> Result<ResearchSources, ApiError> {
  let trimmed = queries
    .iter()
    .map(|query| query.trim().to_string())
    .filter(|query| !query.is_empty())
    .collect::<Vec<_>>();
  if trimmed.is_empty() {
    return Ok(ResearchSources::default());
  }

  let futures = trimmed
    .iter()
    .map(|query| web_search(config, query, MAX_PER_QUERY))
    .collect::<Vec<_>>();
  let results = futures::future::join_all(futures).await;

  let mut accumulator = SourcesAccumulator::default();
  let mut errors = Vec::new();
  for outcome in results {
    match outcome {
      Ok(items) => accumulator.merge(items),
      Err(error) => errors.push(error.to_string()),
    }
  }

  Ok(ResearchSources {
    results: accumulator.into_results(),
    errors,
  })
}
```

The `futures::future::join_all` dependency is already used elsewhere in `knowledge-server` (check via `grep "futures" crates/knowledge-server/Cargo.toml`). If missing, add `futures.workspace = true`.

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p knowledge-server deep_research::sources`
Expected: 3 tests PASS.

- [ ] **Step 8: Clippy + commit**

Run: `cargo clippy -p knowledge-server --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/knowledge-server/src/deep_research/ crates/knowledge-server/src/lib.rs
git commit -m "feat: add deep research source collector with dedup and cap"
```

---

### Task 3: Synthesis prompt + executor

**Files:**
- Modify: `crates/knowledge-server/src/tasks/executors.rs`

Upstream reference: `upstream_llm_wiki/src/lib/deep-research.ts:160-338` (`executeResearch`). Ported logic: read wiki/index.md (best-effort, truncate to 8 KiB), build system prompt verbatim, build user prompt with numbered source context, call `provider.complete_text`, render with `render_research_page`, save, update index. No streaming. No auto-ingest.

- [ ] **Step 1: Add the dispatch arm**

In `crates/knowledge-server/src/tasks/executors.rs`, extend the match at the top of `run_task_executor` (after the `project.dedup_merge` arm added by Phase 3c):

```rust
    "project.deep_research" => run_deep_research_executor(state, &task).await.map_err(Into::into),
```

- [ ] **Step 2: Extend imports**

In `executors.rs` (combine with existing import blocks; group neatly):

```rust
use crate::deep_research::collect_research_sources;
use crate::web_search::config::load_web_search_config;
use knowledge_core::project::queries::{
  save_query_page as save_query_page_existing, SaveQueryPageInput, SavedQueryCitation,
};
use knowledge_core::project::research::{
  render_research_page, RenderResearchPageInput, RenderResearchReference,
};
```

(`save_query_page_existing` is the existing function already imported — leave its existing import in place; don't rename it. The line above is just to mention it for context. If `SaveQueryPageInput`/`SavedQueryCitation` are already imported, don't double-import.)

- [ ] **Step 3: Add the executor**

Append after the dedup executors:

```rust
const RESEARCH_WIKI_INDEX_MAX_BYTES: usize = 8 * 1024;

async fn run_deep_research_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let provider = load_ingest_provider(state)
    .await?
    .ok_or_else(|| ApiError::bad_request("deep research requires an openai-compatible provider"))?;
  let search_config = load_web_search_config(state)
    .await?
    .ok_or_else(|| ApiError::bad_request("deep research requires a configured web search provider"))?;

  let topic = read_string(&task.payload, "topic")?;
  let topic = topic.trim();
  if topic.is_empty() {
    return Err(ApiError::bad_request("topic is required"));
  }
  let queries_value = task.payload.get("searchQueries");
  let queries: Vec<String> = queries_value
    .and_then(Value::as_array)
    .map(|values| {
      values
        .iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .filter(|value| !value.trim().is_empty())
        .collect()
    })
    .unwrap_or_default();
  let queries = if queries.is_empty() {
    vec![topic.to_string()]
  } else {
    queries
  };

  let sources = collect_research_sources(&queries, &search_config).await?;
  if sources.results.is_empty() {
    return Err(ApiError::bad_request(format!(
      "no research sources found{}",
      if sources.errors.is_empty() {
        String::new()
      } else {
        format!(": {}", sources.errors.join("; "))
      }
    )));
  }

  let source_context = sources
    .results
    .iter()
    .enumerate()
    .map(|(index, item)| {
      format!(
        "[{}] **{}** ({})\n{}",
        index + 1,
        item.title,
        item.source,
        item.snippet
      )
    })
    .collect::<Vec<_>>()
    .join("\n\n");

  let wiki_index = try_read_project_file(&root, "wiki/index.md");
  let mut wiki_index = wiki_index;
  if wiki_index.len() > RESEARCH_WIKI_INDEX_MAX_BYTES {
    tracing::debug!(
      "truncating wiki/index.md from {} bytes to {} for deep research synthesizer",
      wiki_index.len(),
      RESEARCH_WIKI_INDEX_MAX_BYTES
    );
    wiki_index.truncate(RESEARCH_WIKI_INDEX_MAX_BYTES);
  }

  let mut system_parts = vec![
    "You are a research assistant. Synthesize the collected research sources into a comprehensive wiki page.".to_string(),
    String::new(),
    "## Cross-referencing (IMPORTANT)".to_string(),
    "- The wiki already has existing pages listed in the Wiki Index below.".to_string(),
    "- When your synthesis mentions an entity or concept that exists in the wiki, ALWAYS use [[wikilink]] syntax to link to it.".to_string(),
    "- For example, if the wiki has an entity 'anthropic', write [[anthropic]] when mentioning it.".to_string(),
    "- This is critical for connecting new research to existing knowledge in the graph.".to_string(),
    String::new(),
    "## Writing Rules".to_string(),
    "- Organize into clear sections with headings".to_string(),
    "- Cite sources using [N] notation".to_string(),
    "- Note contradictions or gaps".to_string(),
    "- Suggest additional sources worth finding".to_string(),
    "- Neutral, encyclopedic tone".to_string(),
  ];
  if !wiki_index.trim().is_empty() {
    system_parts.push(String::new());
    system_parts.push(format!(
      "## Existing Wiki Index (link to these pages with [[wikilink]])\n{wiki_index}"
    ));
  }
  let system_prompt = system_parts.join("\n");

  let user_prompt = format!(
    "Research topic: **{topic}**\n\n## Research Sources\n\n{source_context}\n\nSynthesize into a wiki page."
  );

  let response = provider
    .complete_text(ProviderTextRequest {
      system_prompt,
      user_prompt,
    })
    .await
    .map_err(TaskExecutionError::from_provider_error)
    .map_err(TaskExecutionError::into_api_error)?;

  let now = OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format timestamp"))?;
  let date = now[..10].to_string();
  let slug = research_slug(topic);
  let references = sources
    .results
    .iter()
    .map(|item| RenderResearchReference {
      title: item.title.clone(),
      url: item.url.clone(),
      source: item.source.clone(),
    })
    .collect::<Vec<_>>();
  let page_content = render_research_page(&RenderResearchPageInput {
    topic: topic.to_string(),
    slug: slug.clone(),
    date,
    synthesis: response.text,
    references,
  });

  let relative_path = format!("wiki/queries/research-{slug}.md");
  let absolute_path = root
    .safe_join(&relative_path)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  if let Some(parent) = absolute_path.parent() {
    std::fs::create_dir_all(parent)
      .map_err(|error| ApiError::internal(error.to_string()))?;
  }
  std::fs::write(&absolute_path, &page_content)
    .map_err(|error| ApiError::internal(error.to_string()))?;
  research_update_index(&root, &slug, topic)
    .map_err(|error| ApiError::internal(error.to_string()))?;

  Ok(json!({
    "savedPath": relative_path,
    "sourceCount": sources.results.len(),
    "errors": sources.errors
  }))
}

fn research_slug(topic: &str) -> String {
  let mut slug = String::new();
  let mut last_dash = false;
  for ch in topic.chars() {
    if ch.is_ascii_alphanumeric() {
      slug.push(ch.to_ascii_lowercase());
      last_dash = false;
    } else if !last_dash && !slug.is_empty() {
      slug.push('-');
      last_dash = true;
    }
  }
  let trimmed = slug.trim_end_matches('-').to_string();
  if trimmed.is_empty() { "research".to_string() } else { trimmed }
}

fn research_update_index(
  root: &knowledge_core::project::root::ProjectRoot,
  slug: &str,
  topic: &str,
) -> std::io::Result<()> {
  let path = root.as_path().join("wiki/index.md");
  let existing = std::fs::read_to_string(&path).unwrap_or_default();
  let entry = format!("- [[queries/research-{slug}]] - Research: {topic}\n");
  if existing.contains(entry.trim_end()) {
    return Ok(());
  }
  let marker = "## Queries\n";
  let updated = if let Some(index) = existing.find(marker) {
    let insert_at = index + marker.len();
    let mut updated = existing.clone();
    updated.insert_str(insert_at, &entry);
    updated
  } else {
    format!("{existing}\n## Queries\n{entry}")
  };
  std::fs::write(path, updated)
}
```

(Drop the `save_query_page_existing` import — we use a focused `render_research_page` + direct `std::fs::write` instead. The existing `save_query_page` doesn't fit the upstream research format. Mirror its `update_index` logic in `research_update_index` above.)

- [ ] **Step 4: Build to check for typos**

Run: `cargo build -p knowledge-server`
Expected: clean compile. If any import is unused, remove it.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/tasks/executors.rs
git commit -m "feat: add deep research executor with synthesis and page save"
```

---

### Task 4: REST route + integration test

**Files:**
- Modify: `crates/knowledge-server/src/deep_research/routes.rs`
- Modify: `crates/knowledge-server/src/http/router.rs`
- Modify: `tests/rust-integration/tests/support/mock_openai.rs`
- Create: `tests/rust-integration/tests/deep_research_api.rs`

Pattern: mirror the `detect_dedup_handler` (`crates/knowledge-server/src/projects/routes.rs`) — POST, session OR Bearer authenticated, CSRF for sessions, queues a task, audit log, returns `{taskId, status}`.

- [ ] **Step 1: Add the mock scenario**

In `tests/rust-integration/tests/support/mock_openai.rs`, add `DeepResearchSuccess` to the `MockScenario` enum (next to `DedupSuccess`):

```rust
    DedupSuccess,
    DeepResearchSuccess,
```

Add an `impl` constructor:

```rust
    #[allow(dead_code)]
    pub fn deep_research_success() -> Self {
        Self::DeepResearchSuccess
    }
```

Add a scenario arm in `chat_completions_json` (next to the `DedupSuccess` arm):

```rust
        MockScenario::DeepResearchSuccess => {
            let content = "# Knowledge Graphs\n\nGraphs of typed entities and relations [1]. They power retrieval-augmented generation [2].".to_string();
            (
                StatusCode::OK,
                Json(json!({
                  "id": "chatcmpl-mock-research",
                  "object": "chat.completion",
                  "created": 1_717_171_717,
                  "model": "mock-model",
                  "choices": [
                    {
                      "index": 0,
                      "message": { "role": "assistant", "content": content },
                      "finish_reason": "stop"
                    }
                  ],
                  "usage": { "prompt_tokens": 17, "completion_tokens": 25, "total_tokens": 42 }
                })),
            )
        }
```

- [ ] **Step 2: Write the failing integration test**

Create `tests/rust-integration/tests/deep_research_api.rs`:

```rust
mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::config::AppConfig;
use knowledge_server::tasks::{scheduler, store};
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{Value, json};
use support::TestEnvironment;
use support::mock_openai::{MockOpenAiServer, MockScenario};
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tower::util::ServiceExt;

#[tokio::test]
async fn deep_research_succeeds_with_mock_provider_and_searxng() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("deep-research-flow").await.unwrap();
    let mock_openai = MockOpenAiServer::start(MockScenario::deep_research_success())
        .await
        .unwrap();
    let (searxng_handle, searxng_base) = spawn_mock_searxng().await;
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("deep-research-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    configure_settings(&state, &mock_openai, &searxng_base).await;

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/deep-research"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "topic": "Knowledge Graphs",
                      "searchQueries": ["knowledge graphs", "rag knowledge graph"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let payload = read_json(response.into_body()).await;
    let task_id = payload["taskId"].as_str().unwrap().to_string();

    wait_for_task_terminal(&state, &task_id).await;
    let task = store::get_task_by_id(&state, &task_id).await.unwrap();
    assert_eq!(task.status, "succeeded");

    let saved = project_root.join("wiki/queries/research-knowledge-graphs.md");
    assert!(saved.exists(), "research page should be saved at {saved:?}");
    let content = std::fs::read_to_string(&saved).unwrap();
    assert!(content.contains("origin: deep-research"));
    assert!(content.contains("# Research: Knowledge Graphs"));
    assert!(content.contains("Graphs of typed entities and relations"));
    assert!(content.contains("## References"));
    assert!(content.contains("[Knowledge graphs explained]"));

    let index = std::fs::read_to_string(project_root.join("wiki/index.md")).unwrap();
    assert!(index.contains("- [[queries/research-knowledge-graphs]] - Research: Knowledge Graphs"));

    searxng_handle.await.unwrap();
}

async fn configure_settings(
    state: &knowledge_server::app::state::AppState,
    mock: &MockOpenAiServer,
    searxng_base: &str,
) {
    sqlx::query(
        "UPDATE system_settings
     SET provider_mode = $1,
         provider_base_url = $2,
         provider_api_key = $3,
         provider_model = $4,
         provider_timeout_seconds = $5,
         search_provider = $6,
         searxng_url = $7,
         searxng_categories = $8",
    )
    .bind("openai-compatible")
    .bind(mock.base_url())
    .bind("test-key")
    .bind("mock-model")
    .bind(30_i64)
    .bind("searxng")
    .bind(searxng_base)
    .bind(serde_json::json!(["general"]))
    .execute(&state.pool)
    .await
    .unwrap();
}

async fn spawn_mock_searxng() -> (tokio::task::JoinHandle<()>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        // Each web_search call opens a new connection; respond to as many as arrive.
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let _ = socket.read(&mut buf).await;
                let body = json!({
                  "results": [
                    {
                      "title": "Knowledge graphs explained",
                      "url": "https://example.com/knowledge-graphs",
                      "content": "An overview of knowledge graphs and their applications.",
                      "engine": "duckduckgo"
                    }
                  ]
                })
                .to_string();
                let payload = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(payload.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    (handle, format!("http://127.0.0.1:{port}"))
}

async fn login_and_csrf(state: knowledge_server::app::state::AppState) -> (String, String) {
    let login = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                      "username": "admin",
                      "password": "secret-password"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = login
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let body = read_json(login.into_body()).await;
    let csrf = body.get("csrfToken").and_then(Value::as_str).unwrap().to_string();
    (cookie, csrf)
}

async fn create_project(
    state: knowledge_server::app::state::AppState,
    cookie: &str,
    csrf: &str,
    project_root: std::path::PathBuf,
) -> String {
    support::create_project_with_alias(state, cookie, csrf, project_root).await
}

async fn read_json(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn wait_for_task_terminal(state: &knowledge_server::app::state::AppState, task_id: &str) {
    for _ in 0..120 {
        let progressed = scheduler::run_scheduler_tick(state).await.unwrap_or(false);
        let task = store::get_task_by_id(state, task_id).await.unwrap();
        if matches!(task.status.as_str(), "succeeded" | "failed" | "cancelled") {
            return;
        }
        if !progressed {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
    panic!("deep research task did not reach a terminal state");
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p rust-integration --test deep_research_api -- --test-threads=1`
Expected: FAIL — the route returns 404.

- [ ] **Step 4: Implement the route**

Replace the contents of `crates/knowledge-server/src/deep_research/routes.rs`:

```rust
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::projects::audit::{append_audit_log, CreateAuditLog};
use crate::projects::routes::{authorized_principal, validate_csrf};
use crate::projects::tasks::{create_queued_task, CreateTaskRecord};

pub fn router() -> Router<AppState> {
  Router::new().route(
    "/api/projects/{project_id}/deep-research",
    post(deep_research_handler),
  )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepResearchRequest {
  pub topic: String,
  #[serde(default)]
  pub search_queries: Option<Vec<String>>,
}

async fn deep_research_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
  Json(payload): Json<DeepResearchRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
  let principal = authorized_principal(&state, &headers, Some(&project_id)).await?;
  validate_csrf(&headers, &principal)?;
  let topic = payload.topic.trim();
  if topic.is_empty() {
    return Err(ApiError::bad_request("topic is required"));
  }
  let queries = payload.search_queries.clone().unwrap_or_default();
  let task = create_queued_task(
    &state,
    CreateTaskRecord {
      project_id: project_id.clone(),
      task_type: "project.deep_research".to_string(),
      title: format!("Deep research: {topic}"),
      relative_path: None,
      detail: json!({}),
      created_by: principal.user_id.clone(),
    },
    json!({
      "topic": topic,
      "searchQueries": queries
    }),
  )
  .await?;
  append_audit_log(
    &state,
    CreateAuditLog {
      project_id: Some(project_id),
      actor_id: principal.user_id,
      action: "deep_research.enqueued".to_string(),
      target_type: "deep_research".to_string(),
      target_id: "task".to_string(),
      task_id: Some(task.id.clone()),
      summary: format!("Queued deep research: {topic}"),
      metadata: json!({ "topic": topic, "searchQueries": queries }),
    },
  )
  .await?;
  Ok((
    StatusCode::ACCEPTED,
    Json(json!({ "taskId": task.id, "status": task.status })),
  ))
}
```

(The `(StatusCode, Json<_>)` tuple implements `IntoResponse` directly, so the integration test's `assert_eq!(response.status(), StatusCode::ACCEPTED)` will pass.)

In `crates/knowledge-server/src/http/router.rs`, merge the new router:

```rust
use crate::deep_research;
```

And inside `build_router`:

```rust
    .merge(deep_research::routes::router())
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p rust-integration --test deep_research_api -- --test-threads=1`
Expected: PASS.

- [ ] **Step 6: Clippy + commit**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/knowledge-server/src/deep_research/routes.rs crates/knowledge-server/src/http/router.rs tests/rust-integration/tests/support/mock_openai.rs tests/rust-integration/tests/deep_research_api.rs
git commit -m "feat: add deep research route and integration test"
```

---

### Task 5: Admin API client + query hook

**Files:**
- Modify: `apps/admin/src/features/shared/api.ts`
- Create: `apps/admin/src/features/deep-research/queries.ts`

- [ ] **Step 1: Add the API function**

In `apps/admin/src/features/shared/api.ts`, after the existing dedup section (near `dismissProjectDuplicateGroup` at ~line 902), add:

```ts
export async function createDeepResearchTask(input: {
  projectId: string;
  topic: string;
  searchQueries?: string[];
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/deep-research`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({
        topic: input.topic,
        searchQueries: input.searchQueries,
      }),
    },
    z.object({
      taskId: z.string(),
      status: z.string(),
    }),
  );
}
```

- [ ] **Step 2: Create the hook**

Create `apps/admin/src/features/deep-research/queries.ts`:

```ts
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { createDeepResearchTask } from "../shared/api";

export function useCreateDeepResearchTaskMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: createDeepResearchTask,
    onSuccess: async (_result, variables) => {
      await queryClient.invalidateQueries({ queryKey: ["project-tasks", variables.projectId] });
    },
  });
}
```

- [ ] **Step 3: Verify the admin suite still passes**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS (no new tests yet — that's Task 6).

- [ ] **Step 4: Commit**

```bash
git add apps/admin/src/features/shared/api.ts apps/admin/src/features/deep-research/queries.ts
git commit -m "feat: add deep research api client and mutation hook"
```

---

### Task 6: Admin Deep Research page + routing

**Files:**
- Create: `apps/admin/src/features/deep-research/page.tsx`
- Create: `apps/admin/src/features/deep-research/page.test.tsx`
- Modify: `apps/admin/src/app/router.tsx`
- Modify: `apps/admin/src/lib/route-meta.ts`

Pattern: mirror the existing project page conventions (see `apps/admin/src/features/dedup/page.tsx` for the Card + form layout; `apps/admin/src/features/lint/page.tsx` for the "list recent tasks" pattern).

- [ ] **Step 1: Write the failing tests**

Create `apps/admin/src/features/deep-research/page.test.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { DeepResearchPage } from "./page";

const mockCreate = vi.fn();
const mockTasks = vi.fn();

vi.mock("./queries", () => ({
  useCreateDeepResearchTaskMutation: () => ({
    mutateAsync: mockCreate,
    isPending: false,
  }),
}));

vi.mock("../tasks/queries", () => ({
  useProjectTasksQuery: () => ({ data: mockTasks(), isLoading: false }),
}));

function renderPage() {
  const queryClient = new QueryClient();
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/projects/project-1/deep-research"]}>
        <Routes>
          <Route path="projects/:projectId/deep-research" element={<DeepResearchPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("deep research page", () => {
  it("submits topic and parsed queries", async () => {
    const user = userEvent.setup();
    mockTasks.mockReturnValue([]);
    mockCreate.mockResolvedValue({ taskId: "task-1", status: "queued" });

    renderPage();

    await user.type(screen.getByLabelText("Topic"), "Knowledge Graphs");
    await user.type(screen.getByLabelText("Search Queries"), "knowledge graphs, rag knowledge graph");
    await user.click(screen.getByRole("button", { name: "Start Research" }));

    expect(mockCreate).toHaveBeenCalledWith({
      projectId: "project-1",
      topic: "Knowledge Graphs",
      searchQueries: ["knowledge graphs", "rag knowledge graph"],
    });
  });

  it("lists recent deep research tasks", () => {
    mockTasks.mockReturnValue([
      {
        id: "task-1",
        taskType: "project.deep_research",
        title: "Deep research: KG",
        status: "succeeded",
        createdAt: "2026-06-15T00:00:00Z",
        updatedAt: "2026-06-15T00:01:00Z",
      },
      {
        id: "task-2",
        taskType: "project.ingest_source",
        title: "Ingest",
        status: "succeeded",
        createdAt: "2026-06-15T00:00:00Z",
        updatedAt: "2026-06-15T00:00:30Z",
      },
    ]);
    renderPage();
    expect(screen.getByText("Deep research: KG")).toBeInTheDocument();
    expect(screen.queryByText("Ingest")).not.toBeInTheDocument();
  });
});
```

(`useProjectTasksQuery` is verified to exist in `apps/admin/src/features/tasks/queries.ts:5`. Tasks come from `tasksSchema` in `shared/api.ts:41` with the field name `taskType` — NOT `type`.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm run test --workspace @knowledge/admin -- src/features/deep-research/page.test.tsx`
Expected: FAIL — `Cannot find module './page'`.

- [ ] **Step 3: Implement the page**

Create `apps/admin/src/features/deep-research/page.tsx`:

```tsx
import { useState } from "react";
import { useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { PageSection } from "@/components/layout/page-section";
import { StatusBadge } from "@/components/layout/status-badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";

import { useProjectTasksQuery } from "../tasks/queries";

import { useCreateDeepResearchTaskMutation } from "./queries";

export function DeepResearchPage() {
  const { projectId = "" } = useParams();
  const [topic, setTopic] = useState("");
  const [queries, setQueries] = useState("");
  const createTask = useCreateDeepResearchTaskMutation();
  const tasksQuery = useProjectTasksQuery(projectId);
  const tasks = (tasksQuery.data ?? []).filter((task) => task.taskType === "project.deep_research");

  return (
    <PageSection
      description="Run a deep research task: fan-out search queries against the configured provider, synthesize a wiki page, save it under wiki/queries/."
      title="Deep Research"
    >
      <Card>
        <CardHeader>
          <CardTitle>New Research</CardTitle>
          <CardDescription>
            The synthesizer uses the configured chat provider. Web search uses the configured search provider (see Settings).
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <label className="grid gap-2 text-sm font-medium">
            Topic
            <Input
              aria-label="Topic"
              onChange={(event) => setTopic(event.target.value)}
              placeholder="e.g. Knowledge Graphs"
              value={topic}
            />
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Search Queries
            <Input
              aria-label="Search Queries"
              onChange={(event) => setQueries(event.target.value)}
              placeholder="Comma-separated. Leave blank to use the topic as the only query."
              value={queries}
            />
          </label>
          <div className="flex justify-end">
            <Button
              disabled={createTask.isPending || topic.trim().length === 0}
              onClick={async () => {
                const parsed = queries
                  .split(",")
                  .map((value) => value.trim())
                  .filter((value) => value.length > 0);
                await createTask.mutateAsync({
                  projectId,
                  topic: topic.trim(),
                  searchQueries: parsed.length > 0 ? parsed : undefined,
                });
                setTopic("");
                setQueries("");
              }}
            >
              Start Research
            </Button>
          </div>
        </CardContent>
      </Card>

      {tasks.length ? (
        <div className="grid gap-3">
          {tasks.map((task) => (
            <Card key={task.id}>
              <CardHeader>
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="space-y-1">
                    <CardTitle>{task.title}</CardTitle>
                    <CardDescription>
                      Updated {task.updatedAt ?? "—"} · created {task.createdAt ?? "—"}
                    </CardDescription>
                  </div>
                  <StatusBadge value={task.status} />
                </div>
              </CardHeader>
            </Card>
          ))}
        </div>
      ) : (
        <EmptyState description="No research tasks yet for this project." title="No research" />
      )}
    </PageSection>
  );
}
```

(Verified field names against `tasksSchema` in `apps/admin/src/features/shared/api.ts:41-62`: `id`, `taskType`, `title`, `status`, `createdAt`, `updatedAt` are the relevant ones; the last three are `.optional()` so the page handles their absence with `?? "—"`.)

- [ ] **Step 4: Wire routing**

In `apps/admin/src/app/router.tsx`, add the import alphabetically:

```tsx
import { DeepResearchPage } from "../features/deep-research/page";
```

Inside the `projects/:projectId` children (after `dedup`):

```tsx
            <Route path="deep-research" element={<DeepResearchPage />} />
```

In `apps/admin/src/lib/route-meta.ts`, add to `projectRoutes` (after `"/dedup"`):

```ts
  { suffix: "/deep-research", label: "Deep Research" },
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `npm run test --workspace @knowledge/admin -- src/features/deep-research/page.test.tsx`
Expected: 2 tests PASS.

- [ ] **Step 6: Run the full admin suite**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS (including the new page tests).

- [ ] **Step 7: Commit**

```bash
git add apps/admin/src/features/deep-research/page.tsx apps/admin/src/features/deep-research/page.test.tsx apps/admin/src/app/router.tsx apps/admin/src/lib/route-meta.ts
git commit -m "feat: add deep research admin page with form and task list"
```

---

### Task 7: Playwright e2e

**Files:**
- Create: `tests/web/tests/deep-research.spec.ts`

The existing mock-openai server already handles `/v1/chat/completions` (default reply works as the synthesis output) and `/search` (Phase 4c added the SearXNG mock). No mock changes are needed.

- [ ] **Step 1: Write the spec**

Create `tests/web/tests/deep-research.spec.ts`:

```ts
import { expect, test } from "@playwright/test";

test("admin queues a deep research task that completes successfully", async ({ page }) => {
  await signInAsAdmin(page);

  await page.getByRole("link", { name: "Settings" }).click();
  await page.getByLabel("Provider Mode", { exact: true }).fill("openai-compatible");
  await page.getByLabel("Language", { exact: true }).fill("en");
  await page.getByLabel("Provider Base URL", { exact: true }).fill("http://127.0.0.1:18080/v1");
  await page.getByLabel("Provider API Key", { exact: true }).fill("test-key");
  await page.getByLabel("Provider Model", { exact: true }).fill("mock-model");
  await page.getByLabel("Provider Timeout Seconds", { exact: true }).fill("30");
  await page.getByLabel(/Search Provider/i).selectOption("searxng");
  await page.getByLabel(/SearXNG Instance URL/i).fill("http://127.0.0.1:18080");
  await page.getByRole("button", { name: "Save Settings" }).click();

  await page.getByRole("link", { name: "Projects" }).click();
  await createProject(page, "deep-research-project");

  await page.getByRole("tab", { name: "Deep Research" }).click();
  await page.getByLabel("Topic").fill("Knowledge Graphs");
  await page.getByLabel("Search Queries").fill("knowledge graphs");
  await page.getByRole("button", { name: "Start Research" }).click();

  await expect(page.getByText("Deep research: Knowledge Graphs")).toBeVisible({ timeout: 10_000 });
  await expect(page.getByText("succeeded").first()).toBeVisible({ timeout: 30_000 });
});

async function signInAsAdmin(page: import("@playwright/test").Page) {
  await page.goto("/login");
  await page.getByLabel("Username").fill("admin");
  await page.getByLabel("Password").fill("secret-password");
  await page.getByRole("button", { name: "Sign in" }).click();
  await page.waitForURL("**/projects");
}

async function createProject(page: import("@playwright/test").Page, name: string) {
  await page.getByRole("button", { name: "Create Project" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.getByLabel("Name").fill(name);
  await page.getByRole("dialog").getByRole("button", { name: "Create Project" }).click();
  await page.waitForURL(/\/projects\/[^/]+$/);
}
```

- [ ] **Step 2: Run the spec**

Run: `npm run test --workspace @knowledge/web -- tests/deep-research.spec.ts`
Expected: PASS.

- [ ] **Step 3: Full playwright suite**

Run: `npm run test --workspace @knowledge/web`
Expected: 10 specs PASS (9 existing + new). `workers: 1` already pinned.

- [ ] **Step 4: Commit**

```bash
git add tests/web/tests/deep-research.spec.ts
git commit -m "test: add deep research e2e flow"
```

---

### Task 8: Full verification

**Files:** none (verification only)

- [ ] **Step 1: Rust lint + tests**

Run: `cargo fmt --all -- --check`
Expected: clean.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

Run: `cargo test --workspace -- --test-threads=1`
Expected: PASS — including 5 new `research` unit tests (knowledge-core), 3 new `deep_research::sources` unit tests (knowledge-server), 1 new `deep_research_api` integration test.

- [ ] **Step 2: Frontend tests + lint**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS — including 2 new `deep-research/page.test.tsx`.

Run: `npm run test --workspace @knowledge/api-client`
Expected: PASS (regression check).

Run: `npm run test --workspace @knowledge/mcp-server`
Expected: PASS (regression check).

Run: `npm run lint`
Expected: tsc + clippy clean.

- [ ] **Step 3: Playwright suite**

Run: `npm run test --workspace @knowledge/web`
Expected: 10 specs PASS.

- [ ] **Step 4: Final commit (only if fixes were needed)**

```bash
git status
```

If steps 1–3 required fixes, commit them. Otherwise the work is complete — Phase 4 is done.
