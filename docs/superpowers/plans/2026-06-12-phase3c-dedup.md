# Phase 3c — Dedup (Duplicate Detection & Merge) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port upstream's duplicate-page detection and merge (`upstream_llm_wiki/src/lib/dedup.ts`, `dedup-storage.ts`) to the server: an embedding prefilter narrows candidate pages, an LLM detects duplicate groups, the admin UI lets the user merge (LLM body merge + deterministic frontmatter union + cross-reference rewrite) or dismiss (persistent not-duplicates whitelist).

**Architecture:** Three layers. (1) `knowledge-core/src/project/dedup.rs` + `dedup_store.rs` — pure data transforms ported from upstream dedup.ts (entity summaries, detector parsing, merge computation, cross-reference rewrites) and a `.knowledge/dedup/` JSON store. (2) `knowledge-server` — an embedding prefilter in `retrieval/dedup.rs`, two task executors (`project.dedup_detect`, `project.dedup_merge`), and four REST routes. (3) `apps/admin/src/features/dedup/` — a Dedup page with detect/merge/dismiss actions. Mock OpenAI servers (Rust + Node) gain prompt-aware dedup responses so integration and e2e tests run without a real LLM.

**Tech Stack:** Rust (axum 0.8, sqlx, thiserror, uuid, serde), React 19 + TanStack Query + Zod, Playwright, vitest.

**Upstream references (memory rule: port, don't invent):**
- `upstream_llm_wiki/src/lib/dedup.ts` — all three stages, prompts verbatim.
- `upstream_llm_wiki/src/lib/dedup-storage.ts` — not-duplicates whitelist semantics.
- `upstream_llm_wiki/src/lib/dedup.test.ts` — test expectations to mirror.
- `upstream_llm_wiki/src/lib/dedup-queue.ts` / `dedup-runner.ts` — upstream's in-app queue/runner layer; NOT ported. The server's existing task queue (`crates/knowledge-server/src/tasks/`) plays this role via the two executors.

**Documented divergences from upstream:**
1. No `max_tokens` / `temperature` on LLM calls — `ProviderTextRequest` only carries `system_prompt`/`user_prompt`.
2. `rewriteIndexMd` is NOT ported. Phase 3a's `delete_wiki_pages_with_refs` cascade already removes deleted pages' index lines. To avoid a duplicate canonical line, `wiki/index.md` is **excluded** from cross-reference rewrites in the merge executor.
3. Store lives at `.knowledge/dedup/` (not upstream's `.llm-wiki/`), matching this repo's `.knowledge/reviews/items.json` pattern. Backups go to `.knowledge/dedup/backups/<stamp>/` instead of `.llm-wiki/page-history/`.
4. Detection adds an embedding prefilter (server has embeddings infra; upstream sends every page to the LLM). Threshold 0.80 cosine, max 40 candidate pages.
5. Slug→page resolution errors (ambiguous or missing slug) fail the merge task with a clear message; upstream's UI never hits this because it holds paths directly.

**Indentation conventions:** `crates/**` source = 2-space, EXCEPT `crates/knowledge-server/src/projects/routes.rs` = 4-space. `tests/rust-integration/**` = 4-space. All TS/TSX = 2-space.

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `crates/knowledge-core/src/project/page_merge.rs` | Modify | Make 5 helpers `pub(crate)` for reuse |
| `crates/knowledge-core/src/project/dedup.rs` | Create | Stages 1–3 ported from upstream dedup.ts (pure, no I/O beyond page walking) |
| `crates/knowledge-core/src/project/dedup_store.rs` | Create | `.knowledge/dedup/groups.json` + `not-duplicates.json` persistence |
| `crates/knowledge-core/src/project/mod.rs` | Modify | Register both modules |
| `crates/knowledge-server/src/retrieval/dedup.rs` | Create | Embedding prefilter (`select_dedup_candidate_pages`) |
| `crates/knowledge-server/src/retrieval/mod.rs` | Modify | Register module |
| `crates/knowledge-server/src/retrieval/service.rs` | Modify | `pub(crate)` on `ensure_project_embeddings`, `cosine_similarity` |
| `crates/knowledge-server/src/tasks/executors.rs` | Modify | `run_dedup_detect_executor`, `run_dedup_merge_executor` + dispatch arms |
| `crates/knowledge-server/src/projects/routes.rs` | Modify | GET overview, POST detect, POST merge, POST dismiss |
| `tests/rust-integration/tests/support/mock_openai.rs` | Modify | `MockScenario::DedupSuccess` (prompt-aware) |
| `tests/rust-integration/tests/dedup_api.rs` | Create | Detect → merge → dismiss integration flows |
| `apps/admin/src/features/shared/api.ts` | Modify | 4 API functions + schemas |
| `apps/admin/src/features/dedup/queries.ts` | Create | Query + 3 mutations |
| `apps/admin/src/features/dedup/page.tsx` | Create | DedupPage UI |
| `apps/admin/src/features/dedup/page.test.tsx` | Create | vitest component test |
| `apps/admin/src/app/router.tsx` | Modify | Route wiring |
| `apps/admin/src/lib/route-meta.ts` | Modify | Nav tab |
| `tests/web/mock-openai.mjs` | Modify | Prompt-aware chat + `/v1/embeddings` |
| `tests/web/tests/dedup.spec.ts` | Create | e2e detect/merge flow |

---

### Task 1: Stage 1 — entity summaries & page walkers (`dedup.rs`)

**Files:**
- Modify: `crates/knowledge-core/src/project/page_merge.rs` (visibility only)
- Create: `crates/knowledge-core/src/project/dedup.rs`
- Modify: `crates/knowledge-core/src/project/mod.rs`

Upstream reference: `upstream_llm_wiki/src/lib/dedup.ts` lines 116–165 (`extractEntitySummary`, `slugFromPath`, `firstBodyParagraph`, `truncate`).

- [ ] **Step 1: Make page_merge helpers reusable**

In `crates/knowledge-core/src/project/page_merge.rs`, change the visibility of exactly these five functions from `fn` to `pub(crate) fn` (no other changes):

```rust
pub(crate) fn merge_array_fields_into_content(   // line ~149
pub(crate) fn parse_frontmatter_array(           // line ~183
pub(crate) fn write_frontmatter_array(           // line ~230
pub(crate) fn set_frontmatter_scalar(            // line ~299
pub(crate) fn split_frontmatter_lines_owned(     // line ~330
```

- [ ] **Step 2: Register the new module**

In `crates/knowledge-core/src/project/mod.rs`, add (alphabetical, after `pub mod ingest_sanitize;` would be wrong — list is roughly alphabetical; insert after `pub mod enrich;`):

```rust
pub mod dedup;
pub mod dedup_store;
```

(`dedup_store` is created in Task 4; declaring it now would break the build — **only add `pub mod dedup;` in this task**, add `pub mod dedup_store;` in Task 4.)

- [ ] **Step 3: Write failing tests for Stage 1**

Create `crates/knowledge-core/src/project/dedup.rs` containing ONLY the test module first:

```rust
#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extract_entity_summary_reads_frontmatter_fields() {
    let content = "---\ntype: entity\ntitle: Volatile Fatty Acids\ndescription: Short-chain fatty acids.\ntags: [chemistry, metabolism]\n---\n\n# VFA\n\nBody text here.\n";
    let summary = extract_entity_summary("wiki/entities/vfa.md", content).unwrap();
    assert_eq!(summary.slug, "vfa");
    assert_eq!(summary.path, "wiki/entities/vfa.md");
    assert_eq!(summary.page_type, "entity");
    assert_eq!(summary.title, "Volatile Fatty Acids");
    assert_eq!(summary.description.as_deref(), Some("Short-chain fatty acids."));
    assert_eq!(summary.tags, vec!["chemistry".to_string(), "metabolism".to_string()]);
  }

  #[test]
  fn extract_entity_summary_returns_none_without_frontmatter() {
    assert!(extract_entity_summary("wiki/entities/foo.md", "# Foo\n\nNo frontmatter.").is_none());
  }

  #[test]
  fn extract_entity_summary_falls_back_to_first_body_paragraph() {
    let content = "---\ntype: concept\ntitle: Attention\n---\n\n# Attention\n\n| a | b |\n\nFocuses computation on relevant tokens.\n";
    let summary = extract_entity_summary("wiki/concepts/attention.md", content).unwrap();
    assert_eq!(
      summary.description.as_deref(),
      Some("Focuses computation on relevant tokens.")
    );
  }

  #[test]
  fn extract_entity_summary_truncates_long_descriptions() {
    let long_line = "x".repeat(300);
    let content = format!("---\ntype: concept\ntitle: Foo\n---\n\n{long_line}\n");
    let summary = extract_entity_summary("wiki/concepts/foo.md", &content).unwrap();
    let description = summary.description.unwrap();
    assert_eq!(description.chars().count(), 200);
    assert!(description.ends_with('…'));
  }

  #[test]
  fn extract_entity_summary_defaults_title_and_type() {
    let content = "---\ncreated: 2026-06-12\n---\n\nBody.\n";
    let summary = extract_entity_summary("wiki/entities/some-slug.md", content).unwrap();
    assert_eq!(summary.title, "some-slug");
    assert_eq!(summary.page_type, "unknown");
    assert!(summary.tags.is_empty());
  }

  #[test]
  fn collect_entity_pages_walks_entities_and_concepts_only() {
    let temp = tempfile::tempdir().unwrap();
    let root = crate::project::root::ProjectRoot::new(temp.path()).unwrap();
    std::fs::create_dir_all(temp.path().join("wiki/entities")).unwrap();
    std::fs::create_dir_all(temp.path().join("wiki/concepts/nested")).unwrap();
    std::fs::create_dir_all(temp.path().join("wiki/sources")).unwrap();
    std::fs::write(temp.path().join("wiki/entities/b.md"), "b").unwrap();
    std::fs::write(temp.path().join("wiki/concepts/a.md"), "a").unwrap();
    std::fs::write(temp.path().join("wiki/concepts/nested/c.md"), "c").unwrap();
    std::fs::write(temp.path().join("wiki/concepts/skip.txt"), "no").unwrap();
    std::fs::write(temp.path().join("wiki/sources/s.md"), "s").unwrap();

    let pages = collect_entity_pages(&root).unwrap();
    let paths = pages.iter().map(|page| page.path.as_str()).collect::<Vec<_>>();
    assert_eq!(
      paths,
      vec!["wiki/concepts/a.md", "wiki/concepts/nested/c.md", "wiki/entities/b.md"]
    );
    assert_eq!(pages[0].slug, "a");
    assert_eq!(pages[0].content, "a");
  }

  #[test]
  fn collect_all_wiki_pages_includes_every_markdown_file() {
    let temp = tempfile::tempdir().unwrap();
    let root = crate::project::root::ProjectRoot::new(temp.path()).unwrap();
    std::fs::create_dir_all(temp.path().join("wiki/sources")).unwrap();
    std::fs::write(temp.path().join("wiki/index.md"), "index").unwrap();
    std::fs::write(temp.path().join("wiki/sources/s.md"), "s").unwrap();

    let pages = collect_all_wiki_pages(&root).unwrap();
    let paths = pages.iter().map(|page| page.path.as_str()).collect::<Vec<_>>();
    assert_eq!(paths, vec!["wiki/index.md", "wiki/sources/s.md"]);
  }
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test -p knowledge-core dedup`
Expected: FAIL — `cannot find function extract_entity_summary` (compile error).

- [ ] **Step 5: Implement Stage 1**

Prepend to `crates/knowledge-core/src/project/dedup.rs` (above the test module). Port of dedup.ts lines 116–165; `parseFrontmatter` gate becomes `split_frontmatter_lines_owned` (returns `None` exactly when upstream's `frontmatter` is null):

```rust
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::page_merge::{parse_frontmatter_array, split_frontmatter_lines_owned};
use super::root::{ProjectRoot, ProjectRootError};

/// Ported from upstream_llm_wiki/src/lib/dedup.ts (EntitySummary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitySummary {
  pub slug: String,
  pub path: String,
  pub page_type: String,
  pub title: String,
  pub description: Option<String>,
  pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DedupPage {
  pub slug: String,
  pub path: String,
  pub content: String,
}

pub fn extract_entity_summary(relative_path: &str, content: &str) -> Option<EntitySummary> {
  let (frontmatter_lines, body, _newline) = split_frontmatter_lines_owned(content)?;

  let mut fields = BTreeMap::new();
  for line in &frontmatter_lines {
    let Some((key, value)) = line.split_once(':') else {
      continue;
    };
    let normalized = value.trim().trim_matches('"').trim_matches('\'').trim();
    if normalized.is_empty() {
      continue;
    }
    fields.insert(key.trim().to_string(), normalized.to_string());
  }

  let slug = slug_from_path(relative_path);
  let page_type = fields.get("type").cloned().unwrap_or_else(|| "unknown".to_string());
  let title = fields.get("title").cloned().unwrap_or_else(|| slug.clone());
  let description = fields
    .get("description")
    .cloned()
    .or_else(|| first_body_paragraph(&body))
    .map(|value| truncate_chars(&value, 200));
  let tags = parse_frontmatter_array(content, "tags");

  Some(EntitySummary {
    slug,
    path: relative_path.to_string(),
    page_type,
    title,
    description,
    tags,
  })
}

pub fn slug_from_path(path: &str) -> String {
  let base = path.rsplit('/').next().unwrap_or(path);
  base.strip_suffix(".md").unwrap_or(base).to_string()
}

fn first_body_paragraph(body: &str) -> Option<String> {
  body
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .find(|line| !line.starts_with('#') && !line.starts_with('|'))
    .map(str::to_string)
}

fn truncate_chars(value: &str, max: usize) -> String {
  if value.chars().count() <= max {
    return value.to_string();
  }
  let mut truncated = value.chars().take(max - 1).collect::<String>();
  truncated.push('…');
  truncated
}

/// Walk wiki/entities + wiki/concepts (upstream's extract scope), sorted by path.
pub fn collect_entity_pages(root: &ProjectRoot) -> Result<Vec<DedupPage>, ProjectRootError> {
  let mut pages = Vec::new();
  for subdir in ["wiki/concepts", "wiki/entities"] {
    let dir = root.safe_join(subdir)?;
    if dir.exists() {
      collect_markdown_pages(root.as_path(), &dir, &mut pages)?;
    }
  }
  pages.sort_by(|left, right| left.path.cmp(&right.path));
  Ok(pages)
}

/// Every .md under wiki/ — upstream's MergeRequest.otherWikiPages source.
pub fn collect_all_wiki_pages(root: &ProjectRoot) -> Result<Vec<DedupPage>, ProjectRootError> {
  let mut pages = Vec::new();
  let wiki_root = root.safe_join("wiki")?;
  if wiki_root.exists() {
    collect_markdown_pages(root.as_path(), &wiki_root, &mut pages)?;
  }
  pages.sort_by(|left, right| left.path.cmp(&right.path));
  Ok(pages)
}

fn collect_markdown_pages(
  project_root: &Path,
  dir: &Path,
  pages: &mut Vec<DedupPage>,
) -> Result<(), ProjectRootError> {
  for entry in fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();
    if entry.file_type()?.is_dir() {
      collect_markdown_pages(project_root, &path, pages)?;
      continue;
    }
    if path.extension().and_then(|value| value.to_str()) != Some("md") {
      continue;
    }
    let relative_path = path
      .strip_prefix(project_root)
      .unwrap_or(&path)
      .to_string_lossy()
      .replace('\\', "/");
    let content = fs::read_to_string(&path)?;
    pages.push(DedupPage {
      slug: slug_from_path(&relative_path),
      path: relative_path,
      content,
    });
  }
  Ok(())
}
```

Note: `ProjectRoot::as_path` — if the method does not exist, check `root.rs`; the retrieval service calls `root.as_path()` so it exists.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p knowledge-core dedup`
Expected: 7 tests PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/knowledge-core/src/project/dedup.rs crates/knowledge-core/src/project/page_merge.rs crates/knowledge-core/src/project/mod.rs
git commit -m "feat: add dedup entity summaries and wiki page walkers"
```

---

### Task 2: Stage 2 — detector prompt, parsing, filtering (`dedup.rs`)

**Files:**
- Modify: `crates/knowledge-core/src/project/dedup.rs`

Upstream reference: `upstream_llm_wiki/src/lib/dedup.ts` lines 171–315 (`DETECTOR_SYSTEM_PROMPT`, `buildDetectorUserMessage`, `parseDetectorResponse`, `extractFirstJsonObject`, `normalizeGroupKey`, and the filter logic inside `detectDuplicateGroups`). The LLM call itself lives in the server executor (Task 7); core only builds/parses.

- [ ] **Step 1: Add failing tests**

Append inside the existing `mod tests` in `dedup.rs`:

```rust
  #[test]
  fn build_detector_user_message_lists_pages_with_tags_and_description() {
    let summaries = vec![
      EntitySummary {
        slug: "vfa".to_string(),
        path: "wiki/entities/vfa.md".to_string(),
        page_type: "entity".to_string(),
        title: "VFA".to_string(),
        description: Some("Short-chain fatty acids.".to_string()),
        tags: vec!["chemistry".to_string()],
      },
      EntitySummary {
        slug: "volatile-fatty-acids".to_string(),
        path: "wiki/entities/volatile-fatty-acids.md".to_string(),
        page_type: "entity".to_string(),
        title: "Volatile Fatty Acids".to_string(),
        description: None,
        tags: Vec::new(),
      },
    ];
    let message = build_detector_user_message(&summaries);
    assert!(message.starts_with("## Wiki pages to scan (2 entries)\n\n"));
    assert!(message.contains(
      "- type=entity, slug=vfa, title=\"VFA\" [chemistry] — Short-chain fatty acids."
    ));
    assert!(message.contains("- type=entity, slug=volatile-fatty-acids, title=\"Volatile Fatty Acids\""));
    assert!(message.ends_with("\n\nReturn duplicate groups as JSON only."));
  }

  #[test]
  fn parse_detector_response_extracts_json_from_noise() {
    let raw = "Sure, here you go:\n```json\n{\"groups\":[{\"slugs\":[\"a\",\"b\"],\"reason\":\"same\",\"confidence\":\"high\"}]}\n```\nLet me know!";
    let groups = parse_detector_response(raw);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].slugs, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(groups[0].reason, "same");
    assert_eq!(groups[0].confidence, "high");
  }

  #[test]
  fn parse_detector_response_tolerates_garbage_and_bad_entries() {
    assert!(parse_detector_response("no json here").is_empty());
    assert!(parse_detector_response("{not valid json").is_empty());
    assert!(parse_detector_response("{\"groups\": \"nope\"}").is_empty());
    // single-slug group dropped; invalid confidence coerced to "low"
    let raw = "{\"groups\":[{\"slugs\":[\"only-one\"]},{\"slugs\":[\"a\",\"b\"],\"confidence\":\"certain\"}]}";
    let groups = parse_detector_response(raw);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].confidence, "low");
    assert_eq!(groups[0].reason, "");
  }

  #[test]
  fn filter_detected_groups_drops_invented_small_and_whitelisted() {
    let valid_slugs = ["a", "b", "c"]
      .iter()
      .map(|slug| slug.to_string())
      .collect::<std::collections::BTreeSet<_>>();
    let groups = vec![
      DuplicateGroupCandidate {
        slugs: vec!["a".to_string(), "invented".to_string(), "b".to_string()],
        reason: "r1".to_string(),
        confidence: "high".to_string(),
      },
      DuplicateGroupCandidate {
        slugs: vec!["c".to_string(), "invented".to_string()],
        reason: "r2".to_string(),
        confidence: "medium".to_string(),
      },
      DuplicateGroupCandidate {
        slugs: vec!["B".to_string(), "a".to_string()],
        reason: "r3".to_string(),
        confidence: "low".to_string(),
      },
    ];
    let not_duplicates = vec![vec!["b".to_string(), "a".to_string()]];
    let filtered = filter_detected_groups(groups, &valid_slugs, &not_duplicates);
    // group 1 survives with invented slug stripped (a,b remain but a,b is whitelisted? No:
    // whitelist key is "a,b"; group 1 strips to ["a","b"] → filtered out too).
    // group 2 strips to ["c"] → too small. group 3 has "B" not in valid set → ["a"] → too small.
    assert!(filtered.is_empty());

    let surviving = filter_detected_groups(
      vec![DuplicateGroupCandidate {
        slugs: vec!["a".to_string(), "c".to_string()],
        reason: "keep".to_string(),
        confidence: "high".to_string(),
      }],
      &valid_slugs,
      &not_duplicates,
    );
    assert_eq!(surviving.len(), 1);
    assert_eq!(surviving[0].slugs, vec!["a".to_string(), "c".to_string()]);
  }

  #[test]
  fn normalize_group_key_is_case_insensitive_and_sorted() {
    assert_eq!(
      normalize_group_key(&["B".to_string(), "a".to_string()]),
      "a,b"
    );
  }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p knowledge-core dedup`
Expected: FAIL — `cannot find function build_detector_user_message` (compile error).

- [ ] **Step 3: Implement Stage 2**

Add to `dedup.rs` (after the Stage 1 code, before `#[cfg(test)]`). Prompts are **verbatim** from dedup.ts lines 171–198 (the upstream string contains `\`type\`` — a markdown backtick, kept literally):

```rust
use std::collections::BTreeSet;

/// Verbatim from upstream_llm_wiki/src/lib/dedup.ts DETECTOR_SYSTEM_PROMPT.
pub const DETECTOR_SYSTEM_PROMPT: &str = r#"You are a wiki maintenance assistant. You will receive a list of entity / concept pages from a wiki. Identify groups of slugs that likely refer to the same underlying topic under different names — for example:

- Same name in two languages (English vs Chinese, etc.)
- Plural vs singular form (e.g. "dpao" vs "dpaos")
- Abbreviation vs full form (e.g. "vfa" vs "volatile-fatty-acids")
- Synonyms in the same language
- The same proper noun spelled differently

Output ONLY valid JSON. No prose, no markdown fences, no explanation outside the JSON. The schema is:

{
  "groups": [
    {
      "slugs": ["slug-a", "slug-b"],
      "reason": "Both refer to X; first is English, second is Chinese.",
      "confidence": "high"
    }
  ]
}

Rules:
- Only include groups of 2 or more slugs from the input list.
- "high" = clearly the same entity, only naming differs.
- "medium" = likely the same but context-dependent.
- "low" = uncertain; user should review carefully.
- Never invent slugs that aren't in the input.
- If no duplicates exist, output {"groups": []}.
- Pages of different `type` (e.g. an entity and a concept) usually should NOT be grouped — only group across types when they're unambiguously the same thing."#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroupCandidate {
  pub slugs: Vec<String>,
  pub reason: String,
  pub confidence: String,
}

/// Ported from dedup.ts buildDetectorUserMessage.
pub fn build_detector_user_message(summaries: &[EntitySummary]) -> String {
  let lines = summaries
    .iter()
    .map(|summary| {
      let tag_part = if summary.tags.is_empty() {
        String::new()
      } else {
        format!(" [{}]", summary.tags.join(", "))
      };
      let desc_part = summary
        .description
        .as_deref()
        .map(|description| format!(" — {description}"))
        .unwrap_or_default();
      let title = serde_json::to_string(&summary.title).unwrap_or_else(|_| format!("\"{}\"", summary.title));
      format!(
        "- type={}, slug={}, title={}{}{}",
        summary.page_type, summary.slug, title, tag_part, desc_part
      )
    })
    .collect::<Vec<_>>();
  format!(
    "## Wiki pages to scan ({} entries)\n\n{}\n\nReturn duplicate groups as JSON only.",
    summaries.len(),
    lines.join("\n")
  )
}

/// Ported from dedup.ts parseDetectorResponse — tolerant, returns [] on any failure.
pub fn parse_detector_response(raw: &str) -> Vec<DuplicateGroupCandidate> {
  let Some(json_text) = extract_first_json_object(raw) else {
    return Vec::new();
  };
  let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_text) else {
    return Vec::new();
  };
  let Some(groups_raw) = parsed.get("groups").and_then(serde_json::Value::as_array) else {
    return Vec::new();
  };

  let mut out = Vec::new();
  for group in groups_raw {
    let slugs = group
      .get("slugs")
      .and_then(serde_json::Value::as_array)
      .map(|values| {
        values
          .iter()
          .filter_map(serde_json::Value::as_str)
          .map(str::to_string)
          .collect::<Vec<_>>()
      })
      .unwrap_or_default();
    if slugs.len() < 2 {
      continue;
    }
    let reason = group
      .get("reason")
      .and_then(serde_json::Value::as_str)
      .unwrap_or_default()
      .to_string();
    let confidence = match group.get("confidence").and_then(serde_json::Value::as_str) {
      Some("high") => "high",
      Some("medium") => "medium",
      _ => "low",
    }
    .to_string();
    out.push(DuplicateGroupCandidate { slugs, reason, confidence });
  }
  out
}

/// Ported from dedup.ts extractFirstJsonObject — balanced-brace scan.
fn extract_first_json_object(text: &str) -> Option<&str> {
  let start = text.find('{')?;
  let mut depth = 0usize;
  let mut in_string = false;
  let mut escape = false;
  for (index, ch) in text[start..].char_indices() {
    if escape {
      escape = false;
      continue;
    }
    match ch {
      '\\' => escape = true,
      '"' => in_string = !in_string,
      '{' if !in_string => depth += 1,
      '}' if !in_string => {
        depth -= 1;
        if depth == 0 {
          return Some(&text[start..start + index + ch.len_utf8()]);
        }
      }
      _ => {}
    }
  }
  None
}

/// Ported from dedup.ts normalizeGroupKey — lowercased, sorted, comma-joined.
pub fn normalize_group_key(slugs: &[String]) -> String {
  let mut keys = slugs.iter().map(|slug| slug.to_lowercase()).collect::<Vec<_>>();
  keys.sort();
  keys.join(",")
}

/// Ported from the filter chain in dedup.ts detectDuplicateGroups.
pub fn filter_detected_groups(
  groups: Vec<DuplicateGroupCandidate>,
  valid_slugs: &BTreeSet<String>,
  not_duplicates: &[Vec<String>],
) -> Vec<DuplicateGroupCandidate> {
  let not_dup_keys = not_duplicates
    .iter()
    .map(|group| normalize_group_key(group))
    .collect::<BTreeSet<_>>();

  groups
    .into_iter()
    .map(|mut group| {
      group.slugs.retain(|slug| valid_slugs.contains(slug));
      group
    })
    .filter(|group| group.slugs.len() >= 2)
    .filter(|group| !not_dup_keys.contains(&normalize_group_key(&group.slugs)))
    .collect()
}
```

Move the `use std::collections::BTreeSet;` line up to join the existing `use std::collections::BTreeMap;` as `use std::collections::{BTreeMap, BTreeSet};` (rustfmt will demand it).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p knowledge-core dedup`
Expected: 12 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-core/src/project/dedup.rs
git commit -m "feat: add dedup detector prompt building and response parsing"
```

---

### Task 3: Stage 3 — cross-reference rewrites & merge computation (`dedup.rs`)

**Files:**
- Modify: `crates/knowledge-core/src/project/dedup.rs`

Upstream reference: `upstream_llm_wiki/src/lib/dedup.ts` lines 321–513 (`MERGER_SYSTEM_PROMPT`, `mergeDuplicateGroup`, `buildMergerUserMessage`, `rewriteCrossReferences`, `FIELDS_TO_UNION`). Divergence: `rewriteIndexMd` (lines 534–559) is NOT ported — see plan header.

- [ ] **Step 1: Add failing tests**

Append inside `mod tests`:

```rust
  #[test]
  fn rewrite_cross_references_rewrites_wikilinks_and_related() {
    let mut redirects = std::collections::BTreeMap::new();
    redirects.insert("attention".to_string(), "attention-mechanism".to_string());

    let content = "---\ntype: source\ntitle: Paper\nrelated: [attention, transformer]\n---\n\nSee [[attention]] and [[attention|the attention idea]] and [[transformer]].\n";
    let rewritten = rewrite_cross_references(content, &redirects).unwrap();
    assert!(rewritten.contains("See [[attention-mechanism]] and [[attention-mechanism|the attention idea]] and [[transformer]]."));
    assert!(rewritten.contains("related: [\"attention-mechanism\", \"transformer\"]"));
  }

  #[test]
  fn rewrite_cross_references_dedups_related_case_insensitively() {
    let mut redirects = std::collections::BTreeMap::new();
    redirects.insert("attn".to_string(), "Attention".to_string());

    let content = "---\ntype: concept\ntitle: Foo\nrelated: [attention, attn]\n---\n\nBody.\n";
    let rewritten = rewrite_cross_references(content, &redirects).unwrap();
    assert!(rewritten.contains("related: [\"attention\"]"));
  }

  #[test]
  fn rewrite_cross_references_returns_none_when_unchanged() {
    let mut redirects = std::collections::BTreeMap::new();
    redirects.insert("missing".to_string(), "other".to_string());
    let content = "---\ntype: concept\ntitle: Foo\n---\n\nNo links here.\n";
    assert!(rewrite_cross_references(content, &redirects).is_none());
  }

  #[test]
  fn build_merger_user_message_lists_group_pages() {
    let group = vec![
      DedupPage {
        slug: "attention".to_string(),
        path: "wiki/concepts/attention.md".to_string(),
        content: "content-a".to_string(),
      },
      DedupPage {
        slug: "attention-mechanism".to_string(),
        path: "wiki/concepts/attention-mechanism.md".to_string(),
        content: "content-b".to_string(),
      },
    ];
    let message = build_merger_user_message(&group);
    assert!(message.starts_with("These 2 wiki pages have been confirmed by the user to describe the same topic."));
    assert!(message.contains("## Page 1 (slug: attention)\n\ncontent-a"));
    assert!(message.contains("## Page 2 (slug: attention-mechanism)\n\ncontent-b"));
    assert!(message.contains("the canonical slug will be \"attention\""));
    assert!(message.ends_with("Now output the merged file. First character must be `-`."));
  }

  #[test]
  fn compute_dedup_merge_unions_frontmatter_and_rewrites_references() {
    let group = vec![
      DedupPage {
        slug: "attention".to_string(),
        path: "wiki/concepts/attention.md".to_string(),
        content: "---\ntype: concept\ntitle: Attention\nsources: [\"a.md\"]\ntags: [transformers]\nrelated: [transformer]\n---\n\nOld attention body.\n".to_string(),
      },
      DedupPage {
        slug: "attention-mechanism".to_string(),
        path: "wiki/concepts/attention-mechanism.md".to_string(),
        content: "---\ntype: concept\ntitle: Attention Mechanism\nsources: [\"b.md\"]\n---\n\nMechanism body.\n".to_string(),
      },
    ];
    let other_pages = vec![DedupPage {
      slug: "transformer-paper".to_string(),
      path: "wiki/sources/transformer-paper.md".to_string(),
      content: "---\ntype: source\ntitle: Transformer Paper\nrelated: [attention]\n---\n\nSee [[attention]] for details.\n".to_string(),
    }];
    let llm_output = "---\ntype: concept\ntitle: Attention Mechanism\nsources: [\"b.md\"]\n---\n\nMerged body covering both.\n";

    let outcome = compute_dedup_merge(
      &group,
      "attention-mechanism",
      &other_pages,
      llm_output,
      "2026-06-12",
    )
    .unwrap();

    assert_eq!(outcome.canonical_path, "wiki/concepts/attention-mechanism.md");
    assert!(outcome.canonical_content.contains("Merged body covering both."));
    assert!(outcome.canonical_content.contains("sources: [\"b.md\", \"a.md\"]"));
    assert!(outcome.canonical_content.contains("tags: [\"transformers\"]"));
    assert!(outcome.canonical_content.contains("related: [\"transformer\"]"));
    assert!(outcome.canonical_content.contains("updated: 2026-06-12"));

    assert_eq!(outcome.pages_to_delete, vec!["wiki/concepts/attention.md".to_string()]);

    assert_eq!(outcome.rewrites.len(), 1);
    assert_eq!(outcome.rewrites[0].path, "wiki/sources/transformer-paper.md");
    assert!(outcome.rewrites[0].new_content.contains("See [[attention-mechanism]] for details."));
    assert!(outcome.rewrites[0].new_content.contains("related: [\"attention-mechanism\"]"));

    // backup: both group pages + the rewritten page, pre-merge content
    let backup_paths = outcome.backup.iter().map(|entry| entry.path.as_str()).collect::<Vec<_>>();
    assert_eq!(
      backup_paths,
      vec![
        "wiki/concepts/attention.md",
        "wiki/concepts/attention-mechanism.md",
        "wiki/sources/transformer-paper.md"
      ]
    );
    assert!(outcome.backup[2].content.contains("See [[attention]] for details."));
  }

  #[test]
  fn compute_dedup_merge_rejects_bad_canonical_or_small_group() {
    let page = DedupPage {
      slug: "a".to_string(),
      path: "wiki/concepts/a.md".to_string(),
      content: "---\ntype: concept\ntitle: A\n---\n\nBody.\n".to_string(),
    };
    let err = compute_dedup_merge(&[page.clone(), page.clone()], "missing", &[], "---\n---\n", "2026-06-12")
      .unwrap_err();
    assert!(matches!(err, DedupMergeError::CanonicalNotInGroup(_)));

    let err = compute_dedup_merge(&[page], "a", &[], "---\n---\n", "2026-06-12").unwrap_err();
    assert!(matches!(err, DedupMergeError::GroupTooSmall));
  }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p knowledge-core dedup`
Expected: FAIL — `cannot find function rewrite_cross_references` (compile error).

- [ ] **Step 3: Implement Stage 3**

Add to `dedup.rs` (extend the page_merge import to `use super::page_merge::{merge_array_fields_into_content, parse_frontmatter_array, set_frontmatter_scalar, split_frontmatter_lines_owned, write_frontmatter_array};`). The merger prompt is verbatim from dedup.ts lines 321–331:

```rust
/// Verbatim from upstream_llm_wiki/src/lib/dedup.ts MERGER_SYSTEM_PROMPT.
pub const MERGER_SYSTEM_PROMPT: &str = r#"You are a wiki maintenance assistant. You will be given several wiki pages that all describe the same entity or concept under different names. Merge them into a single coherent wiki page.

Output the COMPLETE merged file (frontmatter + body). The first character of your response MUST be "-" (the opening of "---"). No preamble, no explanation outside the file.

Rules:
- Preserve every distinct factual claim from every input page.
- Eliminate redundancy (don't say the same thing twice across sections).
- Reorganize sections so the structure is logical for the unified topic, not a concatenation of inputs.
- Use [[wikilink]] syntax in the body where the inputs did.
- Frontmatter: keep the standard fields (type, title, created, updated, tags, related, sources). The caller will overwrite sources / tags / related / updated with deterministic unions afterward — your job is to produce a sensible body and reasonable frontmatter shape.
- Pick the most descriptive title. If the inputs use different languages, prefer the language that matches the majority of the body content."#;

const FIELDS_TO_UNION: &[&str] = &["sources", "tags", "related"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DedupRewrite {
  pub path: String,
  pub new_content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DedupBackupEntry {
  pub path: String,
  pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DedupMergeOutcome {
  pub canonical_content: String,
  pub canonical_path: String,
  pub rewrites: Vec<DedupRewrite>,
  pub pages_to_delete: Vec<String>,
  pub backup: Vec<DedupBackupEntry>,
}

#[derive(Debug, thiserror::Error)]
pub enum DedupMergeError {
  #[error("canonical slug \"{0}\" is not in the group")]
  CanonicalNotInGroup(String),
  #[error("dedup merge requires at least 2 pages in the group")]
  GroupTooSmall,
}

/// Ported from dedup.ts buildMergerUserMessage.
pub fn build_merger_user_message(group: &[DedupPage]) -> String {
  let sections = group
    .iter()
    .enumerate()
    .map(|(index, page)| {
      format!("## Page {} (slug: {})\n\n{}\n", index + 1, page.slug, page.content)
    })
    .collect::<Vec<_>>();
  let canonical_hint = group.first().map(|page| page.slug.clone()).unwrap_or_default();
  [
    format!(
      "These {} wiki pages have been confirmed by the user to describe the same topic.",
      group.len()
    ),
    format!(
      "Merge them into a single coherent page (the canonical slug will be \"{canonical_hint}\" or whichever the caller chose)."
    ),
    String::new(),
    sections.join("\n---\n\n"),
    String::new(),
    "Now output the merged file. First character must be `-`.".to_string(),
  ]
  .join("\n")
}

/// Ported from dedup.ts rewriteCrossReferences. Returns Some(new_content)
/// only when something changed (upstream compares strings at the call site).
pub fn rewrite_cross_references(
  content: &str,
  slug_redirects: &BTreeMap<String, String>,
) -> Option<String> {
  let mut out = rewrite_wikilinks(content, slug_redirects);

  let existing = parse_frontmatter_array(&out, "related");
  if !existing.is_empty() {
    let rewritten = existing
      .iter()
      .map(|slug| slug_redirects.get(slug).cloned().unwrap_or_else(|| slug.clone()))
      .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for slug in rewritten {
      if seen.insert(slug.to_lowercase()) {
        unique.push(slug);
      }
    }
    if unique != existing {
      out = write_frontmatter_array(&out, "related", &unique);
    }
  }

  if out == content { None } else { Some(out) }
}

/// [[slug]] and [[slug|alias]] forms — exact slug match on the target portion.
fn rewrite_wikilinks(content: &str, slug_redirects: &BTreeMap<String, String>) -> String {
  let mut out = String::with_capacity(content.len());
  let mut rest = content;
  while let Some(start) = rest.find("[[") {
    let Some(end_offset) = rest[start + 2..].find("]]") else {
      break;
    };
    let inner = &rest[start + 2..start + 2 + end_offset];
    let (target, alias) = match inner.split_once('|') {
      Some((target, alias)) => (target, Some(alias)),
      None => (inner, None),
    };
    out.push_str(&rest[..start]);
    match slug_redirects.get(target) {
      Some(new_slug) => {
        out.push_str("[[");
        out.push_str(new_slug);
        if let Some(alias) = alias {
          out.push('|');
          out.push_str(alias);
        }
        out.push_str("]]");
      }
      None => out.push_str(&rest[start..start + 2 + end_offset + 2]),
    }
    rest = &rest[start + 2 + end_offset + 2..];
  }
  out.push_str(rest);
  out
}

/// Ported from dedup.ts mergeDuplicateGroup, minus the LLM call (the server
/// executor calls the provider and passes llm_output in).
pub fn compute_dedup_merge(
  group: &[DedupPage],
  canonical_slug: &str,
  other_wiki_pages: &[DedupPage],
  llm_output: &str,
  today: &str,
) -> Result<DedupMergeOutcome, DedupMergeError> {
  let canonical = group
    .iter()
    .find(|page| page.slug == canonical_slug)
    .ok_or_else(|| DedupMergeError::CanonicalNotInGroup(canonical_slug.to_string()))?;
  if group.len() < 2 {
    return Err(DedupMergeError::GroupTooSmall);
  }

  let mut merged = llm_output.to_string();
  for page in group {
    merged = merge_array_fields_into_content(&merged, Some(&page.content), FIELDS_TO_UNION);
  }
  merged = set_frontmatter_scalar(&merged, "updated", today);

  let mut slug_redirects = BTreeMap::new();
  for page in group {
    if page.slug != canonical_slug {
      slug_redirects.insert(page.slug.clone(), canonical_slug.to_string());
    }
  }

  let mut rewrites = Vec::new();
  for page in other_wiki_pages {
    if let Some(new_content) = rewrite_cross_references(&page.content, &slug_redirects) {
      rewrites.push(DedupRewrite {
        path: page.path.clone(),
        new_content,
      });
    }
  }

  let mut backup = group
    .iter()
    .map(|page| DedupBackupEntry {
      path: page.path.clone(),
      content: page.content.clone(),
    })
    .collect::<Vec<_>>();
  for rewrite in &rewrites {
    if let Some(original) = other_wiki_pages.iter().find(|page| page.path == rewrite.path) {
      backup.push(DedupBackupEntry {
        path: original.path.clone(),
        content: original.content.clone(),
      });
    }
  }

  let pages_to_delete = group
    .iter()
    .filter(|page| page.slug != canonical_slug)
    .map(|page| page.path.clone())
    .collect::<Vec<_>>();

  Ok(DedupMergeOutcome {
    canonical_content: merged,
    canonical_path: canonical.path.clone(),
    rewrites,
    pages_to_delete,
    backup,
  })
}
```

Check `crates/knowledge-core/Cargo.toml` has `thiserror` (it does — `ProjectRootError` uses it).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p knowledge-core dedup`
Expected: 18 tests PASS.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p knowledge-core --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-core/src/project/dedup.rs
git commit -m "feat: add dedup merge computation with cross-reference rewrites"
```

---

### Task 4: Dedup store (`dedup_store.rs`)

**Files:**
- Create: `crates/knowledge-core/src/project/dedup_store.rs`
- Modify: `crates/knowledge-core/src/project/mod.rs`

Upstream reference: `upstream_llm_wiki/src/lib/dedup-storage.ts` (not-duplicates whitelist: tolerant load, idempotent add, sorted storage, <2 slugs no-op). Group persistence mirrors this repo's `.knowledge/reviews/items.json` pattern (`crates/knowledge-core/src/project/reviews.rs:63-75`). Divergence: paths under `.knowledge/dedup/`.

- [ ] **Step 1: Register module**

In `crates/knowledge-core/src/project/mod.rs`, after `pub mod dedup;` add:

```rust
pub mod dedup_store;
```

- [ ] **Step 2: Write failing tests**

Create `crates/knowledge-core/src/project/dedup_store.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
  use super::*;
  use crate::project::root::ProjectRoot;

  #[test]
  fn load_dedup_store_defaults_when_missing_and_errors_on_garbage() {
    let temp = tempfile::tempdir().unwrap();
    let root = ProjectRoot::new(temp.path()).unwrap();
    assert!(load_dedup_store(&root).unwrap().groups.is_empty());

    std::fs::create_dir_all(temp.path().join(".knowledge/dedup")).unwrap();
    std::fs::write(temp.path().join(".knowledge/dedup/groups.json"), "not json").unwrap();
    assert!(load_dedup_store(&root).is_err());
  }

  #[test]
  fn save_and_load_dedup_store_round_trips() {
    let temp = tempfile::tempdir().unwrap();
    let root = ProjectRoot::new(temp.path()).unwrap();
    let store = DedupStore {
      groups: vec![DedupGroup {
        id: "group-1".to_string(),
        slugs: vec!["a".to_string(), "b".to_string()],
        reason: "same thing".to_string(),
        confidence: "high".to_string(),
        status: "candidate".to_string(),
        created_at: "2026-06-12T00:00:00Z".to_string(),
      }],
    };
    save_dedup_store(&root, &store).unwrap();
    let loaded = load_dedup_store(&root).unwrap();
    assert_eq!(loaded.groups, store.groups);
    let raw = std::fs::read_to_string(temp.path().join(".knowledge/dedup/groups.json")).unwrap();
    assert!(raw.contains("\"createdAt\""));
  }

  #[test]
  fn not_duplicates_load_is_tolerant_and_add_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let root = ProjectRoot::new(temp.path()).unwrap();
    assert!(load_not_duplicates(&root).unwrap().is_empty());

    std::fs::create_dir_all(temp.path().join(".knowledge/dedup")).unwrap();
    std::fs::write(temp.path().join(".knowledge/dedup/not-duplicates.json"), "garbage").unwrap();
    assert!(load_not_duplicates(&root).unwrap().is_empty());

    add_not_duplicate(&root, &["b".to_string(), "a".to_string()]).unwrap();
    add_not_duplicate(&root, &["A".to_string(), "B".to_string()]).unwrap();
    add_not_duplicate(&root, &["only-one".to_string()]).unwrap();
    let groups = load_not_duplicates(&root).unwrap();
    assert_eq!(groups, vec![vec!["a".to_string(), "b".to_string()]]);
  }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p knowledge-core dedup_store`
Expected: FAIL — compile error, `load_dedup_store` not found.

- [ ] **Step 4: Implement the store**

Prepend to `dedup_store.rs`:

```rust
use std::fs;

use serde::{Deserialize, Serialize};

use super::dedup::normalize_group_key;
use super::root::{ProjectRoot, ProjectRootError};

const GROUPS_FILE: &str = ".knowledge/dedup/groups.json";
const NOT_DUPLICATES_FILE: &str = ".knowledge/dedup/not-duplicates.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DedupGroup {
  pub id: String,
  pub slugs: Vec<String>,
  pub reason: String,
  pub confidence: String,
  pub status: String,
  pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DedupStore {
  pub groups: Vec<DedupGroup>,
}

pub fn load_dedup_store(root: &ProjectRoot) -> Result<DedupStore, ProjectRootError> {
  let path = root.safe_join(GROUPS_FILE)?;
  if !path.exists() {
    return Ok(DedupStore::default());
  }
  let raw = fs::read_to_string(path)?;
  serde_json::from_str(&raw).map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))
}

pub fn save_dedup_store(root: &ProjectRoot, store: &DedupStore) -> Result<(), ProjectRootError> {
  let path = root.safe_join(GROUPS_FILE)?;
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  let raw = serde_json::to_string_pretty(store)
    .map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))?;
  fs::write(path, raw)?;
  Ok(())
}

/// Ported from dedup-storage.ts loadNotDuplicates — missing or garbage file → [].
pub fn load_not_duplicates(root: &ProjectRoot) -> Result<Vec<Vec<String>>, ProjectRootError> {
  let path = root.safe_join(NOT_DUPLICATES_FILE)?;
  if !path.exists() {
    return Ok(Vec::new());
  }
  let raw = fs::read_to_string(path)?;
  Ok(serde_json::from_str(&raw).unwrap_or_default())
}

/// Ported from dedup-storage.ts addNotDuplicate — idempotent, sorted, <2 slugs no-op.
pub fn add_not_duplicate(root: &ProjectRoot, slugs: &[String]) -> Result<(), ProjectRootError> {
  if slugs.len() < 2 {
    return Ok(());
  }
  let mut groups = load_not_duplicates(root)?;
  let key = normalize_group_key(slugs);
  if groups.iter().any(|group| normalize_group_key(group) == key) {
    return Ok(());
  }
  let mut sorted = slugs.to_vec();
  sorted.sort();
  groups.push(sorted);

  let path = root.safe_join(NOT_DUPLICATES_FILE)?;
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  let raw = serde_json::to_string_pretty(&groups)
    .map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))?;
  fs::write(path, raw)?;
  Ok(())
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p knowledge-core dedup_store`
Expected: 3 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-core/src/project/dedup_store.rs crates/knowledge-core/src/project/mod.rs
git commit -m "feat: add dedup group store and not-duplicates whitelist"
```

---

### Task 5: Embedding prefilter (`retrieval/dedup.rs`)

**Files:**
- Create: `crates/knowledge-server/src/retrieval/dedup.rs`
- Modify: `crates/knowledge-server/src/retrieval/mod.rs`
- Modify: `crates/knowledge-server/src/retrieval/service.rs` (visibility only)

This is the server-side divergence: upstream sends every entity/concept page to the detector LLM; we first narrow to pages whose chunk embeddings are mutually similar, reusing the existing embedding index (`project_embedding_chunks`).

- [ ] **Step 1: Make service helpers reusable**

In `crates/knowledge-server/src/retrieval/service.rs`:
- Line ~132: `async fn ensure_project_embeddings(` → `pub(crate) async fn ensure_project_embeddings(`
- Line ~419: `fn cosine_similarity(` → `pub(crate) fn cosine_similarity(`

- [ ] **Step 2: Register module**

In `crates/knowledge-server/src/retrieval/mod.rs`:

```rust
pub mod chunker;
pub mod dedup;
pub mod service;
pub mod store;
```

- [ ] **Step 3: Write failing tests**

Create `crates/knowledge-server/src/retrieval/dedup.rs` with only tests:

```rust
#[cfg(test)]
mod tests {
  use super::*;
  use crate::retrieval::store::StoredChunk;

  fn chunk(relative_path: &str, embedding: Vec<f32>) -> StoredChunk {
    StoredChunk {
      page_id: relative_path.to_string(),
      relative_path: relative_path.to_string(),
      title: "t".to_string(),
      embedding_model: "mock-embedding".to_string(),
      chunk_index: 0,
      heading_path: String::new(),
      chunk_text: "text".to_string(),
      content_hash: "hash".to_string(),
      embedding,
    }
  }

  #[test]
  fn selects_pages_with_mutually_similar_embeddings() {
    let chunks = vec![
      chunk("wiki/concepts/attention.md", vec![0.0, 1.0, 0.0]),
      chunk("wiki/concepts/attention-mechanism.md", vec![0.0, 1.0, 0.0]),
      chunk("wiki/entities/rope.md", vec![1.0, 0.0, 0.0]),
    ];
    let pages = select_dedup_candidate_pages(&chunks, 0.8, 40);
    assert_eq!(
      pages,
      vec![
        "wiki/concepts/attention-mechanism.md".to_string(),
        "wiki/concepts/attention.md".to_string()
      ]
    );
  }

  #[test]
  fn ignores_pages_outside_entities_and_concepts() {
    let chunks = vec![
      chunk("wiki/concepts/attention.md", vec![0.0, 1.0, 0.0]),
      chunk("wiki/sources/transformer-paper.md", vec![0.0, 1.0, 0.0]),
    ];
    assert!(select_dedup_candidate_pages(&chunks, 0.8, 40).is_empty());
  }

  #[test]
  fn averages_chunks_per_page_and_respects_max() {
    let chunks = vec![
      chunk("wiki/concepts/a.md", vec![0.0, 1.0, 0.0]),
      chunk("wiki/concepts/a.md", vec![0.0, 1.0, 0.0]),
      chunk("wiki/concepts/b.md", vec![0.0, 1.0, 0.0]),
      chunk("wiki/concepts/c.md", vec![0.0, 1.0, 0.0]),
    ];
    let pages = select_dedup_candidate_pages(&chunks, 0.8, 2);
    assert_eq!(pages.len(), 2);
  }
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test -p knowledge-server retrieval::dedup`
Expected: FAIL — compile error.

- [ ] **Step 5: Implement the prefilter**

Prepend to `retrieval/dedup.rs`:

```rust
use std::collections::{BTreeMap, BTreeSet};

use crate::retrieval::service::cosine_similarity;
use crate::retrieval::store::StoredChunk;

pub const DEDUP_SIMILARITY_THRESHOLD: f32 = 0.80;
pub const DEDUP_MAX_CANDIDATE_PAGES: usize = 40;

/// Mean-embedding per entity/concept page, pairwise cosine, union of pages
/// appearing in any pair at or above the threshold. Sorted, truncated.
pub fn select_dedup_candidate_pages(
  chunks: &[StoredChunk],
  threshold: f32,
  max_pages: usize,
) -> Vec<String> {
  let mut sums: BTreeMap<&str, (Vec<f32>, usize)> = BTreeMap::new();
  for chunk in chunks {
    let path = chunk.relative_path.as_str();
    if !(path.starts_with("wiki/entities/") || path.starts_with("wiki/concepts/")) {
      continue;
    }
    if chunk.embedding.is_empty() {
      continue;
    }
    let entry = sums.entry(path).or_insert_with(|| (vec![0.0; chunk.embedding.len()], 0));
    if entry.0.len() != chunk.embedding.len() {
      continue;
    }
    for (slot, value) in entry.0.iter_mut().zip(chunk.embedding.iter()) {
      *slot += value;
    }
    entry.1 += 1;
  }

  let means = sums
    .into_iter()
    .map(|(path, (sum, count))| {
      let mean = sum.into_iter().map(|value| value / count as f32).collect::<Vec<_>>();
      (path, mean)
    })
    .collect::<Vec<_>>();

  let mut candidates = BTreeSet::new();
  for (left_index, (left_path, left_mean)) in means.iter().enumerate() {
    for (right_path, right_mean) in means.iter().skip(left_index + 1) {
      if let Some(score) = cosine_similarity(left_mean, right_mean)
        && score >= threshold
      {
        candidates.insert((*left_path).to_string());
        candidates.insert((*right_path).to_string());
      }
    }
  }

  let mut pages = candidates.into_iter().collect::<Vec<_>>();
  pages.truncate(max_pages);
  pages
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p knowledge-server retrieval::dedup`
Expected: 3 tests PASS.

- [ ] **Step 7: Clippy + commit**

Run: `cargo clippy -p knowledge-server --all-targets -- -D warnings`
Expected: clean. (If `let ... && let` chain syntax trips edition lints, fall back to nested `if let Some(score) = ... { if score >= threshold { ... } }` — but edition 2024 supports let-chains; service.rs/executors.rs already use them.)

```bash
git add crates/knowledge-server/src/retrieval/dedup.rs crates/knowledge-server/src/retrieval/mod.rs crates/knowledge-server/src/retrieval/service.rs
git commit -m "feat: add embedding prefilter for dedup candidate pages"
```

---

### Task 6: Rust mock scenario `DedupSuccess`

**Files:**
- Modify: `tests/rust-integration/tests/support/mock_openai.rs` (4-space indent)

The embeddings handler increments the shared `request_count`, so the dedup scenario must branch on **system-prompt content**, never on `request_index`. The detector system prompt contains `"Identify groups of slugs"`; the merger contains `"describe the same entity or concept under different names"`.

- [ ] **Step 1: Add the enum variant and constructor**

In the `MockScenario` enum (line ~14), add `DedupSuccess,` after `IngestThenSweepLlmSuccess,`. In the `impl MockScenario` block, add:

```rust
    #[allow(dead_code)]
    pub fn dedup_success() -> Self {
        Self::DedupSuccess
    }
```

- [ ] **Step 2: Add a system-message helper**

Near `payload_has_image_block` (line ~628), add:

```rust
fn system_message_text(payload: &Value) -> String {
    payload
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|message| message.get("role").and_then(Value::as_str) == Some("system"))
        .and_then(|message| message.get("content").and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}
```

- [ ] **Step 3: Add the scenario arm**

In the `match scenario` inside `chat_completions_json` (line ~200), add an arm. Use the existing arms' response shape (id/object/created/model/choices/usage):

```rust
        MockScenario::DedupSuccess => {
            let system = system_message_text(&payload);
            let content = if system.contains("Identify groups of slugs") {
                "{\"groups\":[{\"slugs\":[\"attention\",\"attention-mechanism\"],\"reason\":\"Both describe the attention mechanism.\",\"confidence\":\"high\"}]}".to_string()
            } else if system.contains("describe the same entity or concept under different names") {
                "---\ntype: concept\ntitle: Attention Mechanism\ncreated: 2026-06-01\nsources: []\ntags: []\nrelated: []\n---\n\n# Attention Mechanism\n\nAttention focuses computation on relevant tokens across the sequence.\n".to_string()
            } else {
                "{\"groups\": []}".to_string()
            };
            (
                StatusCode::OK,
                Json(json!({
                  "id": "chatcmpl-mock-dedup",
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

Also check the streaming branch in `chat_completions` (line ~131): if it has an exhaustive `match scenario`, add `MockScenario::DedupSuccess => {}`-style fallthrough to whatever the `Success` arm does (dedup never streams, so the canned stream answer is fine). If it only special-cases `RetryableError`/`InvalidRequest` with a default arm, no change needed.

- [ ] **Step 4: Verify it compiles**

Run: `cargo test -p rust-integration --test search_api -- --test-threads=1`
Expected: existing tests still PASS (requires Postgres/Redis from `npm run docker:up` ports 55432/56379).

- [ ] **Step 5: Commit**

```bash
git add tests/rust-integration/tests/support/mock_openai.rs
git commit -m "test: add prompt-aware dedup scenario to mock openai server"
```

---

### Task 7: Detect — executor, route, integration test

**Files:**
- Modify: `crates/knowledge-server/src/tasks/executors.rs` (2-space indent)
- Modify: `crates/knowledge-server/src/projects/routes.rs` (4-space indent)
- Create: `tests/rust-integration/tests/dedup_api.rs` (4-space indent)

- [ ] **Step 1: Write the failing integration test**

Create `tests/rust-integration/tests/dedup_api.rs`. Harness helpers are copied from `tests/rust-integration/tests/search_api.rs:519-582` (they are per-test-binary, so duplicate them here — that is the established pattern):

```rust
mod support;

use std::fs;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::config::AppConfig;
use knowledge_server::tasks::{scheduler, store};
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{Value, json};
use support::TestEnvironment;
use support::mock_openai::{MockOpenAiServer, MockScenario};
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn dedup_detect_finds_candidate_groups() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("dedup-detect").await.unwrap();
    let mock = MockOpenAiServer::start(MockScenario::dedup_success())
        .await
        .unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("dedup-detect-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    configure_mock_provider(&state, &mock).await;
    write_dedup_fixture_pages(&project_root);

    let task_id = enqueue_detect(state.clone(), &cookie, &csrf, &project_id).await;
    wait_for_task_terminal(&state, &task_id).await;
    let task = store::get_task_by_id(&state, &task_id).await.unwrap();
    assert_eq!(task.status, "succeeded");

    let overview = get_dedup_overview(state.clone(), &cookie, &project_id).await;
    let groups = overview.get("groups").and_then(Value::as_array).unwrap();
    assert_eq!(groups.len(), 1);
    let group = &groups[0];
    assert_eq!(
        group.get("slugs").and_then(Value::as_array).map(Vec::len),
        Some(2)
    );
    assert_eq!(group.get("confidence").and_then(Value::as_str), Some("high"));
    assert_eq!(group.get("status").and_then(Value::as_str), Some("candidate"));
    assert!(group.get("id").and_then(Value::as_str).is_some());
}

async fn configure_mock_provider(
    state: &knowledge_server::app::state::AppState,
    mock: &MockOpenAiServer,
) {
    sqlx::query(
        "UPDATE system_settings
     SET provider_mode = $1,
         provider_base_url = $2,
         provider_api_key = $3,
         provider_model = $4,
         provider_embedding_model = $5,
         provider_timeout_seconds = $6",
    )
    .bind("openai-compatible")
    .bind(mock.base_url())
    .bind("test-key")
    .bind("mock-model")
    .bind("mock-embedding")
    .bind(30_i64)
    .execute(&state.pool)
    .await
    .unwrap();
}

fn write_dedup_fixture_pages(project_root: &std::path::Path) {
    fs::write(
        project_root.join("wiki/concepts/attention.md"),
        "---\ntype: concept\ntitle: Attention\nsources: [\"a.md\"]\ntags: [transformers]\nrelated: [transformer]\n---\n\n# Attention\n\nAttention weighs token relevance.\n",
    )
    .unwrap();
    fs::write(
        project_root.join("wiki/concepts/attention-mechanism.md"),
        "---\ntype: concept\ntitle: Attention Mechanism\nsources: [\"b.md\"]\n---\n\n# Attention Mechanism\n\nThe attention mechanism scores pairs of tokens.\n",
    )
    .unwrap();
    fs::write(
        project_root.join("wiki/entities/rope.md"),
        "---\ntype: entity\ntitle: RoPE\nsources: []\n---\n\n# RoPE\n\nRoPE rotates query vectors.\n",
    )
    .unwrap();
    // Body deliberately avoids the word "attention" outside the wikilink so the
    // fake keyword embedding does not pull this source page into the prefilter
    // (it lives under wiki/sources/ which the prefilter excludes anyway).
    fs::write(
        project_root.join("wiki/sources/transformer-paper.md"),
        "---\ntype: source\ntitle: Transformer Paper\nrelated: [attention]\n---\n\n# Transformer Paper\n\nSee [[attention]] for details.\n",
    )
    .unwrap();
    fs::write(
        project_root.join("wiki/index.md"),
        "# Index\n\n- [[attention]] - token relevance weighting\n- [[attention-mechanism]] - pairwise token scoring\n- [[rope]] - rotary embeddings\n",
    )
    .unwrap();
}

async fn enqueue_detect(
    state: knowledge_server::app::state::AppState,
    cookie: &str,
    csrf: &str,
    project_id: &str,
) -> String {
    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/dedup:detect"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let payload = read_json(response.into_body()).await;
    payload
        .get("taskId")
        .and_then(Value::as_str)
        .unwrap()
        .to_string()
}

async fn get_dedup_overview(
    state: knowledge_server::app::state::AppState,
    cookie: &str,
    project_id: &str,
) -> Value {
    let response = build_app(state)
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_id}/dedup"))
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    read_json(response.into_body()).await
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
    let csrf = body
        .get("csrfToken")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();

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
    for _ in 0..20 {
        let progressed = scheduler::run_scheduler_tick(state).await.unwrap_or(false);
        let task = store::get_task_by_id(state, task_id).await.unwrap();
        if matches!(task.status.as_str(), "succeeded" | "failed" | "cancelled") {
            return;
        }
        if !progressed {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    panic!("task did not reach a terminal state");
}
```

(Project scaffolding pre-creates `wiki/concepts`, `wiki/entities`, `wiki/sources`, `wiki/index.md` — `search_api.rs` writes fixture files the same way without `create_dir_all`.)

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p rust-integration --test dedup_api -- --test-threads=1`
Expected: FAIL — `dedup:detect` route returns 404 / task fails with "unsupported task type".

- [ ] **Step 3: Add the detect route**

In `crates/knowledge-server/src/projects/routes.rs`:

Add to the `use knowledge_core::project::...` imports:

```rust
use knowledge_core::project::dedup_store::{load_dedup_store, load_not_duplicates, save_dedup_store};
```

In `router()` after the `reviews/{review_id}` route (line ~130), add:

```rust
        .route(
            "/api/projects/{project_id}/dedup",
            get(dedup_overview_handler),
        )
        .route(
            "/api/projects/{project_id}/dedup:detect",
            post(detect_dedup_handler),
        )
```

(axum 0.8 supports the literal `:detect` suffix on a static segment — same as the existing `reviews:sweep`. Mixed `{group_id}:merge` is NOT supported, which is why Tasks 8/9 use `/merge` and `/dismiss` sub-paths.)

Add the handlers next to `sweep_reviews_handler` (line ~1090), modeled on it and on `audit_logs_handler`:

```rust
async fn dedup_overview_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
    let root = project_root_for_id(&state, &project_id).await?;
    let store = load_dedup_store(&root).map_err(|error| ApiError::internal(error.to_string()))?;
    let not_duplicates =
        load_not_duplicates(&root).map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(json!({
      "groups": store.groups,
      "notDuplicates": not_duplicates
    })))
}

async fn detect_dedup_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let session = authorized_session(&state, &headers, Some(&project_id)).await?;
    validate_csrf(&headers, &session.csrf_token)?;
    let task = create_queued_task(
        &state,
        CreateTaskRecord {
            project_id: project_id.clone(),
            task_type: "project.dedup_detect".to_string(),
            title: "Detect duplicate pages".to_string(),
            relative_path: None,
            detail: json!({}),
            created_by: session.user_id.clone(),
        },
        json!({}),
    )
    .await?;
    append_audit_log(
        &state,
        CreateAuditLog {
            project_id: Some(project_id),
            actor_id: session.user_id,
            action: "dedup.detect.enqueued".to_string(),
            target_type: "dedup".to_string(),
            target_id: "batch".to_string(),
            task_id: Some(task.id.clone()),
            summary: "Queued duplicate detection".to_string(),
            metadata: json!({}),
        },
    )
    .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
          "taskId": task.id,
          "status": task.status
        })),
    ))
}
```

- [ ] **Step 4: Add the detect executor**

In `crates/knowledge-server/src/tasks/executors.rs`:

Add imports (extend existing blocks):

```rust
use std::collections::BTreeSet;

use crate::retrieval::dedup::{
  select_dedup_candidate_pages, DEDUP_MAX_CANDIDATE_PAGES, DEDUP_SIMILARITY_THRESHOLD,
};
use crate::retrieval::service::{ensure_project_embeddings, load_embedding_config};
use crate::retrieval::store::load_project_chunks;
use knowledge_core::project::dedup::{
  build_detector_user_message, extract_entity_summary, filter_detected_groups,
  parse_detector_response, DETECTOR_SYSTEM_PROMPT,
};
use knowledge_core::project::dedup_store::{
  load_not_duplicates, save_dedup_store, DedupGroup, DedupStore,
};
```

Add the dispatch arm after `"project.update_review" => ...` (line ~56):

```rust
    "project.dedup_detect" => run_dedup_detect_executor(state, &task).await.map_err(Into::into),
```

Add the executor (near `run_manual_review_sweep_executor`):

```rust
async fn run_dedup_detect_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let provider = load_ingest_provider(state)
    .await?
    .ok_or_else(|| ApiError::bad_request("dedup detection requires an openai-compatible provider"))?;
  let embedding_config = load_embedding_config(state)
    .await?
    .ok_or_else(|| ApiError::bad_request("dedup detection requires a configured embedding model"))?;

  ensure_project_embeddings(state, &task.project_id, &root, &embedding_config).await?;
  let chunks = load_project_chunks(&state.pool, &task.project_id).await?;
  let candidate_paths =
    select_dedup_candidate_pages(&chunks, DEDUP_SIMILARITY_THRESHOLD, DEDUP_MAX_CANDIDATE_PAGES);

  let mut summaries = Vec::new();
  let mut seen_slugs = BTreeSet::new();
  for path in &candidate_paths {
    let content = try_read_project_file(&root, path);
    if content.is_empty() {
      continue;
    }
    if let Some(summary) = extract_entity_summary(path, &content)
      && seen_slugs.insert(summary.slug.clone())
    {
      summaries.push(summary);
    }
  }

  let mut groups = Vec::new();
  if summaries.len() >= 2 {
    let response = provider
      .complete_text(ProviderTextRequest {
        system_prompt: DETECTOR_SYSTEM_PROMPT.to_string(),
        user_prompt: build_detector_user_message(&summaries),
      })
      .await
      .map_err(TaskExecutionError::from_provider_error)
      .map_err(TaskExecutionError::into_api_error)?;

    let valid_slugs = summaries.iter().map(|summary| summary.slug.clone()).collect::<BTreeSet<_>>();
    let not_duplicates =
      load_not_duplicates(&root).map_err(|error| ApiError::internal(error.to_string()))?;
    let detected = filter_detected_groups(
      parse_detector_response(&response.text),
      &valid_slugs,
      &not_duplicates,
    );

    let now = OffsetDateTime::now_utc()
      .format(&Rfc3339)
      .map_err(|_| ApiError::internal("failed to format timestamp"))?;
    for candidate in detected {
      groups.push(DedupGroup {
        id: uuid::Uuid::new_v4().to_string(),
        slugs: candidate.slugs,
        reason: candidate.reason,
        confidence: candidate.confidence,
        status: "candidate".to_string(),
        created_at: now.clone(),
      });
    }
  }

  let group_count = groups.len();
  save_dedup_store(&root, &DedupStore { groups })
    .map_err(|error| ApiError::internal(error.to_string()))?;

  Ok(json!({
    "groupCount": group_count,
    "candidatePages": candidate_paths
  }))
}
```

Note: each detect run REPLACES the stored groups (re-detection reflects current wiki state; dismissed pairs stay excluded via the whitelist).

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p rust-integration --test dedup_api -- --test-threads=1`
Expected: PASS.

- [ ] **Step 6: Clippy + commit**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/knowledge-server/src/tasks/executors.rs crates/knowledge-server/src/projects/routes.rs tests/rust-integration/tests/dedup_api.rs
git commit -m "feat: add dedup detect task, routes, and integration coverage"
```

---

### Task 8: Merge — executor, route, integration test

**Files:**
- Modify: `crates/knowledge-server/src/tasks/executors.rs`
- Modify: `crates/knowledge-server/src/projects/routes.rs`
- Modify: `tests/rust-integration/tests/dedup_api.rs`

Upstream reference: `mergeDuplicateGroup` call site semantics in dedup.ts — the executor does the I/O the upstream UI does (write canonical, write rewrites, delete pages, store backup).

- [ ] **Step 1: Write the failing integration test**

Append to `tests/rust-integration/tests/dedup_api.rs`:

```rust
#[tokio::test]
async fn dedup_merge_unifies_pages_and_rewrites_references() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("dedup-merge").await.unwrap();
    let mock = MockOpenAiServer::start(MockScenario::dedup_success())
        .await
        .unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("dedup-merge-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    configure_mock_provider(&state, &mock).await;
    write_dedup_fixture_pages(&project_root);

    let detect_task = enqueue_detect(state.clone(), &cookie, &csrf, &project_id).await;
    wait_for_task_terminal(&state, &detect_task).await;
    let overview = get_dedup_overview(state.clone(), &cookie, &project_id).await;
    let group_id = overview["groups"][0]["id"].as_str().unwrap().to_string();

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/projects/{project_id}/dedup/groups/{group_id}/merge"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "canonicalSlug": "attention-mechanism" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let payload = read_json(response.into_body()).await;
    let merge_task = payload["taskId"].as_str().unwrap().to_string();
    wait_for_task_terminal(&state, &merge_task).await;
    let task = store::get_task_by_id(&state, &merge_task).await.unwrap();
    assert_eq!(task.status, "succeeded");

    // non-canonical page deleted, canonical holds merged content + unioned sources
    assert!(!project_root.join("wiki/concepts/attention.md").exists());
    let canonical =
        fs::read_to_string(project_root.join("wiki/concepts/attention-mechanism.md")).unwrap();
    assert!(canonical.contains("Attention focuses computation on relevant tokens across the sequence."));
    assert!(canonical.contains("\"a.md\""));
    assert!(canonical.contains("\"b.md\""));

    // cross-references rewritten in other pages
    let paper = fs::read_to_string(project_root.join("wiki/sources/transformer-paper.md")).unwrap();
    assert!(paper.contains("[[attention-mechanism]]"));
    assert!(!paper.contains("[[attention]] "));
    assert!(paper.contains("related: [\"attention-mechanism\"]"));

    // index handled by the delete cascade: old entry gone, canonical entry intact
    let index = fs::read_to_string(project_root.join("wiki/index.md")).unwrap();
    assert!(index.contains("[[attention-mechanism]]"));
    assert!(!index.contains("[[attention]] "));

    // embedding chunks for the deleted page were purged
    let page_ids: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT page_id FROM project_embedding_chunks WHERE project_id = $1 ORDER BY page_id",
    )
    .bind(&project_id)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert!(
        !page_ids
            .iter()
            .any(|(page_id,)| page_id == "wiki/concepts/attention.md")
    );

    // group consumed; backup snapshot exists
    let overview = get_dedup_overview(state.clone(), &cookie, &project_id).await;
    assert!(overview["groups"].as_array().unwrap().is_empty());
    let backups_root = project_root.join(".knowledge/dedup/backups");
    assert!(backups_root.exists());
    assert!(fs::read_dir(&backups_root).unwrap().next().is_some());
}
```

Note the deliberate `"[[attention]] "` (trailing space) assertions — plain `!contains("[[attention]]")` would false-fail because `[[attention-mechanism]]` contains that substring.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p rust-integration --test dedup_api -- --test-threads=1`
Expected: new test FAILS — merge route 404.

- [ ] **Step 3: Add the merge route**

In `routes.rs`, add after the `dedup:detect` route:

```rust
        .route(
            "/api/projects/{project_id}/dedup/groups/{group_id}/merge",
            post(merge_dedup_group_handler),
        )
```

Add the request struct next to `UpdateReviewRequest`:

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MergeDedupGroupRequest {
    pub canonical_slug: String,
}
```

Add the handler (pre-validates so the user gets a 4xx instead of a failed task):

```rust
async fn merge_dedup_group_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, group_id)): Path<(String, String)>,
    Json(payload): Json<MergeDedupGroupRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session = authorized_session(&state, &headers, Some(&project_id)).await?;
    validate_csrf(&headers, &session.csrf_token)?;
    let root = project_root_for_id(&state, &project_id).await?;
    let store = load_dedup_store(&root).map_err(|error| ApiError::internal(error.to_string()))?;
    let group = store
        .groups
        .iter()
        .find(|group| group.id == group_id)
        .ok_or_else(|| ApiError::not_found("dedup group not found"))?;
    if !group.slugs.contains(&payload.canonical_slug) {
        return Err(ApiError::bad_request("canonical slug is not part of the group"));
    }
    let task = create_queued_task(
        &state,
        CreateTaskRecord {
            project_id: project_id.clone(),
            task_type: "project.dedup_merge".to_string(),
            title: format!("Merge duplicate group {}", group.slugs.join(" / ")),
            relative_path: None,
            detail: json!({}),
            created_by: session.user_id.clone(),
        },
        json!({
          "groupId": group_id,
          "canonicalSlug": payload.canonical_slug
        }),
    )
    .await?;
    append_audit_log(
        &state,
        CreateAuditLog {
            project_id: Some(project_id),
            actor_id: session.user_id,
            action: "dedup.merge.enqueued".to_string(),
            target_type: "dedup".to_string(),
            target_id: group_id,
            task_id: Some(task.id.clone()),
            summary: format!("Queued duplicate merge into {}", payload.canonical_slug),
            metadata: json!({ "canonicalSlug": payload.canonical_slug }),
        },
    )
    .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
          "taskId": task.id,
          "status": task.status
        })),
    ))
}
```

- [ ] **Step 4: Add the merge executor**

In `executors.rs`, extend imports:

```rust
use crate::retrieval::store::{delete_pages, load_project_chunks};
use knowledge_core::project::dedup::{
  build_detector_user_message, build_merger_user_message, compute_dedup_merge,
  collect_all_wiki_pages, collect_entity_pages, extract_entity_summary, filter_detected_groups,
  parse_detector_response, DETECTOR_SYSTEM_PROMPT, MERGER_SYSTEM_PROMPT,
};
use knowledge_core::project::dedup_store::{
  load_dedup_store, load_not_duplicates, save_dedup_store, DedupGroup, DedupStore,
};
use knowledge_core::project::wiki_pages::{delete_wiki_pages_with_refs, save_wiki_page};
```

(These merge with Task 7's import lines — keep one combined block; `save_wiki_page`/`delete_wiki_pages_with_refs` may already be imported in routes.rs but not in executors.rs.)

Add the dispatch arm:

```rust
    "project.dedup_merge" => run_dedup_merge_executor(state, &task).await.map_err(Into::into),
```

Add the executor:

```rust
async fn run_dedup_merge_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let group_id = read_string(&task.payload, "groupId")?;
  let canonical_slug = read_string(&task.payload, "canonicalSlug")?;
  let root = project_root_for_id(state, &task.project_id).await?;
  let provider = load_ingest_provider(state)
    .await?
    .ok_or_else(|| ApiError::bad_request("dedup merge requires an openai-compatible provider"))?;

  let mut store = load_dedup_store(&root).map_err(|error| ApiError::internal(error.to_string()))?;
  let group = store
    .groups
    .iter()
    .find(|group| group.id == group_id)
    .cloned()
    .ok_or_else(|| ApiError::bad_request("dedup group not found"))?;

  // Resolve slugs to pages. Slug ambiguity across entity/concept dirs is an
  // error (divergence: upstream UI holds paths directly).
  let entity_pages = collect_entity_pages(&root).map_err(|error| ApiError::internal(error.to_string()))?;
  let mut group_pages = Vec::new();
  for slug in &group.slugs {
    let matches = entity_pages
      .iter()
      .filter(|page| &page.slug == slug)
      .collect::<Vec<_>>();
    if matches.len() > 1 {
      return Err(ApiError::bad_request(format!(
        "slug \"{slug}\" is ambiguous across multiple wiki pages"
      )));
    }
    let page = matches
      .first()
      .ok_or_else(|| ApiError::bad_request(format!("page for slug \"{slug}\" not found")))?;
    group_pages.push((*page).clone());
  }

  let group_paths = group_pages.iter().map(|page| page.path.clone()).collect::<BTreeSet<_>>();
  let other_pages = collect_all_wiki_pages(&root)
    .map_err(|error| ApiError::internal(error.to_string()))?
    .into_iter()
    .filter(|page| !group_paths.contains(&page.path) && page.path != "wiki/index.md")
    .collect::<Vec<_>>();

  let response = provider
    .complete_text(ProviderTextRequest {
      system_prompt: MERGER_SYSTEM_PROMPT.to_string(),
      user_prompt: build_merger_user_message(&group_pages),
    })
    .await
    .map_err(TaskExecutionError::from_provider_error)
    .map_err(TaskExecutionError::into_api_error)?;

  let now = OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format timestamp"))?;
  let today = now[..10].to_string();
  let outcome = compute_dedup_merge(&group_pages, &canonical_slug, &other_pages, &response.text, &today)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;

  // Backup BEFORE writing (upstream stores to .llm-wiki/page-history/; we
  // use .knowledge/dedup/backups/<stamp>/).
  let stamp = now.replace([':', '.'], "-");
  for entry in &outcome.backup {
    let backup_path = root
      .safe_join(&format!(".knowledge/dedup/backups/{stamp}/{}", entry.path))
      .map_err(|error| ApiError::internal(error.to_string()))?;
    if let Some(parent) = backup_path.parent() {
      std::fs::create_dir_all(parent).map_err(|error| ApiError::internal(error.to_string()))?;
    }
    std::fs::write(&backup_path, &entry.content)
      .map_err(|error| ApiError::internal(error.to_string()))?;
  }

  save_wiki_page(&root, &outcome.canonical_path, &outcome.canonical_content)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  for rewrite in &outcome.rewrites {
    save_wiki_page(&root, &rewrite.path, &rewrite.new_content)
      .map_err(|error| ApiError::bad_request(error.to_string()))?;
  }
  let delete_result = delete_wiki_pages_with_refs(&root, &outcome.pages_to_delete)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  delete_pages(&state.pool, &task.project_id, &outcome.pages_to_delete).await?;

  let removed_slugs = group
    .slugs
    .iter()
    .filter(|slug| *slug != &canonical_slug)
    .cloned()
    .collect::<BTreeSet<_>>();
  store.groups.retain(|candidate| {
    candidate.id != group_id && !candidate.slugs.iter().any(|slug| removed_slugs.contains(slug))
  });
  save_dedup_store(&root, &store).map_err(|error| ApiError::internal(error.to_string()))?;

  Ok(json!({
    "canonicalPath": outcome.canonical_path,
    "deletedPaths": delete_result.deleted_paths,
    "rewrittenFiles": delete_result.rewritten_files,
    "rewrites": outcome.rewrites.iter().map(|rewrite| rewrite.path.clone()).collect::<Vec<_>>()
  }))
}
```

If `save_wiki_page` rejects non-wiki paths (it does — `WikiPageError::NotWikiPage` for anything outside `wiki/`), this is fine: canonical and rewrites are always under `wiki/`.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p rust-integration --test dedup_api -- --test-threads=1`
Expected: both tests PASS.

- [ ] **Step 6: Clippy + commit**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/knowledge-server/src/tasks/executors.rs crates/knowledge-server/src/projects/routes.rs tests/rust-integration/tests/dedup_api.rs
git commit -m "feat: add dedup merge task with backups and reference rewrites"
```

---

### Task 9: Dismiss — route + integration test

**Files:**
- Modify: `crates/knowledge-server/src/projects/routes.rs`
- Modify: `tests/rust-integration/tests/dedup_api.rs`

Upstream reference: `dedup-storage.ts` `addNotDuplicate`. Dismiss is synchronous (no LLM, no task) — it whitelists the pair and drops the group.

- [ ] **Step 1: Write the failing integration test**

Append to `tests/rust-integration/tests/dedup_api.rs`:

```rust
#[tokio::test]
async fn dedup_dismiss_whitelists_group_for_future_detection() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("dedup-dismiss").await.unwrap();
    let mock = MockOpenAiServer::start(MockScenario::dedup_success())
        .await
        .unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("dedup-dismiss-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    configure_mock_provider(&state, &mock).await;
    write_dedup_fixture_pages(&project_root);

    let detect_task = enqueue_detect(state.clone(), &cookie, &csrf, &project_id).await;
    wait_for_task_terminal(&state, &detect_task).await;
    let overview = get_dedup_overview(state.clone(), &cookie, &project_id).await;
    let group_id = overview["groups"][0]["id"].as_str().unwrap().to_string();

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/projects/{project_id}/dedup/groups/{group_id}/dismiss"
                ))
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = read_json(response.into_body()).await;
    assert_eq!(payload.get("dismissed").and_then(Value::as_bool), Some(true));

    let overview = get_dedup_overview(state.clone(), &cookie, &project_id).await;
    assert!(overview["groups"].as_array().unwrap().is_empty());
    let not_duplicates = overview["notDuplicates"].as_array().unwrap();
    assert_eq!(not_duplicates.len(), 1);

    // re-detection must respect the whitelist
    let detect_task = enqueue_detect(state.clone(), &cookie, &csrf, &project_id).await;
    wait_for_task_terminal(&state, &detect_task).await;
    let overview = get_dedup_overview(state, &cookie, &project_id).await;
    assert!(overview["groups"].as_array().unwrap().is_empty());
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p rust-integration --test dedup_api -- --test-threads=1`
Expected: new test FAILS — dismiss route 404.

- [ ] **Step 3: Add the dismiss route + handler**

In `routes.rs`, extend the dedup_store import with `add_not_duplicate`:

```rust
use knowledge_core::project::dedup_store::{
    add_not_duplicate, load_dedup_store, load_not_duplicates, save_dedup_store,
};
```

Add the route after the merge route:

```rust
        .route(
            "/api/projects/{project_id}/dedup/groups/{group_id}/dismiss",
            post(dismiss_dedup_group_handler),
        )
```

Add the handler:

```rust
async fn dismiss_dedup_group_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, group_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let session = authorized_session(&state, &headers, Some(&project_id)).await?;
    validate_csrf(&headers, &session.csrf_token)?;
    let root = project_root_for_id(&state, &project_id).await?;
    let mut store = load_dedup_store(&root).map_err(|error| ApiError::internal(error.to_string()))?;
    let group = store
        .groups
        .iter()
        .find(|group| group.id == group_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("dedup group not found"))?;

    add_not_duplicate(&root, &group.slugs).map_err(|error| ApiError::internal(error.to_string()))?;
    store.groups.retain(|candidate| candidate.id != group_id);
    save_dedup_store(&root, &store).map_err(|error| ApiError::internal(error.to_string()))?;

    append_audit_log(
        &state,
        CreateAuditLog {
            project_id: Some(project_id),
            actor_id: session.user_id,
            action: "dedup.group.dismissed".to_string(),
            target_type: "dedup".to_string(),
            target_id: group_id,
            task_id: None,
            summary: format!("Marked {} as not duplicates", group.slugs.join(" / ")),
            metadata: json!({ "slugs": group.slugs }),
        },
    )
    .await?;
    Ok(Json(json!({ "dismissed": true })))
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p rust-integration --test dedup_api -- --test-threads=1`
Expected: all 3 tests PASS.

- [ ] **Step 5: Clippy + commit**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/knowledge-server/src/projects/routes.rs tests/rust-integration/tests/dedup_api.rs
git commit -m "feat: add dedup group dismiss with not-duplicates whitelist"
```

---

### Task 10: Admin API client + queries

**Files:**
- Modify: `apps/admin/src/features/shared/api.ts`
- Create: `apps/admin/src/features/dedup/queries.ts`

Patterns: `updateProjectReview`/`sweepProjectReviews` (`api.ts:816-847`) for mutations, `csrfHeader()` (`api.ts:252`), `apiFetch` + inline zod schemas; `apps/admin/src/features/reviews/queries.ts` for the hook shapes.

- [ ] **Step 1: Add API functions**

In `apps/admin/src/features/shared/api.ts`, after `sweepProjectReviews` (line ~847), add:

```ts
const dedupGroupSchema = z.object({
  id: z.string(),
  slugs: z.array(z.string()),
  reason: z.string(),
  confidence: z.string(),
  status: z.string(),
  createdAt: z.string(),
});

const dedupOverviewSchema = z.object({
  groups: z.array(dedupGroupSchema),
  notDuplicates: z.array(z.array(z.string())),
});

export type DedupGroup = z.infer<typeof dedupGroupSchema>;

export async function getProjectDedup(projectId: string) {
  return apiFetch(`/api/projects/${projectId}/dedup`, { method: "GET" }, dedupOverviewSchema);
}

export async function detectProjectDuplicates(input: { projectId: string }) {
  return apiFetch(
    `/api/projects/${input.projectId}/dedup:detect`,
    {
      method: "POST",
      headers: csrfHeader(),
    },
    z.object({
      taskId: z.string(),
      status: z.string(),
    }),
  );
}

export async function mergeProjectDuplicateGroup(input: {
  projectId: string;
  groupId: string;
  canonicalSlug: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/dedup/groups/${input.groupId}/merge`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({ canonicalSlug: input.canonicalSlug }),
    },
    z.object({
      taskId: z.string(),
      status: z.string(),
    }),
  );
}

export async function dismissProjectDuplicateGroup(input: {
  projectId: string;
  groupId: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/dedup/groups/${input.groupId}/dismiss`,
    {
      method: "POST",
      headers: csrfHeader(),
    },
    z.object({ dismissed: z.boolean() }),
  );
}
```

(If `apiFetch` POST callers elsewhere pass `"content-type"` explicitly, mirror them; `updateProjectReview` sends a JSON body with only `csrfHeader()`, so `apiFetch` must already set content-type — follow that.)

- [ ] **Step 2: Create the query hooks**

Create `apps/admin/src/features/dedup/queries.ts`:

```ts
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  detectProjectDuplicates,
  dismissProjectDuplicateGroup,
  getProjectDedup,
  mergeProjectDuplicateGroup,
} from "../shared/api";

export function useProjectDedupQuery(projectId: string) {
  return useQuery({
    queryKey: ["project-dedup", projectId],
    queryFn: () => getProjectDedup(projectId),
    refetchInterval: 3000,
  });
}

function useDedupInvalidation() {
  const queryClient = useQueryClient();
  return async (projectId: string) => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["project-dedup", projectId] }),
      queryClient.invalidateQueries({ queryKey: ["project-tasks", projectId] }),
    ]);
  };
}

export function useDetectDuplicatesMutation() {
  const invalidate = useDedupInvalidation();
  return useMutation({
    mutationFn: detectProjectDuplicates,
    onSuccess: async (_result, variables) => {
      await invalidate(variables.projectId);
    },
  });
}

export function useMergeDuplicateGroupMutation() {
  const invalidate = useDedupInvalidation();
  return useMutation({
    mutationFn: mergeProjectDuplicateGroup,
    onSuccess: async (_result, variables) => {
      await invalidate(variables.projectId);
    },
  });
}

export function useDismissDuplicateGroupMutation() {
  const invalidate = useDedupInvalidation();
  return useMutation({
    mutationFn: dismissProjectDuplicateGroup,
    onSuccess: async (_result, variables) => {
      await invalidate(variables.projectId);
    },
  });
}
```

(`refetchInterval: 3000` keeps the page live while detect/merge tasks run in the background — same poll-style UX the tasks page uses.)

- [ ] **Step 3: Verify build**

Run: `npm run test --workspace @knowledge/admin`
Expected: existing tests PASS (no new tests yet — page test lands in Task 11).

- [ ] **Step 4: Commit**

```bash
git add apps/admin/src/features/shared/api.ts apps/admin/src/features/dedup/queries.ts
git commit -m "feat: add dedup api client functions and query hooks"
```

---

### Task 11: Admin Dedup page + routing

**Files:**
- Create: `apps/admin/src/features/dedup/page.tsx`
- Create: `apps/admin/src/features/dedup/page.test.tsx`
- Modify: `apps/admin/src/app/router.tsx` (import at line ~9, route after `reviews` at line ~43)
- Modify: `apps/admin/src/lib/route-meta.ts` (projectRoutes array, after `"/reviews"`)

Templates: `apps/admin/src/features/reviews/page.tsx` (PageSection/Card/Select layout), `apps/admin/src/features/lint/page.test.tsx` (vi.mock + MemoryRouter test shape).

- [ ] **Step 1: Write the failing page test**

Create `apps/admin/src/features/dedup/page.test.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { DedupPage } from "./page";

const mockDedupOverview = vi.fn();
const mockDetectDuplicates = vi.fn();
const mockMergeGroup = vi.fn();
const mockDismissGroup = vi.fn();

vi.mock("./queries", () => ({
  useProjectDedupQuery: () => ({
    data: mockDedupOverview(),
  }),
  useDetectDuplicatesMutation: () => ({
    mutateAsync: mockDetectDuplicates,
    isPending: false,
  }),
  useMergeDuplicateGroupMutation: () => ({
    mutateAsync: mockMergeGroup,
    isPending: false,
  }),
  useDismissDuplicateGroupMutation: () => ({
    mutateAsync: mockDismissGroup,
    isPending: false,
  }),
}));

function renderDedupPage() {
  const queryClient = new QueryClient();
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/projects/project-1/dedup"]}>
        <Routes>
          <Route path="projects/:projectId/dedup" element={<DedupPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("dedup page", () => {
  it("triggers duplicate detection", async () => {
    const user = userEvent.setup();
    mockDedupOverview.mockReturnValue({ groups: [], notDuplicates: [] });
    mockDetectDuplicates.mockResolvedValue({ taskId: "task-1", status: "queued" });

    renderDedupPage();

    expect(screen.getByText("No duplicate candidates")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Detect Duplicates" }));

    expect(mockDetectDuplicates).toHaveBeenCalledWith({ projectId: "project-1" });
  });

  it("merges a group with the selected canonical slug", async () => {
    const user = userEvent.setup();
    mockDedupOverview.mockReturnValue({
      groups: [
        {
          id: "group-1",
          slugs: ["attention", "attention-mechanism"],
          reason: "Both describe the attention mechanism.",
          confidence: "high",
          status: "candidate",
          createdAt: "2026-06-12T00:00:00Z",
        },
      ],
      notDuplicates: [],
    });
    mockMergeGroup.mockResolvedValue({ taskId: "task-2", status: "queued" });

    renderDedupPage();

    expect(screen.getByText("attention / attention-mechanism")).toBeInTheDocument();
    expect(screen.getByText("Both describe the attention mechanism.")).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Canonical Slug"), "attention-mechanism");
    await user.click(screen.getByRole("button", { name: "Merge" }));

    expect(mockMergeGroup).toHaveBeenCalledWith({
      projectId: "project-1",
      groupId: "group-1",
      canonicalSlug: "attention-mechanism",
    });
  });

  it("dismisses a group as not duplicates", async () => {
    const user = userEvent.setup();
    mockDedupOverview.mockReturnValue({
      groups: [
        {
          id: "group-1",
          slugs: ["attention", "attention-mechanism"],
          reason: "Both describe the attention mechanism.",
          confidence: "high",
          status: "candidate",
          createdAt: "2026-06-12T00:00:00Z",
        },
      ],
      notDuplicates: [["rope", "rotary-embedding"]],
    });
    mockDismissGroup.mockResolvedValue({ dismissed: true });

    renderDedupPage();

    expect(screen.getByText("rope, rotary-embedding")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Not Duplicates" }));

    expect(mockDismissGroup).toHaveBeenCalledWith({
      projectId: "project-1",
      groupId: "group-1",
    });
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- src/features/dedup/page.test.tsx`
Expected: FAIL — cannot resolve `./page` (file does not exist yet).

- [ ] **Step 3: Create the page component**

Create `apps/admin/src/features/dedup/page.tsx`:

```tsx
import { useState } from "react";
import { useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { PageSection } from "@/components/layout/page-section";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Select } from "@/components/ui/select";

import {
  useDetectDuplicatesMutation,
  useDismissDuplicateGroupMutation,
  useMergeDuplicateGroupMutation,
  useProjectDedupQuery,
} from "./queries";

export function DedupPage() {
  const { projectId = "" } = useParams();
  const [canonicalByGroup, setCanonicalByGroup] = useState<Record<string, string>>({});
  const dedup = useProjectDedupQuery(projectId);
  const detectDuplicates = useDetectDuplicatesMutation();
  const mergeGroup = useMergeDuplicateGroupMutation();
  const dismissGroup = useDismissDuplicateGroupMutation();

  const groups = dedup.data?.groups ?? [];
  const notDuplicates = dedup.data?.notDuplicates ?? [];

  return (
    <PageSection
      actions={
        <Button
          disabled={detectDuplicates.isPending}
          onClick={() => detectDuplicates.mutateAsync({ projectId })}
        >
          Detect Duplicates
        </Button>
      }
      description="Detect duplicate wiki pages, then merge them into one canonical page or dismiss false positives."
      title="Dedup"
    >
      {groups.length ? (
        <div className="grid gap-4">
          {groups.map((group) => {
            const canonicalSlug = canonicalByGroup[group.id] ?? group.slugs[0] ?? "";
            return (
              <Card key={group.id}>
                <CardHeader>
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="space-y-1">
                      <CardTitle>{group.slugs.join(" / ")}</CardTitle>
                      <CardDescription>{group.reason}</CardDescription>
                    </div>
                    <Badge variant="outline">{group.confidence}</Badge>
                  </div>
                </CardHeader>
                <CardContent className="grid gap-4">
                  <label className="grid max-w-sm gap-2 text-sm font-medium">
                    Canonical Slug
                    <Select
                      aria-label="Canonical Slug"
                      onChange={(event) =>
                        setCanonicalByGroup((current) => ({
                          ...current,
                          [group.id]: event.target.value,
                        }))
                      }
                      value={canonicalSlug}
                    >
                      {group.slugs.map((slug) => (
                        <option key={slug} value={slug}>
                          {slug}
                        </option>
                      ))}
                    </Select>
                  </label>
                  <div className="flex flex-wrap gap-2">
                    <Button
                      disabled={mergeGroup.isPending}
                      onClick={() =>
                        mergeGroup.mutateAsync({
                          projectId,
                          groupId: group.id,
                          canonicalSlug,
                        })
                      }
                    >
                      Merge
                    </Button>
                    <Button
                      disabled={dismissGroup.isPending}
                      onClick={() => dismissGroup.mutateAsync({ projectId, groupId: group.id })}
                      variant="outline"
                    >
                      Not Duplicates
                    </Button>
                  </div>
                </CardContent>
              </Card>
            );
          })}
        </div>
      ) : (
        <EmptyState
          description="Run detection to scan wiki pages for duplicate candidates."
          title="No duplicate candidates"
        />
      )}

      {notDuplicates.length ? (
        <Card>
          <CardHeader>
            <CardTitle>Dismissed Pairs</CardTitle>
            <CardDescription>These slug groups will not be reported as duplicates again.</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-wrap gap-2">
            {notDuplicates.map((slugs) => (
              <Badge key={slugs.join(",")} variant="outline">
                {slugs.join(", ")}
              </Badge>
            ))}
          </CardContent>
        </Card>
      ) : null}
    </PageSection>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npm run test --workspace @knowledge/admin -- src/features/dedup/page.test.tsx`
Expected: PASS (3 tests).

- [ ] **Step 5: Wire routing**

In `apps/admin/src/app/router.tsx`, add the import alphabetically (after `DashboardPage`, line ~9):

```tsx
import { DedupPage } from "../features/dedup/page";
```

Add the route inside the `projects/:projectId` children, after the `reviews` route (line ~43):

```tsx
            <Route path="dedup" element={<DedupPage />} />
```

In `apps/admin/src/lib/route-meta.ts`, add to `projectRoutes` after the `"/reviews"` entry:

```ts
  { suffix: "/dedup", label: "Dedup" },
```

- [ ] **Step 6: Run the full admin suite**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS, including the new dedup tests.

- [ ] **Step 7: Commit**

```bash
git add apps/admin/src/features/dedup/page.tsx apps/admin/src/features/dedup/page.test.tsx apps/admin/src/app/router.tsx apps/admin/src/lib/route-meta.ts
git commit -m "feat: add dedup admin page with merge and dismiss actions"
```

---

### Task 12: Playwright e2e — prompt-aware mock provider + dedup flow

**Files:**
- Modify: `tests/web/mock-openai.mjs` (full rewrite, currently 72 lines)
- Create: `tests/web/tests/dedup.spec.ts`

The current mock serves a single canned chat completion. Dedup needs the mock to (a) answer the detector prompt with a duplicate group, (b) answer the merger prompt with a merged page, and (c) serve `/v1/embeddings` for the prefilter. Branch on the **system prompt content**, never on request order — embedding indexing makes call counts nondeterministic. This mirrors the Rust mock (`tests/rust-integration/tests/support/mock_openai.rs`, `DedupSuccess` scenario).

Prompt markers (verbatim substrings of the prompts defined in Tasks 2–3):
- Detector system prompt contains `"Identify groups of slugs"`.
- Merger system prompt contains `"describe the same entity or concept under different names"`.

- [ ] **Step 1: Rewrite the mock server**

Replace the entire contents of `tests/web/mock-openai.mjs` with:

```js
import http from "node:http";

const port = Number(process.env.KNOWLEDGE_MOCK_OPENAI_PORT ?? "18080");

const DETECTOR_REPLY = JSON.stringify({
  groups: [
    {
      slugs: ["attention", "attention-mechanism"],
      reason: "Both describe the attention mechanism.",
      confidence: "high",
    },
  ],
});

const MERGER_REPLY = [
  "---",
  "title: Attention Mechanism",
  "type: concept",
  "---",
  "",
  "Attention focuses computation on relevant tokens across the sequence.",
].join("\n");

const DEFAULT_REPLY = "Attention focuses computation on relevant tokens.";

function chatContentFor(parsed) {
  const messages = Array.isArray(parsed.messages) ? parsed.messages : [];
  const system = messages.find((message) => message.role === "system");
  const systemText = typeof system?.content === "string" ? system.content : "";

  if (systemText.includes("Identify groups of slugs")) {
    return DETECTOR_REPLY;
  }
  if (systemText.includes("describe the same entity or concept under different names")) {
    return MERGER_REPLY;
  }
  return DEFAULT_REPLY;
}

function chatPayload(content) {
  return JSON.stringify({
    id: "chatcmpl-web-mock-1",
    object: "chat.completion",
    created: 1717171717,
    model: "mock-model",
    choices: [
      {
        index: 0,
        message: { role: "assistant", content },
        finish_reason: "stop",
      },
    ],
    usage: {
      prompt_tokens: 17,
      completion_tokens: 25,
      total_tokens: 42,
    },
  });
}

function fakeEmbeddingForText(text) {
  const lower = String(text).toLowerCase();
  if (lower.includes("rope") || lower.includes("rotary")) {
    return [1, 0, 0];
  }
  if (lower.includes("attention")) {
    return [0, 1, 0];
  }
  return [0, 0, 1];
}

const server = http.createServer((request, response) => {
  if (request.method === "POST" && request.url === "/v1/chat/completions") {
    let body = "";
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      let parsed = {};
      try {
        parsed = JSON.parse(body);
      } catch {
        parsed = {};
      }

      if (parsed.stream === true) {
        response.writeHead(200, { "content-type": "text/event-stream" });
        response.write('data: {"choices":[{"delta":{"content":"Attention "}}]}\n\n');
        response.write(
          'data: {"choices":[{"delta":{"content":"focuses computation on relevant tokens."}}]}\n\n',
        );
        response.write("data: [DONE]\n\n");
        response.end();
        return;
      }

      response.writeHead(200, { "content-type": "application/json" });
      response.end(chatPayload(chatContentFor(parsed)));
    });
    return;
  }

  if (request.method === "POST" && request.url === "/v1/embeddings") {
    let body = "";
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      let parsed = {};
      try {
        parsed = JSON.parse(body);
      } catch {
        parsed = {};
      }

      const input = typeof parsed.input === "string" ? parsed.input : "";
      response.writeHead(200, { "content-type": "application/json" });
      response.end(
        JSON.stringify({
          object: "list",
          data: [
            {
              object: "embedding",
              index: 0,
              embedding: fakeEmbeddingForText(input),
            },
          ],
          model: "mock-embedding",
          usage: { prompt_tokens: 4, total_tokens: 4 },
        }),
      );
    });
    return;
  }

  response.writeHead(404, { "content-type": "application/json" });
  response.end(JSON.stringify({ error: "not found" }));
});

server.listen(port, "127.0.0.1", () => {
  process.stdout.write(`mock-openai listening on ${port}\n`);
});

function shutdown() {
  server.close(() => process.exit(0));
}

process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);
```

Note the chat stream branch is unchanged — `chat.spec.ts` depends on it. The non-stream default reply is also unchanged, so `query`/`lint` specs keep passing.

- [ ] **Step 2: Write the e2e spec**

Create `tests/web/tests/dedup.spec.ts`:

```ts
import { expect, test } from "@playwright/test";

test("admin detects and merges duplicate wiki pages", async ({ page }) => {
  await signInAsAdmin(page);

  await page.getByRole("link", { name: "Settings" }).click();
  await page.getByLabel("Provider Mode", { exact: true }).fill("openai-compatible");
  await page.getByLabel("Language", { exact: true }).fill("en");
  await page.getByLabel("Provider Base URL", { exact: true }).fill("http://127.0.0.1:18080/v1");
  await page.getByLabel("Provider API Key", { exact: true }).fill("test-key");
  await page.getByLabel("Provider Model", { exact: true }).fill("mock-model");
  await page.getByLabel("Provider Embedding Model", { exact: true }).fill("mock-embedding");
  await page.getByLabel("Provider Timeout Seconds", { exact: true }).fill("30");
  await page.getByRole("button", { name: "Save Settings" }).click();

  await page.getByRole("link", { name: "Projects" }).click();
  await createProject(page, "dedup-project");
  const projectId = page.url().split("/").pop() ?? "";

  await seedWikiPage(
    page,
    projectId,
    "wiki/concepts/attention.md",
    [
      "---",
      "title: Attention",
      "type: concept",
      "description: How attention weighs token relevance.",
      "---",
      "",
      "Attention weighs token relevance.",
    ].join("\n"),
  );
  await seedWikiPage(
    page,
    projectId,
    "wiki/concepts/attention-mechanism.md",
    [
      "---",
      "title: Attention Mechanism",
      "type: concept",
      "description: The attention mechanism in transformers.",
      "---",
      "",
      "The attention mechanism scores pairwise token relevance.",
    ].join("\n"),
  );
  await seedWikiPage(
    page,
    projectId,
    "wiki/concepts/rope.md",
    [
      "---",
      "title: RoPE",
      "type: concept",
      "description: Rotary position embedding.",
      "---",
      "",
      "RoPE rotates query and key vectors by position.",
    ].join("\n"),
  );

  await page.getByRole("tab", { name: "Dedup" }).click();
  await page.getByRole("button", { name: "Detect Duplicates" }).click();

  await expect(page.getByText("attention / attention-mechanism")).toBeVisible({
    timeout: 30_000,
  });
  await expect(page.getByText("Both describe the attention mechanism.")).toBeVisible();

  await page.getByLabel("Canonical Slug").selectOption("attention-mechanism");
  await page.getByRole("button", { name: "Merge" }).click();

  await expect(page.getByText("No duplicate candidates")).toBeVisible({ timeout: 30_000 });

  const csrf = await page.evaluate(() => window.sessionStorage.getItem("knowledge.csrfToken"));
  const canonical = await page.request.get(
    `/api/projects/${projectId}/files/content?path=${encodeURIComponent(
      "wiki/concepts/attention-mechanism.md",
    )}`,
    { headers: { "x-csrf-token": csrf ?? "" } },
  );
  expect(canonical.ok()).toBeTruthy();
  const canonicalBody = await canonical.json();
  expect(canonicalBody.content).toContain(
    "Attention focuses computation on relevant tokens across the sequence.",
  );

  const deleted = await page.request.get(
    `/api/projects/${projectId}/files/content?path=${encodeURIComponent(
      "wiki/concepts/attention.md",
    )}`,
    { headers: { "x-csrf-token": csrf ?? "" } },
  );
  expect(deleted.ok()).toBeFalsy();
});

async function seedWikiPage(
  page: import("@playwright/test").Page,
  projectId: string,
  path: string,
  content: string,
) {
  const csrf = await page.evaluate(() => window.sessionStorage.getItem("knowledge.csrfToken"));
  const response = await page.request.put(`/api/projects/${projectId}/files/content`, {
    data: { path, content },
    headers: { "x-csrf-token": csrf ?? "" },
  });
  if (!response.ok()) {
    throw new Error(`failed to seed ${path}: ${response.status()}`);
  }
}

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

The `files/content` GET shape is verified against `getProjectFileContent` (`apps/admin/src/features/shared/api.ts:415`): `GET /api/projects/{id}/files/content?path=...` returns `{ path, content }`. GET requests don't need the CSRF header — sending it is harmless.

- [ ] **Step 3: Run the dedup spec**

Postgres/Redis must be up (`npm run docker:up` or just the db/redis services). Then:

Run: `npm run test --workspace @knowledge/web -- tests/dedup.spec.ts`
Expected: PASS. The spec drives: settings → seed 3 pages → detect (embedding prefilter pairs attention/attention-mechanism, excludes rope; LLM confirms) → merge into `attention-mechanism` → group disappears → canonical file contains merged body → `attention.md` deleted.

- [ ] **Step 4: Run the existing chat spec to check for regressions**

Run: `npm run test --workspace @knowledge/web -- tests/chat.spec.ts`
Expected: PASS — stream branch and default reply unchanged.

- [ ] **Step 5: Commit**

```bash
git add tests/web/mock-openai.mjs tests/web/tests/dedup.spec.ts
git commit -m "test: add dedup e2e flow with prompt-aware mock provider"
```

---

### Task 13: Full verification

**Files:** none (verification only)

- [ ] **Step 1: Rust formatting and lint**

Run: `cargo fmt --all -- --check`
Expected: no output (clean). If it reports diffs in files this plan touched, run `cargo fmt --all` and include in the final commit.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean compile, zero warnings.

- [ ] **Step 2: Rust tests**

Postgres/Redis must be running (ports 55432/56379). Then:

Run: `cargo test --workspace -- --test-threads=1`
Expected: PASS, including `dedup_detect_review_and_merge_flow` in `tests/rust-integration` and the unit tests in `knowledge-core` (`dedup`, `dedup_store`) and `knowledge-server` (`retrieval::dedup`).

- [ ] **Step 3: Frontend tests and lint**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS, including `src/features/dedup/page.test.tsx`.

Run: `npm run lint`
Expected: eslint and clippy both clean.

- [ ] **Step 4: Playwright suite**

Run: `npm run test --workspace @knowledge/web`
Expected: PASS — all specs including `dedup.spec.ts`.

- [ ] **Step 5: Final commit (only if fixes were needed)**

```bash
git status
```

If steps 1–4 required fixes, commit them:

```bash
git add -u
git commit -m "fix: address phase 3c verification findings"
```

If everything was already clean, there is nothing to commit — the work is complete.
